#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use worker::d1::D1Type;
use worker::{Env, Request, Response, Result};

const VIEWER_COOKIE: &str = "footon_viewer";
const VIEWER_SESSION_TTL_SECONDS: i64 = 2_592_000;
const VIEWER_CHALLENGE_TTL_SECONDS: i64 = 600;
const VIEWER_MAX_ATTEMPTS: i64 = 5;
const EMAIL_RATE_LIMIT: i64 = 5;
const REQUEST_RATE_LIMIT: i64 = 20;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum GeneralAccess {
    #[default]
    Public,
    Private,
}

impl GeneralAccess {
    pub(crate) const fn as_db_str(self) -> &'static str {
        match self {
            Self::Public => "anyone_with_link",
            Self::Private => "restricted",
        }
    }

    pub(crate) fn from_db(value: &str) -> Option<Self> {
        match value {
            "anyone_with_link" => Some(Self::Public),
            "restricted" => Some(Self::Private),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ShareRole {
    Owner,
    Editor,
    Viewer,
}

impl ShareRole {
    pub(crate) const fn as_db_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Editor => "editor",
            Self::Viewer => "viewer",
        }
    }

    pub(crate) fn from_db(value: &str) -> Option<Self> {
        match value {
            "owner" => Some(Self::Owner),
            "editor" => Some(Self::Editor),
            "viewer" => Some(Self::Viewer),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShareAction {
    Read,
    Rename,
    Blackout,
    ViewAccess,
    ManageViewer,
    ManageEditor,
    ChangeAccess,
    Transfer,
    Revoke,
}

#[derive(Debug, Clone)]
pub(crate) struct ShareAccess {
    pub(crate) owner_id: String,
    pub(crate) general_access: GeneralAccess,
    pub(crate) role: Option<ShareRole>,
}

impl ShareAccess {
    #[must_use]
    pub(crate) const fn allows(&self, action: ShareAction) -> bool {
        allows(self.general_access, self.role, action)
    }
}

#[derive(Deserialize)]
struct ShareAccessRow {
    owner_id: String,
    general_access: String,
    role: Option<String>,
}

pub(crate) async fn load_share_access(
    env: &Env,
    share_id: &str,
    user_id: Option<&str>,
    email: Option<&str>,
) -> Result<Option<ShareAccess>> {
    let user_id = user_id.unwrap_or_default();
    let email = email.unwrap_or_default();
    let row = env
        .d1("DB")?
        .prepare(
            "SELECT s.owner_id, s.general_access,
               CASE
                 WHEN s.owner_id = ?2 THEN 'owner'
                 ELSE (SELECT role FROM share_members
                       WHERE share_id = s.id AND email = ?3)
               END AS role
             FROM shares s
             WHERE s.id = ?1 AND s.revoked_at IS NULL",
        )
        .bind_refs(&[
            D1Type::Text(share_id),
            D1Type::Text(user_id),
            D1Type::Text(email),
        ])?
        .first::<ShareAccessRow>(None)
        .await?;
    row.map(|row| {
        let general_access = GeneralAccess::from_db(&row.general_access)
            .ok_or_else(|| worker::Error::RustError("invalid stored general access".to_string()))?;
        let role = match row.role.as_deref() {
            None => None,
            Some(value) => Some(ShareRole::from_db(value).ok_or_else(|| {
                worker::Error::RustError("invalid stored share role".to_string())
            })?),
        };
        Ok(ShareAccess {
            owner_id: row.owner_id,
            general_access,
            role,
        })
    })
    .transpose()
}

#[derive(Debug, Clone)]
pub(crate) struct BrowserPrincipal {
    pub(crate) user_id: String,
    pub(crate) email: String,
}

#[derive(Deserialize)]
struct BrowserSessionRow {
    user_id: String,
    email: String,
    expires_at: String,
    revoked_at: Option<String>,
}

#[derive(Deserialize)]
struct ViewerChallengeRow {
    share_id: String,
    email: String,
    verification_code_hash: String,
    attempts: i64,
    expires_at: String,
}

#[derive(Deserialize)]
struct EmailForm {
    email: String,
}

#[derive(Deserialize)]
struct CodeForm {
    ticket: String,
    code: String,
}

#[derive(Deserialize)]
struct CountRow {
    count: i64,
}

pub(crate) async fn browser_principal(
    req: &Request,
    env: &Env,
) -> Result<Option<BrowserPrincipal>> {
    let Some(token) = cookie(req, VIEWER_COOKIE)? else {
        return Ok(None);
    };
    let row = env
        .d1("DB")?
        .prepare(
            "SELECT user_id, email, expires_at, revoked_at
             FROM share_browser_sessions WHERE token_hash = ?1",
        )
        .bind_refs(&[D1Type::Text(&crate::hash(&token))])?
        .first::<BrowserSessionRow>(None)
        .await?;
    Ok(row.and_then(|row| {
        (row.revoked_at.is_none()
            && crate::parse_time(&row.expires_at).timestamp() >= crate::unix_now())
        .then_some(BrowserPrincipal {
            user_id: row.user_id,
            email: row.email,
        })
    }))
}

pub(crate) async fn request_viewer_code(
    req: &mut Request,
    env: &Env,
    share_id: &str,
) -> Result<Response> {
    crate::validate_share_id(share_id)?;
    let body = req.text().await?;
    let input = serde_urlencoded::from_str::<EmailForm>(&body).ok();
    let email = input
        .as_ref()
        .and_then(|input| crate::normalize_email(&input.email));
    let ticket = crate::token(32);
    let request_key = request_rate_key(req)?;
    let request_limited = rate_limited(env, &request_key, 3_600, REQUEST_RATE_LIMIT).await?;
    let email_limited = if let Some(email) = email.as_deref() {
        rate_limited(
            env,
            &format!("email:{}", crate::hash(email)),
            900,
            EMAIL_RATE_LIMIT,
        )
        .await?
    } else {
        false
    };
    record_attempt(env, &request_key).await?;
    if let Some(email) = email.as_deref() {
        record_attempt(env, &format!("email:{}", crate::hash(email))).await?;
    }
    if request_limited || email_limited {
        return viewer_code_page(share_id, &ticket, Some("Try again later."))
            .map(|response| response.with_status(429));
    }

    if let Some(email) = email {
        let user_id = format!("email:{email}");
        let authorized = load_share_access(env, share_id, Some(&user_id), Some(&email))
            .await?
            .is_some_and(|access| {
                access.general_access == GeneralAccess::Private && access.allows(ShareAction::Read)
            });
        if authorized {
            let code = crate::email_code();
            let created_at = crate::now_string();
            let expires_at = crate::time_string(crate::unix_now() + VIEWER_CHALLENGE_TTL_SECONDS);
            env.d1("DB")?
                .prepare(
                    "INSERT INTO share_viewer_challenges
                     (ticket_hash, share_id, email, verification_code_hash, attempts, created_at, expires_at)
                     VALUES (?1, ?2, ?3, ?4, 0, ?5, ?6)",
                )
                .bind_refs(&[
                    D1Type::Text(&crate::hash(&ticket)),
                    D1Type::Text(share_id),
                    D1Type::Text(&email),
                    D1Type::Text(&crate::hash(&code)),
                    D1Type::Text(&created_at),
                    D1Type::Text(&expires_at),
                ])?
                .run()
                .await?;
            if crate::send_email_code(env, &email, &code).await.is_err() {
                worker::console_error!(
                    "operation=share_viewer_code result=unavailable reason=email_delivery"
                );
            }
        }
    }
    viewer_code_page(share_id, &ticket, None).map(|response| response.with_status(202))
}

pub(crate) async fn verify_viewer_code(
    req: &mut Request,
    env: &Env,
    share_id: &str,
) -> Result<Response> {
    crate::validate_share_id(share_id)?;
    let body = req.text().await?;
    let input = serde_urlencoded::from_str::<CodeForm>(&body).ok();
    let ticket = input
        .as_ref()
        .map(|input| input.ticket.as_str())
        .unwrap_or_default();
    let row = env
        .d1("DB")?
        .prepare(
            "SELECT share_id, email, verification_code_hash, attempts, expires_at
             FROM share_viewer_challenges WHERE ticket_hash = ?1",
        )
        .bind_refs(&[D1Type::Text(&crate::hash(ticket))])?
        .first::<ViewerChallengeRow>(None)
        .await?;
    let valid_code = input
        .as_ref()
        .and_then(|input| crate::normalize_email_code(&input.code));
    let valid = row.as_ref().is_some_and(|row| {
        row.share_id == share_id
            && row.attempts < VIEWER_MAX_ATTEMPTS
            && crate::parse_time(&row.expires_at).timestamp() >= crate::unix_now()
            && valid_code.is_some_and(|code| crate::hash(code) == row.verification_code_hash)
    });
    if !valid {
        if !ticket.is_empty() {
            env.d1("DB")?
                .prepare(
                    "UPDATE share_viewer_challenges SET attempts = attempts + 1
                     WHERE ticket_hash = ?1",
                )
                .bind_refs(&[D1Type::Text(&crate::hash(ticket))])?
                .run()
                .await?;
        }
        return viewer_code_page(share_id, ticket, Some("Enter the valid 6-digit code."))
            .map(|response| response.with_status(400));
    }
    let row = row.expect("valid challenge has a row");
    let session = crate::token(32);
    let now = crate::now_string();
    let expires_at = crate::time_string(crate::unix_now() + VIEWER_SESSION_TTL_SECONDS);
    let db = env.d1("DB")?;
    db.batch(vec![
        db.prepare("DELETE FROM share_viewer_challenges WHERE ticket_hash = ?1")
            .bind_refs(&[D1Type::Text(&crate::hash(ticket))])?,
        db.prepare(
            "INSERT INTO share_browser_sessions
             (token_hash, user_id, email, created_at, expires_at, revoked_at)
             VALUES (?1, ?2, ?3, ?4, ?5, NULL)",
        )
        .bind_refs(&[
            D1Type::Text(&crate::hash(&session)),
            D1Type::Text(&format!("email:{}", row.email)),
            D1Type::Text(&row.email),
            D1Type::Text(&now),
            D1Type::Text(&expires_at),
        ])?,
    ])
    .await?;
    let mut response = Response::empty()?.with_status(303);
    response
        .headers_mut()
        .set("Location", &format!("/s/{share_id}"))?;
    response.headers_mut().set(
        "Set-Cookie",
        &format!(
            "{VIEWER_COOKIE}={session}; Path=/; Max-Age={VIEWER_SESSION_TTL_SECONDS}; Secure; HttpOnly; SameSite=Lax"
        ),
    )?;
    response
        .headers_mut()
        .set("Cache-Control", "private, no-store")?;
    Ok(response)
}

pub(crate) async fn sign_out_viewer(req: &Request, env: &Env) -> Result<Response> {
    if let Some(token) = cookie(req, VIEWER_COOKIE)? {
        env.d1("DB")?
            .prepare("DELETE FROM share_browser_sessions WHERE token_hash = ?1")
            .bind_refs(&[D1Type::Text(&crate::hash(&token))])?
            .run()
            .await?;
    }
    let mut response = Response::empty()?.with_status(303);
    response.headers_mut().set("Location", "/")?;
    response.headers_mut().set(
        "Set-Cookie",
        &format!("{VIEWER_COOKIE}=; Path=/; Max-Age=0; Secure; HttpOnly; SameSite=Lax"),
    )?;
    Ok(response)
}

pub(crate) fn viewer_signin_page(share_id: &str) -> Result<Response> {
    let html = viewer_page_shell(
        "Open private share",
        "Enter the email address that was granted access.",
        &format!(
            "<form method=\"post\" action=\"/s/{share_id}/signin\" class=\"space-y-4\">
               <div class=\"space-y-2\">
                 <label for=\"email\" class=\"block text-sm font-medium\">Email</label>
                 <input id=\"email\" name=\"email\" type=\"email\" autocomplete=\"email\" required class=\"w-full rounded-md border px-3 py-2\">
               </div>
               <button type=\"submit\" class=\"w-full rounded-md bg-primary px-4 py-2 font-medium text-primary-foreground\">Send access code</button>
             </form>"
        ),
    );
    private_html(html, 200)
}

fn viewer_code_page(share_id: &str, ticket: &str, error: Option<&str>) -> Result<Response> {
    let error = error.map_or_else(String::new, |message| {
        format!("<p role=\"alert\" class=\"rounded-md border border-destructive p-3 text-sm\">{message}</p>")
    });
    let html = viewer_page_shell(
        "Enter access code",
        "If that email has access, Footon sent a 6-digit code.",
        &format!(
            "{error}<form method=\"post\" action=\"/s/{share_id}/verify\" class=\"space-y-4\">
               <input type=\"hidden\" name=\"ticket\" value=\"{ticket}\">
               <div class=\"space-y-2\">
                 <label for=\"code\" class=\"block text-sm font-medium\">6-digit code</label>
                 <input id=\"code\" name=\"code\" inputmode=\"numeric\" autocomplete=\"one-time-code\" pattern=\"[0-9]{{6}}\" maxlength=\"6\" required class=\"w-full rounded-md border px-3 py-2 font-mono tracking-widest\">
               </div>
               <button type=\"submit\" class=\"w-full rounded-md bg-primary px-4 py-2 font-medium text-primary-foreground\">Open private share</button>
             </form>"
        ),
    );
    private_html(html, 200)
}

fn viewer_page_shell(title: &str, description: &str, content: &str) -> String {
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{title} - Footon</title><link rel=\"stylesheet\" href=\"/style.css\"></head><body class=\"access-page min-h-screen bg-background text-foreground\"><main class=\"access-shell mx-auto flex min-h-screen max-w-md items-center px-5 py-12\"><section aria-labelledby=\"private-share-title\" class=\"w-full space-y-6 rounded-xl border bg-card p-6 shadow-sm\"><header class=\"access-header space-y-2\"><p class=\"font-mono text-xs uppercase tracking-widest text-muted-foreground\">Footon private share</p><h1 id=\"private-share-title\" class=\"text-2xl font-semibold\">{title}</h1><p class=\"text-sm text-muted-foreground\">{description}</p></header>{content}</section></main></body></html>"
    )
}

fn private_html(html: String, status: u16) -> Result<Response> {
    let mut response = Response::from_html(html)?.with_status(status);
    response
        .headers_mut()
        .set("Cache-Control", "private, no-store")?;
    Ok(response)
}

fn cookie(req: &Request, name: &str) -> Result<Option<String>> {
    let header = req.headers().get("Cookie")?;
    Ok(header.as_deref().and_then(|header| {
        header.split(';').find_map(|part| {
            let (key, value) = part.trim().split_once('=')?;
            (key == name && !value.is_empty()).then(|| value.to_string())
        })
    }))
}

fn request_rate_key(req: &Request) -> Result<String> {
    let source = req
        .headers()
        .get("CF-Connecting-IP")?
        .unwrap_or_else(|| "unknown".to_string());
    let day = crate::unix_now() / 86_400;
    Ok(format!(
        "request:{}",
        crate::hash(&format!("footon:{day}:{source}"))
    ))
}

async fn rate_limited(env: &Env, key: &str, window: i64, limit: i64) -> Result<bool> {
    let cutoff = crate::time_string(crate::unix_now() - window);
    let count = env
        .d1("DB")?
        .prepare(
            "SELECT COUNT(*) AS count FROM share_auth_attempts
             WHERE rate_key = ?1 AND created_at >= ?2",
        )
        .bind_refs(&[D1Type::Text(key), D1Type::Text(&cutoff)])?
        .first::<CountRow>(None)
        .await?
        .map_or(0, |row| row.count);
    Ok(count >= limit)
}

async fn record_attempt(env: &Env, key: &str) -> Result<()> {
    env.d1("DB")?
        .prepare("INSERT INTO share_auth_attempts (rate_key, created_at) VALUES (?1, ?2)")
        .bind_refs(&[D1Type::Text(key), D1Type::Text(&crate::now_string())])?
        .run()
        .await?;
    Ok(())
}

#[must_use]
pub(crate) const fn allows(
    general_access: GeneralAccess,
    role: Option<ShareRole>,
    action: ShareAction,
) -> bool {
    match role {
        Some(ShareRole::Owner) => true,
        Some(ShareRole::Editor) => matches!(
            action,
            ShareAction::Read
                | ShareAction::Rename
                | ShareAction::Blackout
                | ShareAction::ViewAccess
                | ShareAction::ManageViewer
        ),
        Some(ShareRole::Viewer) => matches!(action, ShareAction::Read),
        None => {
            matches!(general_access, GeneralAccess::Public) && matches!(action, ShareAction::Read)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_links_are_free_read_only_access() {
        assert!(allows(GeneralAccess::Public, None, ShareAction::Read));
        assert!(!allows(GeneralAccess::Public, None, ShareAction::Rename));
    }

    #[test]
    fn private_roles_follow_the_owner_editor_viewer_matrix() {
        let cases = [
            (ShareRole::Viewer, ShareAction::Read, true),
            (ShareRole::Viewer, ShareAction::Rename, false),
            (ShareRole::Editor, ShareAction::Rename, true),
            (ShareRole::Editor, ShareAction::ManageViewer, true),
            (ShareRole::Editor, ShareAction::ManageEditor, false),
            (ShareRole::Owner, ShareAction::ManageEditor, true),
            (ShareRole::Owner, ShareAction::ChangeAccess, true),
            (ShareRole::Owner, ShareAction::Transfer, true),
            (ShareRole::Owner, ShareAction::Revoke, true),
        ];

        for (role, action, expected) in cases {
            assert_eq!(
                allows(GeneralAccess::Private, Some(role), action),
                expected,
                "role={role:?} action={action:?}"
            );
        }
        assert!(!allows(GeneralAccess::Private, None, ShareAction::Read));
    }

    #[test]
    fn database_values_are_explicit_and_migration_keeps_existing_shares_public() {
        assert_eq!(
            GeneralAccess::from_db(GeneralAccess::Public.as_db_str()),
            Some(GeneralAccess::Public)
        );
        assert_eq!(
            ShareRole::from_db(ShareRole::Editor.as_db_str()),
            Some(ShareRole::Editor)
        );
        assert_eq!(GeneralAccess::from_db("unknown"), None);
        assert_eq!(ShareRole::from_db("unknown"), None);

        let migration = include_str!("../../migrations/0006_share_access.sql");
        assert!(migration.contains("DEFAULT 'anyone_with_link'"));
        assert!(migration.contains("UNIQUE (share_id, email)"));
        assert!(migration.contains("ON DELETE CASCADE"));
    }

    #[test]
    fn private_signin_page_is_generic_and_accessible() {
        let share_id = "abcdefghijklmnopqrstuvwx";
        let html = viewer_page_shell(
            "Open private share",
            "Enter the email address that was granted access.",
            "<form><label for=\"email\">Email</label><input id=\"email\"></form>",
        );

        assert!(html.contains("<html lang=\"en\">"));
        assert!(html.contains("aria-labelledby=\"private-share-title\""));
        assert!(html.contains("Footon private share"));
        assert!(!html.contains(share_id));
        assert!(!html.contains("owner"));
        assert!(!html.contains("member"));
    }
}
