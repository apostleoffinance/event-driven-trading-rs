//! Workspace request authorization and management API.
//!
//! Header values select a context only. Every request resolves the current
//! session subject, organization membership, workspace and grant from PostgreSQL.
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use domain::{OrganizationId, Workspace, WorkspaceId};
use serde::Deserialize;
use crate::{auth::ApiAuthError, AppState};

#[derive(Debug, Clone)]
pub struct WorkspacePrincipal {
    pub user_id: String,
    pub organization_id: String,
    pub workspace_id: String,
    pub organization_role: String,
    pub workspace_role: String,
    pub grant_revision: Option<i64>,
    pub can_read: bool,
    pub can_trade: bool,
    pub can_manage_risk: bool,
    pub can_manage_workspace: bool,
}

fn error(status: StatusCode, code: &str, message: &str) -> Response {
    (status, Json(serde_json::json!({"error":{"code":code,"message":message}}))).into_response()
}
fn not_found() -> Response {
    error(StatusCode::NOT_FOUND, "not_found", "Workspace resource not found")
}
fn db_error() -> Response {
    error(StatusCode::SERVICE_UNAVAILABLE, "authorization_unavailable", "Workspace authorization could not be verified")
}

/// Resolve an active workspace context. No default workspace fallback is allowed.
pub async fn principal(state: &AppState, headers: &HeaderMap) -> Result<WorkspacePrincipal, Response> {
    let claims = state.auth.authenticate(headers).map_err(ApiAuthError::into_response)?;
    let org = headers.get("x-organization-id").and_then(|v|v.to_str().ok()).map(str::trim).filter(|v|!v.is_empty()).ok_or_else(not_found)?;
    let workspace = headers.get("x-workspace-id").and_then(|v|v.to_str().ok()).map(str::trim).filter(|v|!v.is_empty()).ok_or_else(|| error(StatusCode::BAD_REQUEST,"workspace_context_required","X-Workspace-Id is required for trading resources"))?;
    let row = sqlx::query_as::<_,(String,String,String,String,Option<String>,Option<String>,Option<i64>)>(
        r#"SELECT o.status,m.role,m.status,w.status,g.role,g.status,g.revision
           FROM organizations o
           JOIN organization_memberships m ON m.organization_id=o.id AND m.user_id=$2
           JOIN workspaces w ON w.organization_id=o.id AND w.id=$3
           LEFT JOIN workspace_access_grants g ON g.organization_id=o.id AND g.workspace_id=w.id AND g.user_id=m.user_id
           WHERE o.id=$1"#)
        .bind(org).bind(&claims.sub).bind(workspace).fetch_optional(state.store.pool()).await.map_err(|_|db_error())?;
    let Some((org_status,org_role,member_status,workspace_status,grant_role,grant_status,grant_revision))=row else { return Err(not_found()); };
    if org_status!="ACTIVE" || member_status!="ACTIVE" || workspace_status!="ACTIVE" { return Err(not_found()); }
    let (can_read,can_trade,can_manage_risk,can_manage_workspace,workspace_role)=match org_role.as_str() {
        "OWNER"|"ADMIN" => (true,true,true,true,"WORKSPACE_ADMIN".to_owned()),
        "TRADER"|"RISK_MANAGER"|"VIEWER" => {
            if grant_status.as_deref()!=Some("ACTIVE") { return Err(not_found()); }
            let wr=grant_role.unwrap_or_default();
            let org_trade=org_role=="TRADER";
            let org_risk=org_role=="RISK_MANAGER";
            let (read,trade,risk,manage)=match wr.as_str() {
                "WORKSPACE_ADMIN" => (true,org_trade,org_risk,false),
                "TRADER" => (true,org_trade,false,false),
                "RISK_MANAGER" => (true,false,org_risk,false),
                "VIEWER" => (true,false,false,false),
                _ => return Err(not_found()),
            };
            (read,trade,risk,manage,wr)
        },
        _ => return Err(not_found()),
    };
    Ok(WorkspacePrincipal {
        user_id:claims.sub, organization_id:org.to_owned(), workspace_id:workspace.to_owned(),
        organization_role:org_role, workspace_role, grant_revision,
        can_read, can_trade, can_manage_risk, can_manage_workspace,
    })
}

pub async fn require_org_admin(state:&AppState,headers:&HeaderMap,org:&str)->Result<String,Response>{
    let claims=state.auth.authenticate(headers).map_err(ApiAuthError::into_response)?;
    let selected=headers.get("x-organization-id").and_then(|v|v.to_str().ok()).map(str::trim);
    if selected!=Some(org){return Err(not_found());}
    let allowed:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM organizations o JOIN organization_memberships m ON m.organization_id=o.id WHERE o.id=$1 AND o.status='ACTIVE' AND m.user_id=$2 AND m.status='ACTIVE' AND m.role IN ('OWNER','ADMIN'))")
        .bind(org).bind(&claims.sub).fetch_one(state.store.pool()).await.map_err(|_|db_error())?;
    if !allowed{return Err(not_found());}
    Ok(claims.sub)
}

fn workspace_json(w:&Workspace)->serde_json::Value{
    serde_json::json!({"id":w.id.as_str(),"organizationId":w.organization_id.as_str(),"name":w.name,"slug":w.slug,"status":w.status.as_db(),"isDefault":w.is_default,"revision":w.revision,"createdAt":w.created_at,"updatedAt":w.updated_at})
}

pub async fn list(State(state):State<AppState>,headers:HeaderMap,Path(org):Path<String>)->Response{
    let claims=match state.auth.authenticate(&headers){Ok(v)=>v,Err(e)=>return e.into_response()};
    if headers.get("x-organization-id").and_then(|v|v.to_str().ok()).map(str::trim)!=Some(org.as_str()){return not_found();}
    let rows=sqlx::query_as::<_,(String,String,String,String,String,bool,i64,String,chrono::DateTime<chrono::Utc>,chrono::DateTime<chrono::Utc>)>(
        r#"SELECT w.id,w.organization_id,w.name,w.slug,w.status,w.is_default,w.revision,w.created_by,w.created_at,w.updated_at
           FROM workspaces w JOIN organizations o ON o.id=w.organization_id
           JOIN organization_memberships m ON m.organization_id=o.id AND m.user_id=$2
           LEFT JOIN workspace_access_grants g ON g.organization_id=o.id AND g.workspace_id=w.id AND g.user_id=m.user_id
           WHERE o.id=$1 AND o.status='ACTIVE' AND m.status='ACTIVE' AND w.status='ACTIVE'
             AND (m.role IN ('OWNER','ADMIN') OR g.status='ACTIVE')
           ORDER BY w.is_default DESC,w.created_at,w.id"#)
        .bind(&org).bind(&claims.sub).fetch_all(state.store.pool()).await;
    match rows{
        Ok(rows)=>{
            let values=rows.into_iter().map(|(id,organization_id,name,slug,status,is_default,revision,created_by,created_at,updated_at)|serde_json::json!({"id":id,"organizationId":organization_id,"name":name,"slug":slug,"status":status,"isDefault":is_default,"revision":revision,"createdBy":created_by,"createdAt":created_at,"updatedAt":updated_at})).collect::<Vec<_>>();
            Json(serde_json::json!({"data":values})).into_response()
        },Err(_)=>db_error()
    }
}

#[derive(Deserialize)]
pub struct WorkspaceCreate { pub name:String, pub slug:String, pub reason:String }
pub async fn create(State(state):State<AppState>,headers:HeaderMap,Path(org):Path<String>,Json(input):Json<WorkspaceCreate>)->Response{
    let actor=match require_org_admin(&state,&headers,&org).await{Ok(v)=>v,Err(r)=>return r};
    if input.reason.trim().is_empty(){return error(StatusCode::BAD_REQUEST,"invalid_request","reason is required");}
    let id=match WorkspaceId::new(uuid::Uuid::new_v4().to_string()){Ok(v)=>v,Err(_)=>return db_error()};
    let org_id=match OrganizationId::new(org.clone()){Ok(v)=>v,Err(_)=>return not_found()};
    let w=match Workspace::new(id,org_id,input.name,input.slug,actor,Utc::now()){Ok(v)=>v,Err(_)=>return error(StatusCode::BAD_REQUEST,"invalid_workspace","Workspace name or slug is invalid")};
    match state.store.create_workspace(&w,&input.reason).await{
        Ok(())=>(StatusCode::CREATED,Json(serde_json::json!({"data":workspace_json(&w)}))).into_response(),
        Err(_)=>error(StatusCode::CONFLICT,"workspace_create_rejected","Workspace creation was rejected")
    }
}

pub async fn get(State(state):State<AppState>,headers:HeaderMap,Path((org,workspace)):Path<(String,String)>)->Response{
    let principal=match principal(&state,&headers).await{Ok(v)=>v,Err(r)=>return r};
    if principal.organization_id!=org||principal.workspace_id!=workspace{return not_found();}
    match state.store.workspace(&org,&workspace).await{
        Ok(Some(w))=>Json(serde_json::json!({"data":workspace_json(&w)})).into_response(),
        Ok(None)=>not_found(),Err(_)=>db_error()
    }
}

#[derive(Deserialize)]
pub struct WorkspaceUpdate { pub expected_revision:i64, pub name:Option<String>, pub slug:Option<String>, pub reason:String }
pub async fn update(State(state):State<AppState>,headers:HeaderMap,Path((org,workspace)):Path<(String,String)>,Json(input):Json<WorkspaceUpdate>)->Response{
    let actor=match require_org_admin(&state,&headers,&org).await{Ok(v)=>v,Err(r)=>return r};
    if input.reason.trim().is_empty(){return error(StatusCode::BAD_REQUEST,"invalid_request","reason is required");}
    let mut tx=match state.store.pool().begin().await{Ok(v)=>v,Err(_)=>return db_error()};
    let old=sqlx::query_as::<_,(String,String,bool,i64)>("SELECT name,slug,is_default,revision FROM workspaces WHERE organization_id=$1 AND id=$2 AND status='ACTIVE' FOR UPDATE")
        .bind(&org).bind(&workspace).fetch_optional(&mut *tx).await;
    let Some((old_name,old_slug,is_default,revision))= (match old{Ok(v)=>v,Err(_)=>return db_error()}) else{return not_found()};
    if revision!=input.expected_revision{return error(StatusCode::CONFLICT,"revision_conflict","Workspace revision is stale");}
    let name=input.name.as_deref().unwrap_or(&old_name).trim();
    let slug=input.slug.as_deref().unwrap_or(&old_slug).trim();
    if name.is_empty()||name.len()>120||slug.is_empty()||slug.len()>63||!slug.bytes().all(|b|b.is_ascii_lowercase()||b.is_ascii_digit()||b==b'-')||!slug.as_bytes()[0].is_ascii_alphanumeric()||slug.ends_with('-'){
        return error(StatusCode::BAD_REQUEST,"invalid_workspace","Workspace name or slug is invalid");
    }
    let next=revision+1;
    let changed=sqlx::query("UPDATE workspaces SET name=$3,slug=$4,revision=$5 WHERE organization_id=$1 AND id=$2 AND revision=$6")
        .bind(&org).bind(&workspace).bind(name).bind(slug).bind(next).bind(revision).execute(&mut *tx).await;
    if changed.is_err(){return error(StatusCode::CONFLICT,"workspace_update_rejected","Workspace update failed");}
    let event=sqlx::query("INSERT INTO workspace_events(workspace_id,organization_id,actor_user_id,revision,event_type,previous_name,next_name,previous_status,next_status,previous_is_default,next_is_default,reason) VALUES($1,$2,$3,$4,'RENAMED',$5,$6,'ACTIVE','ACTIVE',$7,$7,$8)")
        .bind(&workspace).bind(&org).bind(actor).bind(next).bind(&old_name).bind(name).bind(is_default).bind(input.reason.trim()).execute(&mut *tx).await;
    if event.is_err()||tx.commit().await.is_err(){return db_error();}
    match state.store.workspace(&org,&workspace).await{Ok(Some(w))=>Json(serde_json::json!({"data":workspace_json(&w)})).into_response(),_=>db_error()}
}

#[derive(Deserialize)]
pub struct WorkspaceLifecycle { pub expected_revision:i64, pub reason:String }
pub async fn archive(State(state):State<AppState>,headers:HeaderMap,Path((org,workspace)):Path<(String,String)>,Json(input):Json<WorkspaceLifecycle>)->Response{
    let actor=match require_org_admin(&state,&headers,&org).await{Ok(v)=>v,Err(r)=>return r};
    if input.reason.trim().is_empty(){return error(StatusCode::BAD_REQUEST,"invalid_request","reason is required");}
    let mut tx=match state.store.pool().begin().await{Ok(v)=>v,Err(_)=>return db_error()};
    let old=sqlx::query_as::<_,(String,i64,bool)>("SELECT name,revision,is_default FROM workspaces WHERE organization_id=$1 AND id=$2 AND status='ACTIVE' FOR UPDATE").bind(&org).bind(&workspace).fetch_optional(&mut *tx).await;
    let Some((name,revision,is_default))=(match old{Ok(v)=>v,Err(_)=>return db_error()}) else{return not_found()};
    if revision!=input.expected_revision{return error(StatusCode::CONFLICT,"revision_conflict","Workspace revision is stale");}
    if is_default{return error(StatusCode::CONFLICT,"default_workspace_required","Set another workspace as default before archiving this workspace");}
    let assigned:i64=match sqlx::query_scalar("SELECT count(*) FROM workspace_account_assignments WHERE organization_id=$1 AND workspace_id=$2").bind(&org).bind(&workspace).fetch_one(&mut *tx).await{Ok(v)=>v,Err(_)=>return db_error()};
    if assigned>0{return error(StatusCode::CONFLICT,"workspace_not_empty","Move all Quant account assignments before archiving this workspace");}
    let next=revision+1;
    if sqlx::query("UPDATE workspaces SET status='ARCHIVED',revision=$3 WHERE organization_id=$1 AND id=$2 AND revision=$4").bind(&org).bind(&workspace).bind(next).bind(revision).execute(&mut *tx).await.is_err(){return db_error();}
    if sqlx::query("INSERT INTO workspace_events(workspace_id,organization_id,actor_user_id,revision,event_type,previous_name,next_name,previous_status,next_status,previous_is_default,next_is_default,reason) VALUES($1,$2,$3,$4,'ARCHIVED',$5,$5,'ACTIVE','ARCHIVED',FALSE,FALSE,$6)").bind(&workspace).bind(&org).bind(actor).bind(next).bind(name).bind(input.reason.trim()).execute(&mut *tx).await.is_err()||tx.commit().await.is_err(){return db_error();}
    Json(serde_json::json!({"data":{"id":workspace,"status":"ARCHIVED","revision":next}})).into_response()
}

pub async fn set_default(State(state):State<AppState>,headers:HeaderMap,Path((org,workspace)):Path<(String,String)>,Json(input):Json<WorkspaceLifecycle>)->Response{
    let actor=match require_org_admin(&state,&headers,&org).await{Ok(v)=>v,Err(r)=>return r};
    if input.reason.trim().is_empty(){return error(StatusCode::BAD_REQUEST,"invalid_request","reason is required");}
    let mut tx=match state.store.pool().begin().await{Ok(v)=>v,Err(_)=>return db_error()};
    let target=sqlx::query_as::<_,(String,i64,bool)>("SELECT name,revision,is_default FROM workspaces WHERE organization_id=$1 AND id=$2 AND status='ACTIVE' FOR UPDATE").bind(&org).bind(&workspace).fetch_optional(&mut *tx).await;
    let Some((name,revision,is_default))=(match target{Ok(v)=>v,Err(_)=>return db_error()}) else{return not_found()};
    if revision!=input.expected_revision{return error(StatusCode::CONFLICT,"revision_conflict","Workspace revision is stale");}
    if is_default{let _=tx.rollback().await;return Json(serde_json::json!({"data":{"id":workspace,"isDefault":true,"revision":revision}})).into_response();}
    let current=sqlx::query_as::<_,(String,String,i64)>("SELECT id,name,revision FROM workspaces WHERE organization_id=$1 AND is_default=TRUE AND status='ACTIVE' FOR UPDATE").bind(&org).fetch_one(&mut *tx).await;
    let (old_id,old_name,old_rev)=match current{Ok(v)=>v,Err(_)=>return db_error()};
    let old_next=old_rev+1;let new_next=revision+1;
    if sqlx::query("UPDATE workspaces SET is_default=FALSE,revision=$2 WHERE id=$1 AND revision=$3").bind(&old_id).bind(old_next).bind(old_rev).execute(&mut *tx).await.is_err(){return db_error();}
    if sqlx::query("UPDATE workspaces SET is_default=TRUE,revision=$2 WHERE id=$1 AND revision=$3").bind(&workspace).bind(new_next).bind(revision).execute(&mut *tx).await.is_err(){return db_error();}
    let e1=sqlx::query("INSERT INTO workspace_events(workspace_id,organization_id,actor_user_id,revision,event_type,previous_name,next_name,previous_status,next_status,previous_is_default,next_is_default,reason) VALUES($1,$2,$3,$4,'DEFAULT_CHANGED',$5,$5,'ACTIVE','ACTIVE',TRUE,FALSE,$6)").bind(&old_id).bind(&org).bind(&actor).bind(old_next).bind(old_name).bind(input.reason.trim()).execute(&mut *tx).await;
    let e2=sqlx::query("INSERT INTO workspace_events(workspace_id,organization_id,actor_user_id,revision,event_type,previous_name,next_name,previous_status,next_status,previous_is_default,next_is_default,reason) VALUES($1,$2,$3,$4,'DEFAULT_CHANGED',$5,$5,'ACTIVE','ACTIVE',FALSE,TRUE,$6)").bind(&workspace).bind(&org).bind(&actor).bind(new_next).bind(name).bind(input.reason.trim()).execute(&mut *tx).await;
    if e1.is_err()||e2.is_err()||tx.commit().await.is_err(){return db_error();}
    Json(serde_json::json!({"data":{"id":workspace,"isDefault":true,"revision":new_next}})).into_response()
}

#[derive(Deserialize)]
pub struct GrantCreate { pub user_id:String, pub role:String, pub reason:String }
#[derive(Deserialize)]
pub struct GrantUpdate { pub expected_revision:i64, pub role:String, pub status:String, pub reason:String }
fn valid_workspace_role(role:&str)->bool{matches!(role,"WORKSPACE_ADMIN"|"TRADER"|"RISK_MANAGER"|"VIEWER")}
pub async fn list_grants(State(state):State<AppState>,headers:HeaderMap,Path((org,workspace)):Path<(String,String)>)->Response{
    if let Err(r)=require_org_admin(&state,&headers,&org).await{return r;}
    let rows=sqlx::query_as::<_,(String,String,String,i64,String,String,String,chrono::DateTime<chrono::Utc>,chrono::DateTime<chrono::Utc>)>(
        "SELECT user_id,role,status,revision,granted_by,updated_by,reason,created_at,updated_at FROM workspace_access_grants WHERE organization_id=$1 AND workspace_id=$2 ORDER BY created_at,user_id")
        .bind(&org).bind(&workspace).fetch_all(state.store.pool()).await;
    match rows{
        Ok(rows)=>Json(serde_json::json!({"data":rows.into_iter().map(|(user,role,status,revision,granted_by,updated_by,reason,created,updated)|serde_json::json!({"userId":user,"role":role,"status":status,"revision":revision,"grantedBy":granted_by,"updatedBy":updated_by,"reason":reason,"createdAt":created,"updatedAt":updated})).collect::<Vec<_>>()})).into_response(),
        Err(_)=>db_error()
    }
}
pub async fn create_grant(State(state):State<AppState>,headers:HeaderMap,Path((org,workspace)):Path<(String,String)>,Json(input):Json<GrantCreate>)->Response{
    let actor=match require_org_admin(&state,&headers,&org).await{Ok(v)=>v,Err(r)=>return r};
    if input.user_id.trim().is_empty()||input.reason.trim().is_empty()||!valid_workspace_role(&input.role){return error(StatusCode::BAD_REQUEST,"invalid_grant","A valid user_id, workspace role and reason are required");}
    let mut tx=match state.store.pool().begin().await{Ok(v)=>v,Err(_)=>return db_error()};
    let inserted=sqlx::query("INSERT INTO workspace_access_grants(organization_id,workspace_id,user_id,role,status,revision,granted_by,updated_by,reason) VALUES($1,$2,$3,$4,'ACTIVE',1,$5,$5,$6)")
        .bind(&org).bind(&workspace).bind(input.user_id.trim()).bind(&input.role).bind(&actor).bind(input.reason.trim()).execute(&mut *tx).await;
    if inserted.is_err(){return error(StatusCode::CONFLICT,"grant_rejected","Grant target is unavailable, already granted, or not eligible");}
    if tx.commit().await.is_err(){return db_error();}
    (StatusCode::CREATED,Json(serde_json::json!({"data":{"userId":input.user_id.trim(),"role":input.role,"status":"ACTIVE","revision":1}}))).into_response()
}
pub async fn update_grant(State(state):State<AppState>,headers:HeaderMap,Path((org,workspace,user)):Path<(String,String,String)>,Json(input):Json<GrantUpdate>)->Response{
    let actor=match require_org_admin(&state,&headers,&org).await{Ok(v)=>v,Err(r)=>return r};
    if input.reason.trim().is_empty()||!valid_workspace_role(&input.role)||!matches!(input.status.as_str(),"ACTIVE"|"REVOKED"){return error(StatusCode::BAD_REQUEST,"invalid_grant","A valid role, status and reason are required");}
    let mut tx=match state.store.pool().begin().await{Ok(v)=>v,Err(_)=>return db_error()};
    let old=sqlx::query_as::<_,(String,i64)>("SELECT status,revision FROM workspace_access_grants WHERE organization_id=$1 AND workspace_id=$2 AND user_id=$3 FOR UPDATE")
        .bind(&org).bind(&workspace).bind(&user).fetch_optional(&mut *tx).await;
    let Some((old_status,revision))=(match old{Ok(v)=>v,Err(_)=>return db_error()}) else{return not_found()};
    if revision!=input.expected_revision{return error(StatusCode::CONFLICT,"revision_conflict","Workspace grant revision is stale");}
    if old_status=="REVOKED"&&input.status=="REVOKED"{return error(StatusCode::CONFLICT,"grant_already_revoked","Grant is already revoked");}
    let next=revision+1;
    let updated=sqlx::query("UPDATE workspace_access_grants SET role=$4,status=$5,revision=$6,updated_by=$7,reason=$8 WHERE organization_id=$1 AND workspace_id=$2 AND user_id=$3 AND revision=$9")
        .bind(&org).bind(&workspace).bind(&user).bind(&input.role).bind(&input.status).bind(next).bind(&actor).bind(input.reason.trim()).bind(revision).execute(&mut *tx).await;
    if updated.is_err(){return error(StatusCode::CONFLICT,"grant_update_rejected","Grant update was rejected by current membership or workspace policy");}
    if tx.commit().await.is_err(){return db_error();}
    Json(serde_json::json!({"data":{"userId":user,"role":input.role,"status":input.status,"revision":next}})).into_response()
}
