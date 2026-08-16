#![forbid(unsafe_code)]

use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

pub const PRO_PRIVATE_SHARE_LIMIT: u32 = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BillingUpdate {
    pub receipt: BillingEventReceipt,
    pub user: BillingUser,
    pub source: BillingSourceIds,
    pub subscription: Option<SubscriptionProjection>,
    pub entitlement: EntitlementProjection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BillingEventReceipt {
    pub event_id: String,
    pub event_name: String,
    pub data_type: String,
    pub data_id: String,
    pub test_mode: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BillingUser {
    pub user_id: String,
    pub email: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BillingSourceIds {
    pub subscription: Option<String>,
    pub customer: Option<String>,
    pub order: Option<String>,
    pub product: Option<String>,
    pub variant: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionProjection {
    pub subscription_id: String,
    pub status: SubscriptionStatus,
    pub valid_until: Option<String>,
    pub customer_portal_url: Option<String>,
    pub test_mode: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionStatus {
    OnTrial,
    Active,
    Paused,
    PastDue,
    Unpaid,
    Cancelled,
    CancelledGrace,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntitlementProjection {
    GrantPro {
        valid_until: String,
        private_share_limit: u32,
        source_subscription_id: Option<String>,
        customer_portal_url: Option<String>,
    },
    RevokePro {
        reason: RevocationReason,
        source_subscription_id: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevocationReason {
    Cancelled,
    Expired,
    Refunded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BillingError {
    InvalidSignature,
    InvalidJson,
    MissingEmail,
    MissingValidUntil,
    UnsupportedEvent,
    UnsupportedStatus(String),
}

/// Verify the Lemon Squeezy HMAC-SHA256 webhook signature over the exact raw body bytes.
#[must_use]
pub fn verify_lemon_squeezy_signature(
    raw_body: &[u8],
    signing_secret: &str,
    signature_hex: &str,
) -> bool {
    let Some(signature) = decode_sha256_hex(signature_hex.trim()) else {
        return false;
    };
    hmac_sha256(signing_secret.as_bytes(), raw_body)
        .ct_eq(&signature)
        .into()
}

/// Verify a webhook signature, then parse it into the bounded billing update model.
///
/// # Errors
///
/// Returns [`BillingError::InvalidSignature`] when verification fails, or a parse/mapping
/// error from [`billing_update`] when the verified body cannot produce a billing update.
pub fn verified_billing_update(
    raw_body: &[u8],
    signing_secret: &str,
    signature_hex: &str,
) -> Result<BillingUpdate, BillingError> {
    if !verify_lemon_squeezy_signature(raw_body, signing_secret, signature_hex) {
        return Err(BillingError::InvalidSignature);
    }
    billing_update(raw_body)
}

/// Parse an official Lemon Squeezy JSON webhook body into the billing projection update.
///
/// # Errors
///
/// Returns an error when the JSON envelope is invalid, no normalized email can be found,
/// the event/status is unsupported, or a granting lifecycle event lacks an explicit expiry.
pub fn billing_update(raw_body: &[u8]) -> Result<BillingUpdate, BillingError> {
    let envelope: LemonSqueezyEnvelope =
        serde_json::from_slice(raw_body).map_err(|_error| BillingError::InvalidJson)?;
    let email = event_email(&envelope)
        .and_then(|value| normalize_billing_email(&value))
        .ok_or(BillingError::MissingEmail)?;
    let user = BillingUser {
        user_id: billing_user_id(&email),
        email,
    };
    let source = BillingSourceIds {
        subscription: subscription_id(&envelope.data),
        customer: attribute_string(&envelope.data.attributes, "customer_id"),
        order: attribute_string(&envelope.data.attributes, "order_id"),
        product: attribute_string(&envelope.data.attributes, "product_id"),
        variant: attribute_string(&envelope.data.attributes, "variant_id"),
    };
    let portal_url = customer_portal_url(&envelope.data.attributes);
    let status = subscription_status(&envelope)?;
    let entitlement = entitlement_projection(
        &envelope.meta.event_name,
        status,
        &envelope.data.attributes,
        source.subscription.clone(),
        portal_url.clone(),
    )?;
    let projection_status = match (&status, &entitlement) {
        (SubscriptionStatus::Cancelled, EntitlementProjection::GrantPro { .. }) => {
            SubscriptionStatus::CancelledGrace
        }
        _ => status,
    };
    let subscription = source
        .subscription
        .clone()
        .map(|subscription_id| SubscriptionProjection {
            subscription_id,
            status: projection_status,
            valid_until: entitlement.valid_until().map(ToOwned::to_owned),
            customer_portal_url: portal_url,
            test_mode: envelope.meta.test_mode,
        });

    Ok(BillingUpdate {
        receipt: BillingEventReceipt {
            event_id: deterministic_event_id(&envelope.meta.event_name, raw_body),
            event_name: envelope.meta.event_name,
            data_type: envelope.data.data_type,
            data_id: envelope.data.id,
            test_mode: envelope.meta.test_mode,
        },
        user,
        source,
        subscription,
        entitlement,
    })
}

/// Build the idempotency key for a Lemon Squeezy event receipt.
#[must_use]
pub fn deterministic_event_id(event_name: &str, raw_body: &[u8]) -> String {
    format!(
        "lemon-squeezy:{event_name}:{}",
        hex_encode(&Sha256::digest(raw_body))
    )
}

/// Normalize purchaser email addresses using the existing Footon email constraints.
#[must_use]
pub fn normalize_billing_email(value: &str) -> Option<String> {
    let email = value.trim().to_lowercase();
    if email.is_empty() || email.len() > 254 || email.chars().any(char::is_whitespace) {
        return None;
    }
    let (local, domain) = email.split_once('@')?;
    if local.is_empty()
        || local.len() > 64
        || domain.is_empty()
        || domain.contains('@')
        || !domain.contains('.')
    {
        return None;
    }
    let valid_domain = domain.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            && label
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphanumeric)
            && label
                .as_bytes()
                .last()
                .is_some_and(u8::is_ascii_alphanumeric)
    });
    valid_domain.then_some(email)
}

/// Build the existing Footon user identifier for a normalized email.
#[must_use]
pub fn billing_user_id(normalized_email: &str) -> String {
    format!("email:{normalized_email}")
}

impl EntitlementProjection {
    fn valid_until(&self) -> Option<&str> {
        match self {
            Self::GrantPro { valid_until, .. } => Some(valid_until),
            Self::RevokePro { .. } => None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct LemonSqueezyEnvelope {
    meta: LemonSqueezyMeta,
    data: LemonSqueezyData,
}

#[derive(Debug, Deserialize)]
struct LemonSqueezyMeta {
    event_name: String,
    #[serde(default)]
    test_mode: bool,
    #[serde(default)]
    custom_data: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct LemonSqueezyData {
    #[serde(rename = "type")]
    data_type: String,
    id: String,
    attributes: Value,
}

fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut key_block = [0_u8; 64];
    if key.len() > key_block.len() {
        key_block[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }

    let mut inner_key = key_block;
    let mut outer_key = key_block;
    for byte in &mut inner_key {
        *byte ^= 0x36;
    }
    for byte in &mut outer_key {
        *byte ^= 0x5c;
    }

    let mut inner = Sha256::new();
    inner.update(inner_key);
    inner.update(message);

    let mut outer = Sha256::new();
    outer.update(outer_key);
    outer.update(inner.finalize());
    outer.finalize().into()
}

fn decode_sha256_hex(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        return None;
    }
    let mut out = [0_u8; 32];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        out[index] = (hex_nibble(chunk[0])? << 4) | hex_nibble(chunk[1])?;
    }
    Some(out)
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from(HEX[usize::from(byte >> 4)]));
        out.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    out
}

fn event_email(envelope: &LemonSqueezyEnvelope) -> Option<String> {
    envelope
        .meta
        .custom_data
        .as_ref()
        .and_then(|data| first_string(data, &["email", "user_email", "footon_email"]))
        .or_else(|| {
            first_string(
                &envelope.data.attributes,
                &["user_email", "customer_email", "email"],
            )
        })
}

fn first_string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| attribute_string(value, key))
}

fn attribute_string(value: &Value, key: &str) -> Option<String> {
    let raw = value.get(key)?;
    match raw {
        Value::String(value) if !value.is_empty() => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn customer_portal_url(attributes: &Value) -> Option<String> {
    attributes
        .get("urls")
        .and_then(|urls| urls.get("customer_portal"))
        .and_then(Value::as_str)
        .filter(|url| !url.is_empty())
        .map(ToOwned::to_owned)
}

fn subscription_id(data: &LemonSqueezyData) -> Option<String> {
    if data.data_type == "subscriptions" {
        Some(data.id.clone())
    } else {
        attribute_string(&data.attributes, "subscription_id")
    }
}

fn subscription_status(
    envelope: &LemonSqueezyEnvelope,
) -> Result<SubscriptionStatus, BillingError> {
    if is_refund_event(&envelope.meta.event_name) {
        return Ok(SubscriptionStatus::Expired);
    }
    let status = attribute_string(&envelope.data.attributes, "status")
        .ok_or(BillingError::UnsupportedEvent)?;
    match status.as_str() {
        "on_trial" => Ok(SubscriptionStatus::OnTrial),
        "active" => Ok(SubscriptionStatus::Active),
        "paused" => Ok(SubscriptionStatus::Paused),
        "past_due" => Ok(SubscriptionStatus::PastDue),
        "unpaid" => Ok(SubscriptionStatus::Unpaid),
        "cancelled" => Ok(SubscriptionStatus::Cancelled),
        "expired" => Ok(SubscriptionStatus::Expired),
        _ => Err(BillingError::UnsupportedStatus(status)),
    }
}

fn entitlement_projection(
    event_name: &str,
    status: SubscriptionStatus,
    attributes: &Value,
    subscription_id: Option<String>,
    customer_portal_url: Option<String>,
) -> Result<EntitlementProjection, BillingError> {
    if is_refund_event(event_name) {
        return Ok(EntitlementProjection::RevokePro {
            reason: RevocationReason::Refunded,
            source_subscription_id: subscription_id,
        });
    }
    match status {
        SubscriptionStatus::OnTrial | SubscriptionStatus::Active => grant_with(
            first_valid_until(attributes, &["trial_ends_at", "renews_at", "ends_at"]),
            subscription_id,
            customer_portal_url,
        ),
        SubscriptionStatus::Paused | SubscriptionStatus::PastDue | SubscriptionStatus::Unpaid => {
            grant_with(
                first_valid_until(attributes, &["ends_at", "renews_at", "trial_ends_at"]),
                subscription_id,
                customer_portal_url,
            )
        }
        SubscriptionStatus::Cancelled => {
            if let Some(ends_at) = attribute_string(attributes, "ends_at") {
                grant_with(Some(ends_at), subscription_id, customer_portal_url)
            } else {
                Ok(EntitlementProjection::RevokePro {
                    reason: RevocationReason::Cancelled,
                    source_subscription_id: subscription_id,
                })
            }
        }
        SubscriptionStatus::Expired | SubscriptionStatus::CancelledGrace => {
            Ok(EntitlementProjection::RevokePro {
                reason: RevocationReason::Expired,
                source_subscription_id: subscription_id,
            })
        }
    }
}

fn grant_with(
    valid_until: Option<String>,
    subscription_id: Option<String>,
    customer_portal_url: Option<String>,
) -> Result<EntitlementProjection, BillingError> {
    let valid_until = valid_until.ok_or(BillingError::MissingValidUntil)?;
    Ok(EntitlementProjection::GrantPro {
        valid_until,
        private_share_limit: PRO_PRIVATE_SHARE_LIMIT,
        source_subscription_id: subscription_id,
        customer_portal_url,
    })
}

fn first_valid_until(attributes: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| attribute_string(attributes, key))
}

fn is_refund_event(event_name: &str) -> bool {
    matches!(
        event_name,
        "order_refunded" | "subscription_payment_refunded"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    const ACTIVE_SUBSCRIPTION: &[u8] = br#"{
  "meta": {
    "event_name": "subscription_updated",
    "test_mode": true,
    "custom_data": { "email": " Doug.Lance@Example.COM " }
  },
  "data": {
    "type": "subscriptions",
    "id": "sub_123",
    "attributes": {
      "status": "active",
      "customer_id": 456,
      "order_id": 789,
      "product_id": 111,
      "variant_id": 222,
      "renews_at": "2026-09-01T00:00:00Z",
      "ends_at": null,
      "trial_ends_at": null,
      "urls": {
        "customer_portal": "https://app.lemonsqueezy.com/my-orders/abc"
      }
    }
  }
}"#;

    #[test]
    fn verifies_hmac_sha256_hex_signature_before_parsing() {
        let body = b"The quick brown fox jumps over the lazy dog";
        let valid = "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8";

        assert!(verify_lemon_squeezy_signature(body, "key", valid));
        assert!(!verify_lemon_squeezy_signature(
            body,
            "key",
            "0".repeat(64).as_str()
        ));
        assert_eq!(
            verified_billing_update(ACTIVE_SUBSCRIPTION, "key", "0".repeat(64).as_str()),
            Err(BillingError::InvalidSignature)
        );
    }

    #[test]
    fn derives_deterministic_receipt_id_from_event_name_and_raw_body_hash() {
        let body_hash = Sha256::digest(ACTIVE_SUBSCRIPTION);
        let expected = format!("lemon-squeezy:subscription_updated:{body_hash:x}");

        assert_eq!(
            deterministic_event_id("subscription_updated", ACTIVE_SUBSCRIPTION),
            expected
        );
        assert_ne!(
            deterministic_event_id("subscription_updated", ACTIVE_SUBSCRIPTION),
            deterministic_event_id("subscription_created", ACTIVE_SUBSCRIPTION)
        );
    }

    #[test]
    fn normalizes_email_and_derives_existing_footon_user_id() {
        assert_eq!(
            normalize_billing_email(" Doug.Lance@Example.COM "),
            Some("doug.lance@example.com".to_string())
        );
        assert_eq!(
            billing_user_id("doug.lance@example.com"),
            "email:doug.lance@example.com"
        );
        assert_eq!(normalize_billing_email("missing-at.example.com"), None);
    }

    #[test]
    fn active_subscription_grants_pro_until_explicit_renewal_and_preserves_portal_url() {
        let update = billing_update(ACTIVE_SUBSCRIPTION).expect("subscription parses");

        assert_eq!(update.user.email, "doug.lance@example.com");
        assert_eq!(update.user.user_id, "email:doug.lance@example.com");
        assert_eq!(update.receipt.event_name, "subscription_updated");
        assert_eq!(update.receipt.data_type, "subscriptions");
        assert_eq!(update.receipt.data_id, "sub_123");
        assert!(update.receipt.test_mode);
        assert_eq!(
            update.entitlement,
            EntitlementProjection::GrantPro {
                valid_until: "2026-09-01T00:00:00Z".to_string(),
                private_share_limit: PRO_PRIVATE_SHARE_LIMIT,
                source_subscription_id: Some("sub_123".to_string()),
                customer_portal_url: Some("https://app.lemonsqueezy.com/my-orders/abc".to_string()),
            }
        );
    }

    #[test]
    fn cancelled_subscription_grants_only_through_ends_at() {
        let body = br#"{
  "meta": { "event_name": "subscription_cancelled", "custom_data": { "email": "person@example.com" } },
  "data": {
    "type": "subscriptions",
    "id": "sub_cancel",
    "attributes": {
      "status": "cancelled",
      "renews_at": null,
      "ends_at": "2026-08-31T00:00:00Z",
      "urls": {}
    }
  }
}"#;

        let update = billing_update(body).expect("cancelled grace parses");

        assert_eq!(
            update.subscription.expect("subscription projection").status,
            SubscriptionStatus::CancelledGrace
        );
        assert_eq!(
            update.entitlement,
            EntitlementProjection::GrantPro {
                valid_until: "2026-08-31T00:00:00Z".to_string(),
                private_share_limit: PRO_PRIVATE_SHARE_LIMIT,
                source_subscription_id: Some("sub_cancel".to_string()),
                customer_portal_url: None,
            }
        );
    }

    #[test]
    fn expired_and_refunded_events_revoke_pro_without_needing_raw_payload_storage() {
        let expired = br#"{
  "meta": { "event_name": "subscription_expired", "custom_data": { "email": "person@example.com" } },
  "data": {
    "type": "subscriptions",
    "id": "sub_expired",
    "attributes": { "status": "expired", "urls": {} }
  }
}"#;
        let refunded = br#"{
  "meta": { "event_name": "subscription_payment_refunded", "custom_data": { "email": "person@example.com" } },
  "data": {
    "type": "subscription-invoices",
    "id": "inv_123",
    "attributes": { "subscription_id": "sub_refunded", "urls": {} }
  }
}"#;

        assert_eq!(
            billing_update(expired).expect("expired parses").entitlement,
            EntitlementProjection::RevokePro {
                reason: RevocationReason::Expired,
                source_subscription_id: Some("sub_expired".to_string()),
            }
        );
        assert_eq!(
            billing_update(refunded).expect("refund parses").entitlement,
            EntitlementProjection::RevokePro {
                reason: RevocationReason::Refunded,
                source_subscription_id: Some("sub_refunded".to_string()),
            }
        );
    }
}
