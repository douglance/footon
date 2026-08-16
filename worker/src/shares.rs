#![forbid(unsafe_code)]

use footon_core::model::{MAX_TITLE_CHARS, Share};
use serde::{Deserialize, Serialize};
use worker::d1::{D1PreparedStatement, D1Type};
use worker::{Env, Request, Response, Result};

use crate::access::{
    GeneralAccess, ShareAccess, ShareAction, ShareRole, action_is_available_for_plan,
    load_share_access,
};

const MAX_NAMED_MEMBERS: i64 = 50;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ShareAccessResponse {
    share_id: String,
    general_access: GeneralAccess,
    actor_role: ShareRole,
    owner: ShareOwner,
    members: Vec<ShareMember>,
}

#[derive(Debug, Serialize)]
struct ShareOwner {
    email: String,
    role: ShareRole,
}

#[derive(Debug, Clone, Deserialize)]
struct ShareMemberRow {
    id: String,
    email: String,
    role: String,
    created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ShareMember {
    id: String,
    email: String,
    role: ShareRole,
    created_at: String,
}

impl TryFrom<ShareMemberRow> for ShareMember {
    type Error = worker::Error;

    fn try_from(row: ShareMemberRow) -> std::result::Result<Self, Self::Error> {
        let role = ShareRole::from_db(&row.role)
            .filter(|role| *role != ShareRole::Owner)
            .ok_or_else(|| worker::Error::RustError("invalid stored member role".to_string()))?;
        Ok(Self {
            id: row.id,
            email: row.email,
            role,
            created_at: row.created_at,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ShareUpdateInput {
    title: Option<String>,
    general_access: Option<GeneralAccess>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ShareMetadataResponse {
    id: String,
    title: String,
    general_access: GeneralAccess,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MemberInput {
    email: String,
    role: ShareRole,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TransferInput {
    email: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ToolUpdateInput {
    id: String,
    title: Option<String>,
    general_access: Option<GeneralAccess>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolMemberInput {
    id: String,
    email: String,
    role: ShareRole,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolEmailInput {
    id: String,
    email: String,
}

#[derive(Debug, Deserialize)]
struct CountRow {
    count: i64,
}

pub(crate) async fn api_access(req: &Request, env: &Env, id: &str) -> Result<Response> {
    let Some(user) = crate::bearer_user(req, env).await? else {
        return Response::error("invalid bearer token", 401);
    };
    if !crate::has_scope(&user, "shares:read") {
        return Response::error("missing scope: shares:read", 403);
    }
    crate::validate_share_id(id)?;
    let Some(access) = actor_access(env, id, &user).await? else {
        return Response::error("not found", 404);
    };
    if !access.allows(ShareAction::ViewAccess) {
        return Response::error("not found", 404);
    }
    if !action_is_available_for_plan(env, &access, ShareAction::ViewAccess, None).await? {
        return Response::error("private sharing requires Pro", 402);
    }
    let actor_role = access
        .role
        .ok_or_else(|| worker::Error::RustError("missing actor role".to_string()))?;
    let members = env
        .d1("DB")?
        .prepare(
            "SELECT id, email, role, created_at FROM share_members
             WHERE share_id = ?1 ORDER BY created_at LIMIT 50",
        )
        .bind_refs(&[D1Type::Text(id)])?
        .all()
        .await?
        .results::<ShareMemberRow>()?
        .into_iter()
        .map(ShareMember::try_from)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    crate::json_response(&ShareAccessResponse {
        share_id: id.to_string(),
        general_access: access.general_access,
        actor_role,
        owner: ShareOwner {
            email: owner_email(&access.owner_id)?.to_string(),
            role: ShareRole::Owner,
        },
        members,
    })
}

#[allow(clippy::too_many_lines)]
pub(crate) async fn api_update(req: &mut Request, env: &Env, id: &str) -> Result<Response> {
    let Some(user) = crate::bearer_user(req, env).await? else {
        return Response::error("invalid bearer token", 401);
    };
    if !crate::has_scope(&user, "shares:write") {
        return Response::error("missing scope: shares:write", 403);
    }
    crate::validate_share_id(id)?;
    let input = req.json::<ShareUpdateInput>().await?;
    if input.title.is_none() && input.general_access.is_none() {
        return Response::error("title or generalAccess is required", 400);
    }
    let Some(access) = actor_access(env, id, &user).await? else {
        return Response::error("not found", 404);
    };
    if input.title.is_some() && !access.allows(ShareAction::Rename)
        || input.general_access.is_some() && !access.allows(ShareAction::ChangeAccess)
    {
        return Response::error("not found", 404);
    }

    let record = crate::load_share(env, id)
        .await?
        .ok_or_else(|| worker::Error::RustError("share not found".to_string()))?;
    let title = input
        .title
        .as_deref()
        .map_or(record.title.as_str(), str::trim);
    if title.is_empty() || title.chars().count() > MAX_TITLE_CHARS {
        return Response::error("title must contain 1 to 160 characters", 400);
    }
    let general_access = input.general_access.unwrap_or(access.general_access);
    if input.title.is_some()
        && !action_is_available_for_plan(env, &access, ShareAction::Rename, None).await?
    {
        return Response::error("private sharing requires Pro", 402);
    }
    if input.general_access.is_some()
        && !action_is_available_for_plan(
            env,
            &access,
            ShareAction::ChangeAccess,
            Some(general_access),
        )
        .await?
    {
        return Response::error("private sharing requires Pro", 402);
    }
    if general_access == GeneralAccess::Private && access.general_access == GeneralAccess::Public {
        if !crate::private_expansion_enabled(env) {
            return Response::error("private sharing is temporarily unavailable", 503);
        }
        if !crate::user_is_pro(env, &access.owner_id).await? {
            return Response::error("private sharing requires Pro", 402);
        }
        if !crate::user_has_private_share_capacity(env, &access.owner_id).await? {
            return Response::error("private share limit reached", 402);
        }
    }

    let db = env.d1("DB")?;
    let mut statements = Vec::<D1PreparedStatement>::new();
    if input.title.is_some() {
        let document = Share {
            schema_version: record.document.schema_version,
            title: title.to_string(),
            approved_at: record.document.approved_at,
            messages: record.document.messages,
            report: record.document.report,
        };
        let document_json = serde_json::to_string(&document)?;
        statements.push(
            db.prepare(
                "UPDATE shares SET title = ?1, document_json = ?2
                 WHERE id = ?3 AND revoked_at IS NULL AND (
                   owner_id = ?4 OR EXISTS (
                     SELECT 1 FROM share_members
                     WHERE share_id = ?3 AND email = ?5 AND role = 'editor'
                   )
                 )",
            )
            .bind_refs(&[
                D1Type::Text(title),
                D1Type::Text(&document_json),
                D1Type::Text(id),
                D1Type::Text(&user.user_id),
                D1Type::Text(&user.email),
            ])?,
        );
    }
    if input.general_access.is_some() {
        statements.push(
            db.prepare(
                "UPDATE shares SET general_access = ?1
                 WHERE id = ?2 AND owner_id = ?3 AND revoked_at IS NULL",
            )
            .bind_refs(&[
                D1Type::Text(general_access.as_db_str()),
                D1Type::Text(id),
                D1Type::Text(&user.user_id),
            ])?,
        );
        if general_access == GeneralAccess::Public {
            statements.push(
                db.prepare("DELETE FROM share_members WHERE share_id = ?1")
                    .bind_refs(&[D1Type::Text(id)])?,
            );
            statements.push(
                db.prepare("DELETE FROM share_viewer_challenges WHERE share_id = ?1")
                    .bind_refs(&[D1Type::Text(id)])?,
            );
        }
    }
    let results = db.batch(statements).await?;
    if results
        .first()
        .and_then(|result| result.meta().ok().flatten())
        .and_then(|meta| meta.changes)
        == Some(0)
    {
        return Response::error("conflicting share update", 409);
    }
    crate::json_response(&ShareMetadataResponse {
        id: id.to_string(),
        title: title.to_string(),
        general_access,
    })
}

#[allow(clippy::too_many_lines)]
pub(crate) async fn api_grant(req: &mut Request, env: &Env, id: &str) -> Result<Response> {
    let Some(user) = crate::bearer_user(req, env).await? else {
        return Response::error("invalid bearer token", 401);
    };
    if !crate::has_scope(&user, "shares:write") {
        return Response::error("missing scope: shares:write", 403);
    }
    crate::validate_share_id(id)?;
    let input = req.json::<MemberInput>().await?;
    if input.role == ShareRole::Owner {
        return Response::error("member role must be viewer or editor", 400);
    }
    let Some(email) = crate::normalize_email(&input.email) else {
        return Response::error("valid email is required", 400);
    };
    let Some(access) = actor_access(env, id, &user).await? else {
        return Response::error("not found", 404);
    };
    if access.general_access != GeneralAccess::Private {
        return Response::error("members require a private share", 409);
    }
    if email == owner_email(&access.owner_id)? {
        return Response::error("owner is already a member", 409);
    }
    let action = if input.role == ShareRole::Editor {
        ShareAction::ManageEditor
    } else {
        ShareAction::ManageViewer
    };
    if !access.allows(action) {
        return Response::error("not found", 404);
    }
    if !action_is_available_for_plan(env, &access, action, None).await? {
        return Response::error("private sharing requires Pro", 402);
    }
    if !crate::private_expansion_enabled(env) {
        return Response::error("private sharing is temporarily unavailable", 503);
    }
    if !crate::user_is_pro(env, &access.owner_id).await? {
        return Response::error("private sharing requires Pro", 402);
    }

    let db = env.d1("DB")?;
    let existing = db
        .prepare(
            "SELECT id, email, role, created_at FROM share_members
             WHERE share_id = ?1 AND email = ?2",
        )
        .bind_refs(&[D1Type::Text(id), D1Type::Text(&email)])?
        .first::<ShareMemberRow>(None)
        .await?;
    if access.role == Some(ShareRole::Editor)
        && existing.as_ref().is_some_and(|row| row.role == "editor")
    {
        return Response::error("not found", 404);
    }
    let count = db
        .prepare("SELECT COUNT(*) AS count FROM share_members WHERE share_id = ?1")
        .bind_refs(&[D1Type::Text(id)])?
        .first::<CountRow>(None)
        .await?
        .map_or(0, |row| row.count);
    if existing.is_none() && count >= MAX_NAMED_MEMBERS {
        return Response::error("private member limit reached", 402);
    }
    let member_id = existing
        .as_ref()
        .map_or_else(|| crate::token(18), |row| row.id.clone());
    let created_at = existing
        .as_ref()
        .map_or_else(crate::now_string, |row| row.created_at.clone());
    let updated_at = crate::now_string();
    let result = db
        .prepare(
            "INSERT INTO share_members (id, share_id, email, role, created_at, updated_at)
             SELECT ?1, ?2, ?3, ?4, ?5, ?6
             WHERE EXISTS (
               SELECT 1 FROM shares s
               WHERE s.id = ?2 AND s.revoked_at IS NULL
                 AND s.general_access = 'restricted'
                 AND (
                   s.owner_id = ?7 OR (
                     ?4 = 'viewer'
                     AND EXISTS (
                       SELECT 1 FROM share_members actor
                       WHERE actor.share_id = s.id AND actor.email = ?8
                         AND actor.role = 'editor'
                     )
                     AND NOT EXISTS (
                       SELECT 1 FROM share_members target
                       WHERE target.share_id = s.id AND target.email = ?3
                         AND target.role = 'editor'
                     )
                   )
                 )
             )
             ON CONFLICT(share_id, email) DO UPDATE SET
               role = excluded.role, updated_at = excluded.updated_at",
        )
        .bind_refs(&[
            D1Type::Text(&member_id),
            D1Type::Text(id),
            D1Type::Text(&email),
            D1Type::Text(input.role.as_db_str()),
            D1Type::Text(&created_at),
            D1Type::Text(&updated_at),
            D1Type::Text(&user.user_id),
            D1Type::Text(&user.email),
        ])?
        .run()
        .await?;
    if result.meta()?.and_then(|meta| meta.changes) == Some(0) {
        return Response::error("conflicting member update", 409);
    }
    crate::json_response(&ShareMember {
        id: member_id,
        email,
        role: input.role,
        created_at,
    })
}

pub(crate) async fn api_remove_member(
    req: &Request,
    env: &Env,
    id: &str,
    member_id: &str,
) -> Result<Response> {
    let Some(user) = crate::bearer_user(req, env).await? else {
        return Response::error("invalid bearer token", 401);
    };
    if !crate::has_scope(&user, "shares:write") {
        return Response::error("missing scope: shares:write", 403);
    }
    crate::validate_share_id(id)?;
    crate::validate_share_id(member_id)?;
    let Some(access) = actor_access(env, id, &user).await? else {
        return Response::error("not found", 404);
    };
    let target = env
        .d1("DB")?
        .prepare(
            "SELECT id, email, role, created_at FROM share_members
             WHERE id = ?1 AND share_id = ?2",
        )
        .bind_refs(&[D1Type::Text(member_id), D1Type::Text(id)])?
        .first::<ShareMemberRow>(None)
        .await?;
    let Some(target) = target else {
        return if access.allows(ShareAction::ManageViewer) {
            Response::empty().map(|response| response.with_status(204))
        } else {
            Response::error("not found", 404)
        };
    };
    let target_role = ShareRole::from_db(&target.role)
        .ok_or_else(|| worker::Error::RustError("invalid stored member role".to_string()))?;
    let action = if target_role == ShareRole::Editor {
        ShareAction::ManageEditor
    } else {
        ShareAction::ManageViewer
    };
    if !access.allows(action) {
        return Response::error("not found", 404);
    }
    if !action_is_available_for_plan(env, &access, action, None).await? {
        return Response::error("private sharing requires Pro", 402);
    }
    env.d1("DB")?
        .prepare(
            "DELETE FROM share_members WHERE id = ?1 AND share_id = ?2 AND EXISTS (
               SELECT 1 FROM shares s WHERE s.id = ?2 AND s.revoked_at IS NULL
                 AND (s.owner_id = ?3 OR (
                   share_members.role = 'viewer' AND EXISTS (
                     SELECT 1 FROM share_members actor
                     WHERE actor.share_id = s.id AND actor.email = ?4
                       AND actor.role = 'editor'
                   )
                 ))
             )",
        )
        .bind_refs(&[
            D1Type::Text(member_id),
            D1Type::Text(id),
            D1Type::Text(&user.user_id),
            D1Type::Text(&user.email),
        ])?
        .run()
        .await?;
    Response::empty().map(|response| response.with_status(204))
}

pub(crate) async fn api_transfer(req: &mut Request, env: &Env, id: &str) -> Result<Response> {
    let Some(user) = crate::bearer_user(req, env).await? else {
        return Response::error("invalid bearer token", 401);
    };
    if !crate::has_scope(&user, "shares:write") {
        return Response::error("missing scope: shares:write", 403);
    }
    crate::validate_share_id(id)?;
    let input = req.json::<TransferInput>().await?;
    let Some(target_email) = crate::normalize_email(&input.email) else {
        return Response::error("valid email is required", 400);
    };
    let Some(access) = actor_access(env, id, &user).await? else {
        return Response::error("not found", 404);
    };
    if !access.allows(ShareAction::Transfer) {
        return Response::error("not found", 404);
    }
    if target_email == user.email {
        return Response::error("target already owns this share", 409);
    }
    let target_user_id = format!("email:{target_email}");
    if access.general_access == GeneralAccess::Private {
        if !crate::private_expansion_enabled(env) {
            return Response::error("private sharing is temporarily unavailable", 503);
        }
        if !crate::user_is_pro(env, &access.owner_id).await?
            || !crate::user_is_pro(env, &target_user_id).await?
        {
            return Response::error("private transfer requires Pro for both owners", 402);
        }
        if !crate::user_has_private_share_capacity(env, &target_user_id).await? {
            return Response::error("target private share limit reached", 402);
        }
    }

    let db = env.d1("DB")?;
    let now = crate::now_string();
    let mut statements = vec![
        db.prepare("DELETE FROM share_members WHERE share_id = ?1 AND email = ?2")
            .bind_refs(&[D1Type::Text(id), D1Type::Text(&target_email)])?,
    ];
    if access.general_access == GeneralAccess::Private {
        statements.push(
            db.prepare(
                "INSERT INTO share_members
                 (id, share_id, email, role, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 'editor', ?4, ?4)
                 ON CONFLICT(share_id, email) DO UPDATE SET
                   role = 'editor', updated_at = excluded.updated_at",
            )
            .bind_refs(&[
                D1Type::Text(&crate::token(18)),
                D1Type::Text(id),
                D1Type::Text(&user.email),
                D1Type::Text(&now),
            ])?,
        );
    }
    statements.push(
        db.prepare(
            "UPDATE shares SET owner_id = ?1
             WHERE id = ?2 AND owner_id = ?3 AND revoked_at IS NULL",
        )
        .bind_refs(&[
            D1Type::Text(&target_user_id),
            D1Type::Text(id),
            D1Type::Text(&user.user_id),
        ])?,
    );
    let results = db.batch(statements).await?;
    if results
        .last()
        .and_then(|result| result.meta().ok().flatten())
        .and_then(|meta| meta.changes)
        == Some(0)
    {
        return Response::error("conflicting ownership transfer", 409);
    }
    crate::json_response(&serde_json::json!({
        "id": id,
        "owner": { "email": target_email, "role": ShareRole::Owner },
        "generalAccess": access.general_access
    }))
}

pub(crate) async fn tool_access(
    env: &Env,
    user: &crate::AuthUser,
    args: serde_json::Value,
) -> Result<serde_json::Value> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        id: String,
    }
    let input = serde_json::from_value::<Input>(args)?;
    crate::validate_share_id(&input.id)?;
    let access = required_action(env, user, &input.id, ShareAction::ViewAccess, None).await?;
    let members = env
        .d1("DB")?
        .prepare(
            "SELECT id, email, role, created_at FROM share_members
             WHERE share_id = ?1 ORDER BY created_at LIMIT 50",
        )
        .bind_refs(&[D1Type::Text(&input.id)])?
        .all()
        .await?
        .results::<ShareMemberRow>()?
        .into_iter()
        .map(ShareMember::try_from)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    serde_json::to_value(ShareAccessResponse {
        share_id: input.id,
        general_access: access.general_access,
        actor_role: access
            .role
            .ok_or_else(|| worker::Error::RustError("missing actor role".to_string()))?,
        owner: ShareOwner {
            email: owner_email(&access.owner_id)?.to_string(),
            role: ShareRole::Owner,
        },
        members,
    })
    .map_err(Into::into)
}

#[allow(clippy::too_many_lines)]
pub(crate) async fn tool_update(
    env: &Env,
    user: &crate::AuthUser,
    args: serde_json::Value,
) -> Result<serde_json::Value> {
    let input = serde_json::from_value::<ToolUpdateInput>(args)?;
    crate::validate_share_id(&input.id)?;
    if input.title.is_none() && input.general_access.is_none() {
        return Err(worker::Error::RustError(
            "title or generalAccess is required".to_string(),
        ));
    }
    let action = if input.general_access.is_some() {
        ShareAction::ChangeAccess
    } else {
        ShareAction::Rename
    };
    let access = required_action(env, user, &input.id, action, input.general_access).await?;
    if input.title.is_some() && !access.allows(ShareAction::Rename) {
        return Err(worker::Error::RustError("share not found".to_string()));
    }
    if input.title.is_some()
        && !action_is_available_for_plan(env, &access, ShareAction::Rename, None).await?
    {
        return Err(worker::Error::RustError(
            "private sharing requires Pro".to_string(),
        ));
    }
    let record = crate::load_share(env, &input.id)
        .await?
        .ok_or_else(|| worker::Error::RustError("share not found".to_string()))?;
    let title = input
        .title
        .as_deref()
        .map_or(record.title.as_str(), str::trim);
    if title.is_empty() || title.chars().count() > MAX_TITLE_CHARS {
        return Err(worker::Error::RustError(
            "title must contain 1 to 160 characters".to_string(),
        ));
    }
    let general_access = input.general_access.unwrap_or(access.general_access);
    if general_access == GeneralAccess::Private && access.general_access == GeneralAccess::Public {
        require_private_capacity(env, &access.owner_id).await?;
    }
    let db = env.d1("DB")?;
    let mut statements = Vec::<D1PreparedStatement>::new();
    if input.title.is_some() {
        let document = Share {
            schema_version: record.document.schema_version,
            title: title.to_string(),
            approved_at: record.document.approved_at,
            messages: record.document.messages,
            report: record.document.report,
        };
        let document_json = serde_json::to_string(&document)?;
        statements.push(
            db.prepare(
                "UPDATE shares SET title = ?1, document_json = ?2
                 WHERE id = ?3 AND revoked_at IS NULL AND (
                   owner_id = ?4 OR EXISTS (
                     SELECT 1 FROM share_members
                     WHERE share_id = ?3 AND email = ?5 AND role = 'editor'
                   )
                 )",
            )
            .bind_refs(&[
                D1Type::Text(title),
                D1Type::Text(&document_json),
                D1Type::Text(&input.id),
                D1Type::Text(&user.user_id),
                D1Type::Text(&user.email),
            ])?,
        );
    }
    if input.general_access.is_some() {
        statements.push(
            db.prepare(
                "UPDATE shares SET general_access = ?1
                 WHERE id = ?2 AND owner_id = ?3 AND revoked_at IS NULL",
            )
            .bind_refs(&[
                D1Type::Text(general_access.as_db_str()),
                D1Type::Text(&input.id),
                D1Type::Text(&user.user_id),
            ])?,
        );
        if general_access == GeneralAccess::Public {
            statements.push(
                db.prepare("DELETE FROM share_members WHERE share_id = ?1")
                    .bind_refs(&[D1Type::Text(&input.id)])?,
            );
            statements.push(
                db.prepare("DELETE FROM share_viewer_challenges WHERE share_id = ?1")
                    .bind_refs(&[D1Type::Text(&input.id)])?,
            );
        }
    }
    let results = db.batch(statements).await?;
    if results
        .first()
        .and_then(|result| result.meta().ok().flatten())
        .and_then(|meta| meta.changes)
        == Some(0)
    {
        return Err(worker::Error::RustError(
            "conflicting share update".to_string(),
        ));
    }
    serde_json::to_value(ShareMetadataResponse {
        id: input.id,
        title: title.to_string(),
        general_access,
    })
    .map_err(Into::into)
}

#[allow(clippy::too_many_lines)]
pub(crate) async fn tool_grant(
    env: &Env,
    user: &crate::AuthUser,
    args: serde_json::Value,
) -> Result<serde_json::Value> {
    let input = serde_json::from_value::<ToolMemberInput>(args)?;
    crate::validate_share_id(&input.id)?;
    if input.role == ShareRole::Owner {
        return Err(worker::Error::RustError(
            "member role must be viewer or editor".to_string(),
        ));
    }
    let email = crate::normalize_email(&input.email)
        .ok_or_else(|| worker::Error::RustError("valid email is required".to_string()))?;
    let action = if input.role == ShareRole::Editor {
        ShareAction::ManageEditor
    } else {
        ShareAction::ManageViewer
    };
    let access = required_action(env, user, &input.id, action, None).await?;
    if access.general_access != GeneralAccess::Private {
        return Err(worker::Error::RustError(
            "members require a private share".to_string(),
        ));
    }
    if email == owner_email(&access.owner_id)? {
        return Err(worker::Error::RustError(
            "owner is already a member".to_string(),
        ));
    }
    require_private_capacity_without_count(env, &access.owner_id).await?;
    let db = env.d1("DB")?;
    let existing = db
        .prepare(
            "SELECT id, email, role, created_at FROM share_members
             WHERE share_id = ?1 AND email = ?2",
        )
        .bind_refs(&[D1Type::Text(&input.id), D1Type::Text(&email)])?
        .first::<ShareMemberRow>(None)
        .await?;
    if access.role == Some(ShareRole::Editor)
        && existing.as_ref().is_some_and(|row| row.role == "editor")
    {
        return Err(worker::Error::RustError("share not found".to_string()));
    }
    let count = db
        .prepare("SELECT COUNT(*) AS count FROM share_members WHERE share_id = ?1")
        .bind_refs(&[D1Type::Text(&input.id)])?
        .first::<CountRow>(None)
        .await?
        .map_or(0, |row| row.count);
    if existing.is_none() && count >= MAX_NAMED_MEMBERS {
        return Err(worker::Error::RustError(
            "private member limit reached".to_string(),
        ));
    }
    let member_id = existing
        .as_ref()
        .map_or_else(|| crate::token(18), |row| row.id.clone());
    let created_at = existing
        .as_ref()
        .map_or_else(crate::now_string, |row| row.created_at.clone());
    let updated_at = crate::now_string();
    let result = db
        .prepare(
            "INSERT INTO share_members (id, share_id, email, role, created_at, updated_at)
             SELECT ?1, ?2, ?3, ?4, ?5, ?6
             WHERE EXISTS (
               SELECT 1 FROM shares s
               WHERE s.id = ?2 AND s.revoked_at IS NULL
                 AND s.general_access = 'restricted'
                 AND (
                   s.owner_id = ?7 OR (
                     ?4 = 'viewer'
                     AND EXISTS (
                       SELECT 1 FROM share_members actor
                       WHERE actor.share_id = s.id AND actor.email = ?8
                         AND actor.role = 'editor'
                     )
                     AND NOT EXISTS (
                       SELECT 1 FROM share_members target
                       WHERE target.share_id = s.id AND target.email = ?3
                         AND target.role = 'editor'
                     )
                   )
                 )
             )
             ON CONFLICT(share_id, email) DO UPDATE SET
               role = excluded.role, updated_at = excluded.updated_at",
        )
        .bind_refs(&[
            D1Type::Text(&member_id),
            D1Type::Text(&input.id),
            D1Type::Text(&email),
            D1Type::Text(input.role.as_db_str()),
            D1Type::Text(&created_at),
            D1Type::Text(&updated_at),
            D1Type::Text(&user.user_id),
            D1Type::Text(&user.email),
        ])?
        .run()
        .await?;
    if result.meta()?.and_then(|meta| meta.changes) == Some(0) {
        return Err(worker::Error::RustError(
            "conflicting member update".to_string(),
        ));
    }
    serde_json::to_value(ShareMember {
        id: member_id,
        email,
        role: input.role,
        created_at,
    })
    .map_err(Into::into)
}

pub(crate) async fn tool_remove(
    env: &Env,
    user: &crate::AuthUser,
    args: serde_json::Value,
) -> Result<serde_json::Value> {
    let input = serde_json::from_value::<ToolEmailInput>(args)?;
    crate::validate_share_id(&input.id)?;
    let email = crate::normalize_email(&input.email)
        .ok_or_else(|| worker::Error::RustError("valid email is required".to_string()))?;
    let target = env
        .d1("DB")?
        .prepare(
            "SELECT id, email, role, created_at FROM share_members
             WHERE share_id = ?1 AND email = ?2",
        )
        .bind_refs(&[D1Type::Text(&input.id), D1Type::Text(&email)])?
        .first::<ShareMemberRow>(None)
        .await?
        .ok_or_else(|| worker::Error::RustError("member not found".to_string()))?;
    let role = ShareRole::from_db(&target.role)
        .ok_or_else(|| worker::Error::RustError("invalid stored member role".to_string()))?;
    required_action(
        env,
        user,
        &input.id,
        if role == ShareRole::Editor {
            ShareAction::ManageEditor
        } else {
            ShareAction::ManageViewer
        },
        None,
    )
    .await?;
    let result = env
        .d1("DB")?
        .prepare(
            "DELETE FROM share_members WHERE id = ?1 AND share_id = ?2 AND EXISTS (
               SELECT 1 FROM shares s WHERE s.id = ?2 AND s.revoked_at IS NULL
                 AND (s.owner_id = ?3 OR (
                   share_members.role = 'viewer' AND EXISTS (
                     SELECT 1 FROM share_members actor
                     WHERE actor.share_id = s.id AND actor.email = ?4
                       AND actor.role = 'editor'
                   )
                 ))
             )",
        )
        .bind_refs(&[
            D1Type::Text(&target.id),
            D1Type::Text(&input.id),
            D1Type::Text(&user.user_id),
            D1Type::Text(&user.email),
        ])?
        .run()
        .await?;
    if result.meta()?.and_then(|meta| meta.changes) == Some(0) {
        return Err(worker::Error::RustError(
            "conflicting member removal".to_string(),
        ));
    }
    serde_json::to_value(ShareMember::try_from(target)?).map_err(Into::into)
}

pub(crate) async fn tool_transfer(
    env: &Env,
    user: &crate::AuthUser,
    args: serde_json::Value,
) -> Result<serde_json::Value> {
    let input = serde_json::from_value::<ToolEmailInput>(args)?;
    crate::validate_share_id(&input.id)?;
    let target_email = crate::normalize_email(&input.email)
        .ok_or_else(|| worker::Error::RustError("valid email is required".to_string()))?;
    let access = required_action(env, user, &input.id, ShareAction::Transfer, None).await?;
    if target_email == user.email {
        return Err(worker::Error::RustError(
            "target already owns this share".to_string(),
        ));
    }
    let target_user_id = format!("email:{target_email}");
    if access.general_access == GeneralAccess::Private {
        require_private_capacity_without_count(env, &access.owner_id).await?;
        require_private_capacity(env, &target_user_id).await?;
    }
    let db = env.d1("DB")?;
    let now = crate::now_string();
    let mut statements = vec![
        db.prepare("DELETE FROM share_members WHERE share_id = ?1 AND email = ?2")
            .bind_refs(&[D1Type::Text(&input.id), D1Type::Text(&target_email)])?,
    ];
    if access.general_access == GeneralAccess::Private {
        statements.push(
            db.prepare(
                "INSERT INTO share_members
                 (id, share_id, email, role, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 'editor', ?4, ?4)
                 ON CONFLICT(share_id, email) DO UPDATE SET role = 'editor', updated_at = ?4",
            )
            .bind_refs(&[
                D1Type::Text(&crate::token(18)),
                D1Type::Text(&input.id),
                D1Type::Text(&user.email),
                D1Type::Text(&now),
            ])?,
        );
    }
    statements.push(
        db.prepare(
            "UPDATE shares SET owner_id = ?1
             WHERE id = ?2 AND owner_id = ?3 AND revoked_at IS NULL",
        )
        .bind_refs(&[
            D1Type::Text(&target_user_id),
            D1Type::Text(&input.id),
            D1Type::Text(&user.user_id),
        ])?,
    );
    let results = db.batch(statements).await?;
    if results
        .last()
        .and_then(|result| result.meta().ok().flatten())
        .and_then(|meta| meta.changes)
        == Some(0)
    {
        return Err(worker::Error::RustError(
            "conflicting ownership transfer".to_string(),
        ));
    }
    Ok(serde_json::json!({
        "id": input.id,
        "owner": { "email": target_email, "role": ShareRole::Owner },
        "generalAccess": access.general_access
    }))
}

async fn required_action(
    env: &Env,
    user: &crate::AuthUser,
    id: &str,
    action: ShareAction,
    resulting_access: Option<GeneralAccess>,
) -> Result<ShareAccess> {
    let access = actor_access(env, id, user)
        .await?
        .ok_or_else(|| worker::Error::RustError("share not found".to_string()))?;
    if !access.allows(action) {
        return Err(worker::Error::RustError("share not found".to_string()));
    }
    if !action_is_available_for_plan(env, &access, action, resulting_access).await? {
        return Err(worker::Error::RustError(
            "private sharing requires Pro".to_string(),
        ));
    }
    Ok(access)
}

async fn require_private_capacity(env: &Env, owner_id: &str) -> Result<()> {
    require_private_capacity_without_count(env, owner_id).await?;
    if crate::user_has_private_share_capacity(env, owner_id).await? {
        Ok(())
    } else {
        Err(worker::Error::RustError(
            "private share limit reached".to_string(),
        ))
    }
}

async fn require_private_capacity_without_count(env: &Env, owner_id: &str) -> Result<()> {
    if !crate::private_expansion_enabled(env) {
        return Err(worker::Error::RustError(
            "private sharing is temporarily unavailable".to_string(),
        ));
    }
    if crate::user_is_pro(env, owner_id).await? {
        Ok(())
    } else {
        Err(worker::Error::RustError(
            "private sharing requires Pro".to_string(),
        ))
    }
}

async fn actor_access(env: &Env, id: &str, user: &crate::AuthUser) -> Result<Option<ShareAccess>> {
    load_share_access(env, id, Some(&user.user_id), Some(&user.email)).await
}

fn owner_email(owner_id: &str) -> Result<&str> {
    owner_id
        .strip_prefix("email:")
        .ok_or_else(|| worker::Error::RustError("invalid stored owner identity".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn management_inputs_reject_unknown_fields_and_owner_member_role() {
        assert!(
            serde_json::from_value::<ShareUpdateInput>(serde_json::json!({
                "generalAccess": "public",
                "unknown": true
            }))
            .is_err()
        );
        let owner = serde_json::from_value::<MemberInput>(serde_json::json!({
            "email": "owner@example.com",
            "role": "owner"
        }))
        .expect("shape is parsed before the handler rejects owner role");
        assert_eq!(owner.role, ShareRole::Owner);
    }

    #[test]
    fn public_and_private_are_the_only_api_visibility_values() {
        assert_eq!(
            serde_json::to_value(GeneralAccess::Public).expect("public"),
            "public"
        );
        assert_eq!(
            serde_json::to_value(GeneralAccess::Private).expect("private"),
            "private"
        );
        assert!(serde_json::from_str::<GeneralAccess>("\"restricted\"").is_err());
    }
}
