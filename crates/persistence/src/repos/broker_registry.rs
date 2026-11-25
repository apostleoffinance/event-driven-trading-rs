use sqlx::PgPool;

use crate::error::{PersistenceError, PersistenceResult};

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct BrokerCatalogEntry {
    pub broker_id: String,
    pub broker_name: String,
    pub broker_display_name: String,
    pub server_id: String,
    pub server_display_name: String,
    pub provider_type: String,
    pub environment: String,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct BrokerBindingView {
    pub binding_id: String,
    pub quant_account_id: String,
    pub external_broker_account_id: String,
    pub broker_id: String,
    pub broker_display_name: String,
    pub broker_server_id: String,
    pub server_display_name: String,
    pub binding_status: String,
    pub verification_status: String,
    pub revision: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokerBindingTransition {
    Suspended,
    Revoked,
}

impl BrokerBindingTransition {
    fn as_db(self) -> &'static str {
        match self {
            Self::Suspended => "SUSPENDED",
            Self::Revoked => "REVOKED",
        }
    }
}

pub async fn list_catalog(pool: &PgPool) -> PersistenceResult<Vec<BrokerCatalogEntry>> {
    Ok(sqlx::query_as::<_, BrokerCatalogEntry>(
        r#"SELECT b.id AS broker_id,b.canonical_name AS broker_name,
                  b.display_name AS broker_display_name,s.id AS server_id,
                  s.display_name AS server_display_name,s.provider_type,s.environment
             FROM brokers b JOIN broker_servers s ON s.broker_id=b.id
            WHERE b.status='ACTIVE' AND s.status='ACTIVE'
            ORDER BY b.canonical_name,s.canonical_identifier"#,
    )
    .fetch_all(pool)
    .await?)
}

pub async fn list_bindings(
    pool: &PgPool,
    organization: &str,
) -> PersistenceResult<Vec<BrokerBindingView>> {
    Ok(sqlx::query_as::<_, BrokerBindingView>(
        r#"SELECT x.id AS binding_id,x.quant_account_id,x.external_broker_account_id,
                  b.id AS broker_id,b.display_name AS broker_display_name,
                  s.id AS broker_server_id,s.display_name AS server_display_name,
                  x.status AS binding_status,a.verification_status,x.revision
             FROM trading_account_broker_bindings x
             JOIN external_broker_accounts a ON a.id=x.external_broker_account_id
             JOIN brokers b ON b.id=a.broker_id
             JOIN broker_servers s ON s.id=a.broker_server_id
            WHERE x.organization_id=$1
            ORDER BY x.created_at DESC"#,
    )
    .bind(organization)
    .fetch_all(pool)
    .await?)
}

#[allow(clippy::too_many_arguments)]
pub async fn register_pending_binding(
    pool: &PgPool,
    organization: &str,
    actor: &str,
    quant_account: &str,
    broker: &str,
    server: &str,
    account_id: &str,
    binding_id: &str,
    normalization_version: i16,
    key_version: &str,
    lookup_hmacs: &[(String, [u8; 32])],
    reason: &str,
) -> PersistenceResult<()> {
    let mut tx = pool.begin().await?;

    let owner = sqlx::query_scalar::<_, i32>(
        "SELECT 1 FROM organizations o JOIN organization_memberships m ON m.organization_id=o.id WHERE o.id=$1 AND o.status='ACTIVE' AND m.user_id=$2 AND m.role='OWNER' AND m.status='ACTIVE' FOR SHARE OF o,m",
    )
    .bind(organization)
    .bind(actor)
    .fetch_optional(&mut *tx)
    .await?;
    if owner.is_none() {
        return Err(PersistenceError::NotFound(
            "registration unavailable".into(),
        ));
    }

    let entitlement = sqlx::query_as::<_, (String, String)>(
        r#"SELECT e.status,w.status
             FROM organization_account_entitlements e
             JOIN workspace_account_assignments waa
               ON waa.organization_id=e.organization_id AND waa.account_id=e.account_id
             JOIN workspaces w
               ON w.organization_id=waa.organization_id AND w.id=waa.workspace_id
            WHERE e.organization_id=$1 AND e.account_id=$2
            FOR UPDATE OF e,w,waa"#,
    )
    .bind(organization)
    .bind(quant_account)
    .fetch_optional(&mut *tx)
    .await?;
    if !matches!(entitlement, Some((ref entitlement_status, ref workspace_status)) if entitlement_status == "ACTIVE" && workspace_status == "ACTIVE")
    {
        return Err(PersistenceError::NotFound(
            "registration unavailable".into(),
        ));
    }

    let trusted_catalog_entry = sqlx::query_scalar::<_, i32>(
        "SELECT 1 FROM brokers b JOIN broker_servers s ON s.broker_id=b.id WHERE b.id=$1 AND s.id=$2 AND b.status='ACTIVE' AND s.status='ACTIVE' FOR SHARE OF b,s",
    )
    .bind(broker)
    .bind(server)
    .fetch_optional(&mut *tx)
    .await?;
    if trusted_catalog_entry.is_none() {
        return Err(PersistenceError::NotFound(
            "registration unavailable".into(),
        ));
    }

    if lookup_hmacs.is_empty()
        || !lookup_hmacs
            .iter()
            .any(|(version, _)| version == key_version)
    {
        return Err(PersistenceError::Other(
            "broker identity protection keyset is invalid".into(),
        ));
    }
    // Look up every retained key version so overlapping HMAC rotations cannot
    // create a second row for an identity already indexed by an older key.
    for (version, token) in lookup_hmacs {
        let duplicate: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM external_broker_account_identifier_tokens WHERE broker_server_id=$1 AND key_version=$2 AND normalization_version=$3 AND lookup_hmac=$4)",
        )
        .bind(server)
        .bind(version)
        .bind(normalization_version)
        .bind(token.as_slice())
        .fetch_one(&mut *tx)
        .await?;
        if duplicate {
            return Err(PersistenceError::Other(
                "broker account registration conflict".into(),
            ));
        }
    }
    let primary_token = lookup_hmacs
        .iter()
        .find(|(version, _)| version == key_version)
        .map(|(_, token)| token)
        .ok_or_else(|| PersistenceError::Other("active broker identifier key is missing".into()))?;
    // Do not expose whether a duplicate account is registered to another
    // organization. The API maps all identity conflicts to one response.
    sqlx::query(
        "INSERT INTO external_broker_accounts(id,broker_id,broker_server_id,identifier_normalization_version,identifier_key_version,identifier_lookup_hmac) VALUES($1,$2,$3,$4,$5,$6)",
    )
    .bind(account_id)
    .bind(broker)
    .bind(server)
    .bind(normalization_version)
    .bind(key_version)
    .bind(primary_token.as_slice())
    .execute(&mut *tx)
    .await?;

    for (version, token) in lookup_hmacs {
        sqlx::query("INSERT INTO external_broker_account_identifier_tokens(external_broker_account_id,broker_server_id,key_version,normalization_version,lookup_hmac) VALUES($1,$2,$3,$4,$5)")
            .bind(account_id).bind(server).bind(version).bind(normalization_version).bind(token.as_slice())
            .execute(&mut *tx).await?;
    }

    sqlx::query(
        "INSERT INTO trading_account_broker_bindings(id,organization_id,quant_account_id,external_broker_account_id,created_by,reassignment_reason) VALUES($1,$2,$3,$4,$5,$6)",
    )
    .bind(binding_id)
    .bind(organization)
    .bind(quant_account)
    .bind(account_id)
    .bind(actor)
    .bind(reason)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "INSERT INTO broker_account_verifications(id,external_broker_account_id,decision,revision,reason) VALUES($1,$2,'PENDING_EVIDENCE',1,$3)",
    )
    .bind(format!("verification-{binding_id}"))
    .bind(account_id)
    .bind(reason)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO trading_account_broker_binding_events(binding_id,organization_id,quant_account_id,external_broker_account_id,actor_user_id,revision,next_status,reason) VALUES($1,$2,$3,$4,$5,1,'PENDING_EVIDENCE',$6)",
    )
    .bind(binding_id)
    .bind(organization)
    .bind(quant_account)
    .bind(account_id)
    .bind(actor)
    .bind(reason)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn transition_binding(
    pool: &PgPool,
    organization: &str,
    actor: &str,
    binding_id: &str,
    expected_revision: i64,
    transition: BrokerBindingTransition,
    reason: &str,
) -> PersistenceResult<i64> {
    let mut tx = pool.begin().await?;
    let owner = sqlx::query_scalar::<_, i32>(
        "SELECT 1 FROM organizations o JOIN organization_memberships m ON m.organization_id=o.id WHERE o.id=$1 AND o.status='ACTIVE' AND m.user_id=$2 AND m.role='OWNER' AND m.status='ACTIVE' FOR SHARE OF o,m",
    )
    .bind(organization)
    .bind(actor)
    .fetch_optional(&mut *tx)
    .await?;
    if owner.is_none() {
        return Err(PersistenceError::NotFound("binding unavailable".into()));
    }
    let row = sqlx::query_as::<_, (String, String, i64, String)>(
        "SELECT external_broker_account_id,quant_account_id,revision,status FROM trading_account_broker_bindings WHERE id=$1 AND organization_id=$2 FOR UPDATE",
    )
    .bind(binding_id)
    .bind(organization)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| PersistenceError::NotFound("binding unavailable".into()))?;
    if row.2 != expected_revision || row.3 == "REVOKED" {
        return Err(PersistenceError::Other(
            "stale or terminal broker binding revision".into(),
        ));
    }
    let next_revision = expected_revision + 1;
    let next_status = transition.as_db();
    sqlx::query("UPDATE trading_account_broker_bindings SET status=$3,revision=$4,updated_at=NOW() WHERE id=$1 AND organization_id=$2")
        .bind(binding_id).bind(organization).bind(next_status).bind(next_revision)
        .execute(&mut *tx).await?;
    sqlx::query("UPDATE external_broker_accounts SET status=$2,verification_status=$2,revision=revision+1,updated_at=NOW() WHERE id=$1")
        .bind(&row.0).bind(next_status).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO broker_account_verifications(id,external_broker_account_id,decision,revision,previous_revision,approval_actor_id,approval_authority,approval_reference,reason) VALUES($1,$2,$3,$4,$5,$6,'ORGANIZATION_OWNER_LIFECYCLE','owner-request',$7)")
        .bind(format!("verification-{binding_id}-{next_revision}")).bind(&row.0).bind(next_status)
        .bind(next_revision).bind(expected_revision).bind(actor).bind(reason)
        .execute(&mut *tx).await?;
    sqlx::query("INSERT INTO trading_account_broker_binding_events(binding_id,organization_id,quant_account_id,external_broker_account_id,actor_user_id,revision,previous_status,next_status,reason) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)")
        .bind(binding_id).bind(organization).bind(&row.1).bind(&row.0).bind(actor)
        .bind(next_revision).bind(&row.3).bind(next_status).bind(reason)
        .execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(next_revision)
}
