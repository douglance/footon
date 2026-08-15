#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use url::Url;
use worker::d1::{D1PreparedStatement, D1Type};
use worker::{Env, Request, Response, Result};

use crate::billing::{
    BillingError, BillingUpdate, EntitlementProjection, SubscriptionStatus, billing_user_id,
    normalize_billing_email, verified_billing_update,
};

pub(crate) const FREE_ACTIVE_SHARE_LIMIT: i32 = 3;
const MAX_WEBHOOK_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CheckoutUrlError {
    InvalidEmail,
    InvalidUrl,
}

pub(crate) async fn checkout(req: &mut Request, env: &Env, interval: &str) -> Result<Response> {
    let binding = match interval {
        "monthly" => "LEMON_SQUEEZY_MONTHLY_CHECKOUT_URL",
        "annual" => "LEMON_SQUEEZY_ANNUAL_CHECKOUT_URL",
        _ => return Response::error("not found", 404),
    };
    let Some(email) = req.form_data().await?.get_field("email") else {
        return Response::error("enter a valid email address", 400);
    };
    let Ok(configured_url) = env.var(binding) else {
        worker::console_error!("checkout result=unavailable reason=configuration");
        return Response::error("checkout is not available yet", 503);
    };
    let url = match build_checkout_url(&configured_url.to_string(), &email) {
        Ok(url) => url,
        Err(CheckoutUrlError::InvalidEmail) => {
            return Response::error("enter a valid email address", 400);
        }
        Err(CheckoutUrlError::InvalidUrl) => {
            worker::console_error!("checkout result=unavailable reason=configuration");
            return Response::error("checkout is not available yet", 503);
        }
    };
    checkout_redirect(&url)
}

fn checkout_redirect(location: &Url) -> Result<Response> {
    Ok(Response::builder()
        .with_status(303)
        .with_header("Location", location.as_str())?
        .empty())
}

pub(crate) fn build_checkout_url(
    configured_url: &str,
    email: &str,
) -> std::result::Result<Url, CheckoutUrlError> {
    let email = normalize_billing_email(email).ok_or(CheckoutUrlError::InvalidEmail)?;
    let user_id = billing_user_id(&email);
    let mut url = Url::parse(configured_url).map_err(|_| CheckoutUrlError::InvalidUrl)?;
    let valid_host = url
        .host_str()
        .is_some_and(|host| host == "lemonsqueezy.com" || host.ends_with(".lemonsqueezy.com"));
    if url.scheme() != "https"
        || !valid_host
        || !url.path().starts_with("/checkout/buy/")
        || url.fragment().is_some()
    {
        return Err(CheckoutUrlError::InvalidUrl);
    }

    let reserved = [
        "checkout[email]",
        "checkout[custom][email]",
        "checkout[custom][user_id]",
    ];
    let preserved = url
        .query_pairs()
        .filter(|(key, _)| !reserved.contains(&key.as_ref()))
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    url.set_query(None);
    {
        let mut query = url.query_pairs_mut();
        query.extend_pairs(preserved);
        query.append_pair("checkout[email]", &email);
        query.append_pair("checkout[custom][email]", &email);
        query.append_pair("checkout[custom][user_id]", &user_id);
    }
    Ok(url)
}

pub(crate) async fn lemon_squeezy_webhook(req: &mut Request, env: &Env) -> Result<Response> {
    let Some(signature) = req.headers().get("X-Signature")? else {
        return Response::error("invalid webhook signature", 401);
    };
    let Ok(secret) = env.secret("LEMON_SQUEEZY_WEBHOOK_SECRET") else {
        worker::console_error!("billing_webhook result=unavailable reason=configuration");
        return Response::error("billing webhook unavailable", 503);
    };
    if request_is_too_large(req)? {
        worker::console_error!("billing_webhook result=rejected reason=body_too_large");
        return Response::error("webhook body too large", 413);
    }
    let body = req.bytes().await?;
    if body.len() > MAX_WEBHOOK_BYTES {
        worker::console_error!("billing_webhook result=rejected reason=body_too_large");
        return Response::error("webhook body too large", 413);
    }
    let update = match verified_billing_update(&body, &secret.to_string(), &signature) {
        Ok(update) => update,
        Err(error) => {
            worker::console_error!(
                "billing_webhook result=rejected reason={}",
                billing_error_code(&error)
            );
            return Response::error(
                "invalid billing event",
                if matches!(error, BillingError::InvalidSignature) {
                    401
                } else {
                    400
                },
            );
        }
    };

    match persist_billing_update(env, &update).await {
        Ok(true) => worker::console_log!("billing_webhook result=processed"),
        Ok(false) => worker::console_log!("billing_webhook result=duplicate"),
        Err(_) => {
            worker::console_error!("billing_webhook result=unavailable reason=database");
            return Response::error("billing webhook unavailable", 503);
        }
    }
    Ok(Response::empty()?.with_status(204))
}

pub(crate) async fn user_is_pro(env: &Env, user_id: &str) -> Result<bool> {
    #[derive(Deserialize)]
    struct Row {
        active: i64,
    }

    let now = crate::now_string();
    let row = env
        .d1("DB")?
        .prepare(
            "SELECT EXISTS (
               SELECT 1 FROM user_entitlements
               WHERE user_id = ?1 AND plan = 'pro'
                 AND revoked_at IS NULL AND valid_until > ?2
             ) AS active",
        )
        .bind_refs(&[D1Type::Text(user_id), D1Type::Text(&now)])?
        .first::<Row>(None)
        .await?;
    Ok(row.is_some_and(|row| row.active == 1))
}

pub(crate) async fn user_has_private_share_capacity(env: &Env, user_id: &str) -> Result<bool> {
    let now = crate::now_string();
    let row = env
        .d1("DB")?
        .prepare(
            "SELECT
               COUNT(*) AS active_shares,
               COALESCE((
                 SELECT active_share_limit
                 FROM user_entitlements
                 WHERE user_id = ?1
                   AND plan = 'pro'
                   AND revoked_at IS NULL
                   AND valid_until > ?2
               ), ?3) AS active_share_limit
             FROM shares
             WHERE owner_id = ?1 AND revoked_at IS NULL
               AND general_access = 'restricted'",
        )
        .bind_refs(&[
            D1Type::Text(user_id),
            D1Type::Text(&now),
            D1Type::Integer(FREE_ACTIVE_SHARE_LIMIT),
        ])?
        .first::<ShareCapacityRow>(None)
        .await?
        .unwrap_or(ShareCapacityRow {
            active_shares: 0,
            active_share_limit: i64::from(FREE_ACTIVE_SHARE_LIMIT),
        });
    Ok(has_share_capacity(
        row.active_shares,
        row.active_share_limit,
    ))
}

pub(crate) async fn billing_status(env: &Env, user_id: &str) -> Result<Response> {
    let now = crate::now_string();
    let row = env
        .d1("DB")?
        .prepare(
            "SELECT
               (SELECT COUNT(*) FROM shares
                WHERE owner_id = ?1 AND revoked_at IS NULL
                  AND general_access = 'restricted') AS active_shares,
               CASE WHEN EXISTS (
                 SELECT 1 FROM user_entitlements
                 WHERE user_id = ?1 AND plan = 'pro'
                   AND revoked_at IS NULL AND valid_until > ?2
               ) THEN 'pro' ELSE 'free' END AS plan,
               COALESCE((
                 SELECT active_share_limit FROM user_entitlements
                 WHERE user_id = ?1 AND plan = 'pro'
                   AND revoked_at IS NULL AND valid_until > ?2
               ), ?3) AS active_share_limit,
               (SELECT valid_until FROM user_entitlements
                WHERE user_id = ?1 AND plan = 'pro'
                  AND revoked_at IS NULL AND valid_until > ?2) AS valid_until,
               (SELECT customer_portal_url FROM user_entitlements
                WHERE user_id = ?1 AND plan = 'pro'
                  AND revoked_at IS NULL AND valid_until > ?2) AS customer_portal_url",
        )
        .bind_refs(&[
            D1Type::Text(user_id),
            D1Type::Text(&now),
            D1Type::Integer(FREE_ACTIVE_SHARE_LIMIT),
        ])?
        .first::<BillingStatusRow>(None)
        .await?
        .unwrap_or(BillingStatusRow {
            plan: "free".to_string(),
            active_shares: 0,
            active_share_limit: i64::from(FREE_ACTIVE_SHARE_LIMIT),
            valid_until: None,
            customer_portal_url: None,
        });
    let response = BillingStatusResponse {
        plan: row.plan,
        active_shares: row.active_shares,
        active_share_limit: row.active_share_limit,
        valid_until: row.valid_until,
        customer_portal_url: row
            .customer_portal_url
            .as_deref()
            .and_then(safe_customer_portal_url),
    };
    crate::json_response(&response)
}

fn safe_customer_portal_url(value: &str) -> Option<String> {
    let url = Url::parse(value).ok()?;
    (url.scheme() == "https"
        && url.host_str() == Some("app.lemonsqueezy.com")
        && url.fragment().is_none())
    .then(|| url.to_string())
}

pub(crate) fn has_share_capacity(active_shares: i64, active_share_limit: i64) -> bool {
    active_shares < active_share_limit
}

fn request_is_too_large(req: &Request) -> Result<bool> {
    Ok(req
        .headers()
        .get("Content-Length")?
        .and_then(|value| value.parse::<usize>().ok())
        .is_some_and(|length| length > MAX_WEBHOOK_BYTES))
}

async fn persist_billing_update(env: &Env, update: &BillingUpdate) -> Result<bool> {
    let db = env.d1("DB")?;
    let now = crate::now_string();
    let mut statements = Vec::<D1PreparedStatement>::new();
    statements.push(
        db.prepare(
            "INSERT OR IGNORE INTO billing_event_receipts
             (event_id, event_name, source_type, source_id, user_id, subscription_id,
              order_id, customer_id, test_mode, received_at, processed_at, error)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, NULL, NULL)",
        )
        .bind_refs(&[
            D1Type::Text(&update.receipt.event_id),
            D1Type::Text(&update.receipt.event_name),
            D1Type::Text(&update.receipt.data_type),
            D1Type::Text(&update.receipt.data_id),
            D1Type::Text(&update.user.user_id),
            optional_text(update.source.subscription.as_deref()),
            optional_text(update.source.order.as_deref()),
            optional_text(update.source.customer.as_deref()),
            D1Type::Boolean(update.receipt.test_mode),
            D1Type::Text(&now),
        ])?,
    );

    if let Some(subscription) = &update.subscription {
        let status = subscription_status_name(subscription.status);
        let grace_until = (subscription.status == SubscriptionStatus::CancelledGrace)
            .then_some(subscription.valid_until.as_deref())
            .flatten();
        statements.push(
            db.prepare(
                "INSERT INTO billing_subscriptions
                 (subscription_id, user_id, email, status, pro_valid_until, grace_until,
                  renews_at, ends_at, trial_ends_at, customer_id, order_id, product_id,
                  variant_id, customer_portal_url, test_mode, source_event_id, created_at, updated_at)
                 SELECT ?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, NULL, ?7, ?8, ?9, ?10, ?11,
                        ?12, ?13, ?14, ?14
                 WHERE EXISTS (
                   SELECT 1 FROM billing_event_receipts
                   WHERE event_id = ?13 AND processed_at IS NULL
                 )
                 ON CONFLICT(subscription_id) DO UPDATE SET
                   user_id = excluded.user_id,
                   email = excluded.email,
                   status = excluded.status,
                   pro_valid_until = excluded.pro_valid_until,
                   grace_until = excluded.grace_until,
                   customer_id = COALESCE(excluded.customer_id, billing_subscriptions.customer_id),
                   order_id = COALESCE(excluded.order_id, billing_subscriptions.order_id),
                   product_id = COALESCE(excluded.product_id, billing_subscriptions.product_id),
                   variant_id = COALESCE(excluded.variant_id, billing_subscriptions.variant_id),
                   customer_portal_url = COALESCE(excluded.customer_portal_url, billing_subscriptions.customer_portal_url),
                   test_mode = excluded.test_mode,
                   source_event_id = excluded.source_event_id,
                   updated_at = excluded.updated_at",
            )
            .bind_refs(&[
                D1Type::Text(&subscription.subscription_id),
                D1Type::Text(&update.user.user_id),
                D1Type::Text(&update.user.email),
                D1Type::Text(status),
                optional_text(subscription.valid_until.as_deref()),
                optional_text(grace_until),
                optional_text(update.source.customer.as_deref()),
                optional_text(update.source.order.as_deref()),
                optional_text(update.source.product.as_deref()),
                optional_text(update.source.variant.as_deref()),
                optional_text(subscription.customer_portal_url.as_deref()),
                D1Type::Boolean(subscription.test_mode),
                D1Type::Text(&update.receipt.event_id),
                D1Type::Text(&now),
            ])?,
        );
    }

    statements.push(entitlement_statement(&db, update, &now)?);
    statements.push(
        db.prepare(
            "UPDATE billing_event_receipts
             SET processed_at = ?1
             WHERE event_id = ?2 AND processed_at IS NULL",
        )
        .bind_refs(&[D1Type::Text(&now), D1Type::Text(&update.receipt.event_id)])?,
    );

    let results = db.batch(statements).await?;
    Ok(results
        .first()
        .and_then(|result| result.meta().ok().flatten())
        .and_then(|meta| meta.changes)
        == Some(1))
}

fn entitlement_statement(
    db: &worker::d1::D1Database,
    update: &BillingUpdate,
    now: &str,
) -> Result<D1PreparedStatement> {
    match &update.entitlement {
        EntitlementProjection::GrantPro {
            valid_until,
            active_share_limit,
            source_subscription_id,
            customer_portal_url,
        } => {
            let active_share_limit = i32::try_from(*active_share_limit).map_err(|_| {
                worker::Error::RustError("billing share limit exceeds D1 integer range".to_string())
            })?;
            db.prepare(
                "INSERT INTO user_entitlements
                 (user_id, plan, active_share_limit, valid_until, grace_until,
                  source_subscription_id, source_event_id, customer_portal_url,
                  test_mode, created_at, updated_at, revoked_at)
                 SELECT ?1, 'pro', ?2, ?3, NULL, ?4, ?5, ?6, ?7, ?8, ?8, NULL
                 WHERE EXISTS (
                   SELECT 1 FROM billing_event_receipts
                   WHERE event_id = ?5 AND processed_at IS NULL
                 )
                 ON CONFLICT(user_id) DO UPDATE SET
                   plan = 'pro',
                   active_share_limit = excluded.active_share_limit,
                   valid_until = excluded.valid_until,
                   grace_until = excluded.grace_until,
                   source_subscription_id = excluded.source_subscription_id,
                   source_event_id = excluded.source_event_id,
                   customer_portal_url = COALESCE(excluded.customer_portal_url, user_entitlements.customer_portal_url),
                   test_mode = excluded.test_mode,
                   updated_at = excluded.updated_at,
                   revoked_at = NULL",
            )
            .bind_refs(&[
                D1Type::Text(&update.user.user_id),
                D1Type::Integer(active_share_limit),
                D1Type::Text(valid_until),
                optional_text(source_subscription_id.as_deref()),
                D1Type::Text(&update.receipt.event_id),
                optional_text(customer_portal_url.as_deref()),
                D1Type::Boolean(update.receipt.test_mode),
                D1Type::Text(now),
            ])
        }
        EntitlementProjection::RevokePro { .. } => db
            .prepare(
                "UPDATE user_entitlements
                 SET plan = 'free', active_share_limit = ?1, valid_until = ?2,
                     source_event_id = ?3, updated_at = ?2, revoked_at = ?2
                 WHERE user_id = ?4
                   AND EXISTS (
                     SELECT 1 FROM billing_event_receipts
                     WHERE event_id = ?3 AND processed_at IS NULL
                   )",
            )
            .bind_refs(&[
                D1Type::Integer(FREE_ACTIVE_SHARE_LIMIT),
                D1Type::Text(now),
                D1Type::Text(&update.receipt.event_id),
                D1Type::Text(&update.user.user_id),
            ]),
    }
}

fn optional_text(value: Option<&str>) -> D1Type<'_> {
    value.map_or(D1Type::Null, D1Type::Text)
}

fn subscription_status_name(status: SubscriptionStatus) -> &'static str {
    match status {
        SubscriptionStatus::OnTrial => "on_trial",
        SubscriptionStatus::Active => "active",
        SubscriptionStatus::Paused => "paused",
        SubscriptionStatus::PastDue => "past_due",
        SubscriptionStatus::Unpaid => "unpaid",
        SubscriptionStatus::Cancelled => "cancelled",
        SubscriptionStatus::CancelledGrace => "cancelled_grace",
        SubscriptionStatus::Expired => "expired",
    }
}

fn billing_error_code(error: &BillingError) -> &'static str {
    match error {
        BillingError::InvalidSignature => "invalid_signature",
        BillingError::InvalidJson => "invalid_json",
        BillingError::MissingEmail => "missing_email",
        BillingError::MissingValidUntil => "missing_valid_until",
        BillingError::UnsupportedEvent => "unsupported_event",
        BillingError::UnsupportedStatus(_) => "unsupported_status",
    }
}

#[derive(Deserialize)]
struct ShareCapacityRow {
    active_shares: i64,
    active_share_limit: i64,
}

#[derive(Deserialize)]
struct BillingStatusRow {
    plan: String,
    active_shares: i64,
    active_share_limit: i64,
    valid_until: Option<String>,
    customer_portal_url: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BillingStatusResponse {
    plan: String,
    active_shares: i64,
    active_share_limit: i64,
    valid_until: Option<String>,
    customer_portal_url: Option<String>,
}

#[cfg(test)]
mod adapter_tests {
    use super::*;

    #[test]
    fn customer_portal_url_accepts_only_the_lemon_squeezy_account_host() {
        assert_eq!(
            safe_customer_portal_url("https://app.lemonsqueezy.com/my-orders/order-1"),
            Some("https://app.lemonsqueezy.com/my-orders/order-1".to_string())
        );
        assert_eq!(
            safe_customer_portal_url("https://app.lemonsqueezy.com.evil.test/my-orders/order-1"),
            None
        );
        assert_eq!(
            safe_customer_portal_url("https://app.lemonsqueezy.com/my-orders/order-1#token"),
            None
        );
    }

    #[test]
    fn billing_status_uses_the_public_camel_case_contract() {
        let value = serde_json::to_value(BillingStatusResponse {
            plan: "pro".to_string(),
            active_shares: 4,
            active_share_limit: 100,
            valid_until: Some("2026-09-01T00:00:00Z".to_string()),
            customer_portal_url: Some("https://app.lemonsqueezy.com/my-orders/order-1".to_string()),
        })
        .expect("billing status");
        assert_eq!(value["plan"], "pro");
        assert_eq!(value["activeShares"], 4);
        assert_eq!(value["activeShareLimit"], 100);
        assert_eq!(value["validUntil"], "2026-09-01T00:00:00Z");
        assert!(value.get("customer_portal_url").is_none());
        assert!(value["customerPortalUrl"].is_string());
    }
}
