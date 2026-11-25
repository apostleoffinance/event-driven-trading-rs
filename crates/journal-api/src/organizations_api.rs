//! Organization, membership and Quant-account entitlement API.
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use domain::{
    EntitlementStatus, MembershipStatus, Organization, OrganizationId, OrganizationRole,
    OrganizationStatus,
};
use serde::Deserialize;

use crate::{auth::ApiAuthError, AppState};

#[derive(Debug, Clone)]
pub struct TenantPrincipal {
    pub user_id: String,
    pub organization_id: String,
    pub role: OrganizationRole,
}

#[derive(Clone, Copy)]
pub enum Permission {
    Read,
    Admin,
    Owner,
}

fn error(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(serde_json::json!({"error":{"code":code,"message":message}})),
    )
        .into_response()
}

#[allow(clippy::result_large_err)]
pub async fn tenant_context(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<TenantPrincipal, Response> {
    let claims = state
        .auth
        .authenticate(headers)
        .map_err(ApiAuthError::into_response)?;
    let org = headers
        .get("x-organization-id")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| {
            error(
                StatusCode::NOT_FOUND,
                "not_found",
                "Organization resource not found",
            )
        })?;
    let membership = state
        .store
        .organization_membership(org, &claims.sub)
        .await
        .map_err(|_| {
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Unable to authorize organization request",
            )
        })?;
    match membership {
        Some((role, MembershipStatus::Active, _revision)) => Ok(TenantPrincipal {
            user_id: claims.sub,
            organization_id: org.to_owned(),
            role,
        }),
        _ => Err(error(
            StatusCode::NOT_FOUND,
            "not_found",
            "Organization resource not found",
        )),
    }
}

#[allow(clippy::result_large_err)]
pub async fn authorize(
    state: &AppState,
    headers: &HeaderMap,
    permission: Permission,
) -> Result<TenantPrincipal, Response> {
    let principal = tenant_context(state, headers).await?;
    let allowed = match permission {
        Permission::Read => true,
        Permission::Admin => principal.role.may_administer(),
        Permission::Owner => principal.role.may_manage_owner(),
    };
    if allowed {
        Ok(principal)
    } else {
        Err(error(
            StatusCode::FORBIDDEN,
            "forbidden",
            "Organization role does not permit this operation",
        ))
    }
}

fn api_role(role: OrganizationRole) -> &'static str {
    role.as_db()
}
fn parse_role(value: &str) -> Option<OrganizationRole> {
    match value {
        "OWNER" => Some(OrganizationRole::Owner),
        "ADMIN" => Some(OrganizationRole::Admin),
        "TRADER" => Some(OrganizationRole::Trader),
        "RISK_MANAGER" => Some(OrganizationRole::RiskManager),
        "VIEWER" => Some(OrganizationRole::Viewer),
        _ => None,
    }
}
fn parse_membership_status(value: &str) -> Option<MembershipStatus> {
    match value {
        "ACTIVE" => Some(MembershipStatus::Active),
        "SUSPENDED" => Some(MembershipStatus::Suspended),
        "REVOKED" => Some(MembershipStatus::Revoked),
        _ => None,
    }
}
fn parse_entitlement_status(value: &str) -> Option<EntitlementStatus> {
    match value {
        "ACTIVE" => Some(EntitlementStatus::Active),
        "SUSPENDED" => Some(EntitlementStatus::Suspended),
        "REVOKED" => Some(EntitlementStatus::Revoked),
        _ => None,
    }
}
fn conflict_response(message: &str) -> Response {
    error(StatusCode::CONFLICT, "revision_conflict", message)
}

#[derive(Deserialize)]
pub struct OrganizationWrite {
    pub name: String,
}

pub async fn list(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let claims = match state.auth.authenticate(&headers) {
        Ok(c) => c,
        Err(e) => return e.into_response(),
    };
    match state.store.organizations_for_user(&claims.sub).await {
        Ok(rows)=>Json(serde_json::json!({"data":rows.into_iter().map(|(org,role,revision)|serde_json::json!({"id":org.id.as_str(),"name":org.name,"status":format!("{:?}",org.status).to_uppercase(),"revision":org.revision,"membershipRole":api_role(role),"membershipRevision":revision})).collect::<Vec<_>>()})).into_response(),
        Err(_)=>error(StatusCode::INTERNAL_SERVER_ERROR,"internal_error","Unable to list organizations"),
    }
}

pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<OrganizationWrite>,
) -> Response {
    let claims = match state.auth.authenticate(&headers) {
        Ok(c) => c,
        Err(e) => return e.into_response(),
    };
    let id = match OrganizationId::new(uuid::Uuid::new_v4().to_string()) {
        Ok(id) => id,
        Err(_) => {
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Unable to allocate organization identifier",
            )
        }
    };
    let org = match Organization::new(id, input.name, Utc::now()) {
        Ok(v) => v,
        Err(_) => {
            return error(
                StatusCode::BAD_REQUEST,
                "invalid_organization",
                "Organization name must contain 1 to 120 characters",
            )
        }
    };
    match state.store.create_organization(&org,&claims.sub).await {
        Ok(workspace)=>(StatusCode::CREATED,Json(serde_json::json!({"data":{"id":org.id.as_str(),"name":org.name,"status":"ACTIVE","revision":1,"membershipRole":"OWNER","membershipRevision":1,"defaultWorkspace":{"id":workspace.id.as_str(),"organizationId":workspace.organization_id.as_str(),"name":workspace.name,"slug":workspace.slug,"status":workspace.status.as_db(),"isDefault":workspace.is_default,"revision":workspace.revision,"createdAt":workspace.created_at,"updatedAt":workspace.updated_at}}}))).into_response(),
        Err(_)=>error(StatusCode::INTERNAL_SERVER_ERROR,"internal_error","Unable to create organization and default workspace"),
    }
}

#[derive(Deserialize)]
pub struct OrganizationUpdate {
    pub expected_revision: i64,
    pub name: Option<String>,
    pub status: Option<String>,
    pub reason: String,
}
#[allow(clippy::result_large_err)]
async fn lifecycle_owner(
    state: &AppState,
    headers: &HeaderMap,
    path_org: &str,
) -> Result<TenantPrincipal, Response> {
    let claims = state
        .auth
        .authenticate(headers)
        .map_err(ApiAuthError::into_response)?;
    let selected = headers
        .get("x-organization-id")
        .and_then(|v| v.to_str().ok())
        .map(str::trim);
    if selected != Some(path_org) {
        return Err(error(
            StatusCode::NOT_FOUND,
            "not_found",
            "Organization resource not found",
        ));
    }
    let allowed: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM organizations o JOIN organization_memberships m ON m.organization_id=o.id WHERE o.id=$1 AND o.status IN ('ACTIVE','SUSPENDED') AND m.user_id=$2 AND m.role='OWNER' AND m.status='ACTIVE')")
        .bind(path_org).bind(&claims.sub).fetch_one(state.store.pool()).await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"internal_error","Unable to authorize organization request"))?;
    if !allowed {
        return Err(error(
            StatusCode::NOT_FOUND,
            "not_found",
            "Organization resource not found",
        ));
    }
    Ok(TenantPrincipal {
        user_id: claims.sub,
        organization_id: path_org.to_owned(),
        role: OrganizationRole::Owner,
    })
}
pub async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(org): Path<String>,
    Json(input): Json<OrganizationUpdate>,
) -> Response {
    let actor = match lifecycle_owner(&state, &headers, &org).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if input.reason.trim().is_empty() {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "reason is required",
        );
    }
    let status = match input.status.as_deref() {
        None => None,
        Some("ACTIVE") => Some(OrganizationStatus::Active),
        Some("SUSPENDED") => Some(OrganizationStatus::Suspended),
        Some("CLOSED") => Some(OrganizationStatus::Closed),
        _ => {
            return error(
                StatusCode::BAD_REQUEST,
                "invalid_status",
                "Unknown organization status",
            )
        }
    };
    match state
        .store
        .revise_organization(
            &org,
            &actor.user_id,
            input.expected_revision,
            input.name.as_deref(),
            status,
            &input.reason,
        )
        .await
    {
        Ok(revision) => Json(serde_json::json!({"data":{"revision":revision}})).into_response(),
        Err(_) => conflict_response("Organization update failed or its revision is stale"),
    }
}

#[derive(Deserialize)]
pub struct MembershipCreate {
    pub user_id: String,
    pub role: String,
    pub reason: String,
}
pub async fn members(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(org): Path<String>,
) -> Response {
    match authorize_org_path(&state,&headers,&org,Permission::Admin).await {
        Ok(_)=>match state.store.organization_members(&org).await {Ok(rows)=>Json(serde_json::json!({"data":rows.into_iter().map(|m|serde_json::json!({"organizationId":m.organization_id.as_str(),"userId":m.user_id,"role":api_role(m.role),"status":m.status.as_db(),"revision":m.revision,"createdAt":m.created_at,"updatedAt":m.updated_at})).collect::<Vec<_>>()})).into_response(),Err(_)=>error(StatusCode::INTERNAL_SERVER_ERROR,"internal_error","Unable to list memberships")},
        Err(r)=>r,
    }
}
#[allow(clippy::result_large_err)]
async fn authorize_org_path(
    state: &AppState,
    headers: &HeaderMap,
    org: &str,
    permission: Permission,
) -> Result<TenantPrincipal, Response> {
    let principal = tenant_context(state, headers).await?;
    if principal.organization_id != org {
        return Err(error(
            StatusCode::NOT_FOUND,
            "not_found",
            "Organization resource not found",
        ));
    }
    let allowed = match permission {
        Permission::Read => true,
        Permission::Admin => principal.role.may_administer(),
        Permission::Owner => principal.role.may_manage_owner(),
    };
    if allowed {
        Ok(principal)
    } else {
        Err(error(
            StatusCode::FORBIDDEN,
            "forbidden",
            "Organization role does not permit this operation",
        ))
    }
}
pub async fn add_member(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(org): Path<String>,
    Json(input): Json<MembershipCreate>,
) -> Response {
    let actor = match authorize_org_path(&state, &headers, &org, Permission::Admin).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some(role) = parse_role(&input.role) else {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_role",
            "Unknown organization role",
        );
    };
    if input.reason.trim().is_empty() || input.user_id.trim().is_empty() {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "user_id and reason are required",
        );
    }
    match state
        .store
        .create_organization_membership(&org, &input.user_id, &actor.user_id, role, &input.reason)
        .await
    {
        Ok(()) => StatusCode::CREATED.into_response(),
        Err(_) => error(
            StatusCode::CONFLICT,
            "membership_change_rejected",
            "Membership creation was rejected",
        ),
    }
}
#[derive(Deserialize)]
pub struct MembershipUpdate {
    pub expected_revision: i64,
    pub role: String,
    pub status: String,
    pub reason: String,
}
pub async fn update_member(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((org, user)): Path<(String, String)>,
    Json(input): Json<MembershipUpdate>,
) -> Response {
    let actor = match authorize_org_path(&state, &headers, &org, Permission::Admin).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let (Some(role), Some(status)) = (
        parse_role(&input.role),
        parse_membership_status(&input.status),
    ) else {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_membership",
            "Unknown role or membership status",
        );
    };
    match state
        .store
        .revise_organization_membership(
            &org,
            &user,
            &actor.user_id,
            input.expected_revision,
            role,
            status,
            &input.reason,
        )
        .await
    {
        Ok(revision) => Json(serde_json::json!({"data":{"revision":revision}})).into_response(),
        Err(_) => conflict_response("Membership update failed or its revision is stale"),
    }
}
#[derive(Deserialize)]
pub struct OwnerTransfer {
    pub target_user_id: String,
    pub actor_expected_revision: i64,
    pub target_expected_revision: i64,
    pub reason: String,
}
pub async fn transfer_owner(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(org): Path<String>,
    Json(input): Json<OwnerTransfer>,
) -> Response {
    let actor = match authorize_org_path(&state, &headers, &org, Permission::Owner).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if input.reason.trim().is_empty() {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "reason is required",
        );
    }
    match state
        .store
        .transfer_organization_owner(
            &org,
            &actor.user_id,
            &input.target_user_id,
            input.actor_expected_revision,
            input.target_expected_revision,
            &input.reason,
        )
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(_) => conflict_response("Owner transfer failed or a membership revision is stale"),
    }
}
pub async fn entitlements(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(org): Path<String>,
) -> Response {
    match authorize_org_path(&state,&headers,&org,Permission::Read).await {
        Ok(_)=>match state.store.organization_entitlements(&org).await {Ok(rows)=>Json(serde_json::json!({"data":rows.into_iter().map(|e|serde_json::json!({"organizationId":e.organization_id.as_str(),"accountId":e.account_id.as_str(),"status":e.status.as_db(),"revision":e.revision,"assignedBy":e.assigned_by,"reason":e.reason,"createdAt":e.created_at,"updatedAt":e.updated_at})).collect::<Vec<_>>()})).into_response(),Err(_)=>error(StatusCode::INTERNAL_SERVER_ERROR,"internal_error","Unable to list account entitlements")},Err(r)=>r
    }
}
#[derive(Deserialize)]
pub struct EntitlementAssignment {
    pub expected_account_revision: i64,
    pub expected_entitlement_revision: i64,
    pub reason: String,
}
pub async fn assign_entitlement(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((org, account)): Path<(String, String)>,
    Json(input): Json<EntitlementAssignment>,
) -> Response {
    let actor = match authorize_org_path(&state, &headers, &org, Permission::Owner).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if input.reason.trim().is_empty() {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "reason is required",
        );
    }
    match state
        .store
        .assign_account_entitlement(
            &org,
            &account,
            &actor.user_id,
            input.expected_account_revision,
            input.expected_entitlement_revision,
            &input.reason,
        )
        .await
    {
        Ok(revision) => Json(
            serde_json::json!({"data":{"accountId":account,"revision":revision,"status":"ACTIVE"}}),
        )
        .into_response(),
        Err(_) => conflict_response(
            "Account assignment failed, account is unavailable, or a revision is stale",
        ),
    }
}
#[derive(Deserialize)]
pub struct EntitlementUpdate {
    pub expected_account_revision: i64,
    pub expected_entitlement_revision: i64,
    pub status: String,
    pub reason: String,
}
pub async fn update_entitlement(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((org, account)): Path<(String, String)>,
    Json(input): Json<EntitlementUpdate>,
) -> Response {
    let actor = match authorize_org_path(&state, &headers, &org, Permission::Owner).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some(status) = parse_entitlement_status(&input.status) else {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_status",
            "Unknown entitlement status",
        );
    };
    if input.reason.trim().is_empty() {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "reason is required",
        );
    }
    match state.store.set_account_entitlement_status(&org,&account,&actor.user_id,input.expected_account_revision,input.expected_entitlement_revision,status,&input.reason).await {
        Ok(revision)=>Json(serde_json::json!({"data":{"accountId":account,"revision":revision,"status":status.as_db()}})).into_response(),Err(_)=>conflict_response("Entitlement update failed or a revision is stale"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn permissions_do_not_grant_execution_to_any_role() {
        assert!(OrganizationRole::Owner.may_write_trading_workspace());
        assert!(OrganizationRole::RiskManager.may_manage_risk_configuration());
        assert!(!OrganizationRole::RiskManager.may_write_trading_workspace());
        assert!(!OrganizationRole::Trader.may_administer());
        assert!(!OrganizationRole::Viewer.may_write_trading_workspace());
    }
}
