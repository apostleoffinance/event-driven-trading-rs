#![allow(clippy::unwrap_used)]

use chrono::Utc;
use domain::{
    Account, AccountEnvironment, AccountId, AccountState, AccountType, EntitlementStatus,
    MembershipStatus, Organization, OrganizationId, OrganizationRole, VenueId,
};
use persistence::{migrate, TradingStore};
use rust_decimal::Decimal;
use sqlx::PgPool;

async fn fixture(pool: &PgPool, user: &str, account: &str) {
    sqlx::query("INSERT INTO identity_users(id,email,display_name,password_hash) VALUES($1,$2,$1,'test-only')")
        .bind(user).bind(format!("{user}@example.test")).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO venues(id,venue_type,name,created_at) VALUES($1,'simulated','Test venue',NOW()) ON CONFLICT(id) DO NOTHING")
        .bind(format!("venue-{account}")).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO accounts(id,account_type,status,venue_id,starting_capital,created_at,environment) VALUES($1,'simulated','active',$2,100,NOW(),'PAPER')")
        .bind(account).bind(format!("venue-{account}")).execute(pool).await.unwrap();
}

fn organization(id: &str, name: &str) -> Organization {
    Organization::new(OrganizationId::new(id).unwrap(), name, Utc::now()).unwrap()
}

async fn owner_context(pool: &PgPool, org: &str, owner: &str) -> TradingStore {
    sqlx::query("INSERT INTO identity_users(id,email,display_name,password_hash) VALUES($1,$2,$1,'test-only')")
        .bind(owner).bind(format!("{owner}@example.test")).execute(pool).await.unwrap();
    let store = TradingStore::new(pool.clone());
    store
        .create_organization(&organization(org, "Paper account test"), owner)
        .await
        .unwrap();
    store
}

fn paper_account(id: &str) -> (Account, AccountState) {
    let now = Utc::now();
    let id = AccountId::new(id).unwrap();
    let account = Account::new(
        id.clone(),
        AccountType::Simulated,
        VenueId::new("simulated").unwrap(),
        AccountEnvironment::Paper,
        now,
    );
    let state = AccountState::new(id, Decimal::from(100), now).unwrap();
    (account, state)
}

#[sqlx::test]
async fn tenant_entitlements_isolate_accounts_and_legacy_grants(pool: PgPool) {
    migrate(&pool).await.unwrap();
    fixture(&pool, "user-a", "account-a").await;
    fixture(&pool, "user-b", "account-b").await;
    let store = TradingStore::new(pool.clone());
    store
        .create_organization(&organization("org-a", "Organization A"), "user-a")
        .await
        .unwrap();
    store
        .create_organization(&organization("org-b", "Organization B"), "user-b")
        .await
        .unwrap();

    // An old dashboard grant is deliberately not translated into tenant access.
    sqlx::query(
        "INSERT INTO dashboard_account_access(owner_id,account_id) VALUES('user-a','account-b')",
    )
    .execute(&pool)
    .await
    .unwrap();
    let legacy_count: i64 = sqlx::query_scalar("SELECT count(*) FROM dashboard_account_access WHERE owner_id='user-a' AND account_id='account-b'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(legacy_count, 1);
    let unassigned_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM organization_account_entitlements WHERE account_id='account-b'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(unassigned_count, 0);

    store
        .assign_account_entitlement("org-a", "account-a", "user-a", 1, 0, "approved assignment")
        .await
        .unwrap();
    store
        .assign_account_entitlement("org-b", "account-b", "user-b", 1, 0, "approved assignment")
        .await
        .unwrap();
    let tenant_count: i64 = sqlx::query_scalar("SELECT count(*) FROM organization_account_entitlements e JOIN organization_memberships m ON m.organization_id=e.organization_id JOIN organizations o ON o.id=e.organization_id WHERE m.user_id=$1 AND m.status='ACTIVE' AND o.status='ACTIVE' AND e.organization_id=$2 AND e.status='ACTIVE' AND e.account_id=$3")
        .bind("user-a").bind("org-a").bind("account-a").fetch_one(&pool).await.unwrap();
    assert_eq!(tenant_count, 1);
    let cross_tenant_count: i64 = sqlx::query_scalar("SELECT count(*) FROM organization_account_entitlements e JOIN organization_memberships m ON m.organization_id=e.organization_id WHERE m.user_id=$1 AND m.status='ACTIVE' AND e.organization_id=$2 AND e.status='ACTIVE' AND e.account_id=$3")
        .bind("user-a").bind("org-a").bind("account-b").fetch_one(&pool).await.unwrap();
    assert_eq!(cross_tenant_count, 0);

    store
        .set_account_entitlement_status(
            "org-a",
            "account-a",
            "user-a",
            1,
            1,
            EntitlementStatus::Suspended,
            "suspend access",
        )
        .await
        .unwrap();
    assert!(store
        .set_account_entitlement_status(
            "org-a",
            "account-a",
            "user-a",
            1,
            1,
            EntitlementStatus::Active,
            "stale revision"
        )
        .await
        .is_err());
    let suspended_count: i64 = sqlx::query_scalar("SELECT count(*) FROM organization_account_entitlements WHERE organization_id='org-a' AND account_id='account-a' AND status='ACTIVE'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(suspended_count, 0);
    let history_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM organization_account_entitlement_events WHERE account_id='account-a'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(history_count, 2);
}

#[sqlx::test]
async fn membership_owner_transfer_and_stale_revision_are_enforced(pool: PgPool) {
    migrate(&pool).await.unwrap();
    fixture(&pool, "owner-a", "unused-a").await;
    fixture(&pool, "member-b", "unused-b").await;
    let store = TradingStore::new(pool.clone());
    store
        .create_organization(&organization("org-transfer", "Transfer"), "owner-a")
        .await
        .unwrap();
    store
        .create_organization_membership(
            "org-transfer",
            "member-b",
            "owner-a",
            OrganizationRole::Trader,
            "invite",
        )
        .await
        .unwrap();
    assert!(store
        .revise_organization_membership(
            "org-transfer",
            "owner-a",
            "owner-a",
            1,
            OrganizationRole::Admin,
            MembershipStatus::Active,
            "implicit transfer"
        )
        .await
        .is_err());
    assert!(store
        .revise_organization_membership(
            "org-transfer",
            "member-b",
            "owner-a",
            9,
            OrganizationRole::Viewer,
            MembershipStatus::Active,
            "stale"
        )
        .await
        .is_err());
    store
        .transfer_organization_owner(
            "org-transfer",
            "owner-a",
            "member-b",
            1,
            1,
            "approved owner transfer",
        )
        .await
        .unwrap();
    let former_owner = store
        .organization_membership("org-transfer", "owner-a")
        .await
        .unwrap()
        .unwrap();
    let new_owner = store
        .organization_membership("org-transfer", "member-b")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(former_owner.0, OrganizationRole::Admin);
    assert_eq!(new_owner.0, OrganizationRole::Owner);
    assert!(store
        .revise_organization_membership(
            "org-transfer",
            "member-b",
            "member-b",
            new_owner.2,
            OrganizationRole::Admin,
            MembershipStatus::Active,
            "self elevation"
        )
        .await
        .is_err());
    let event_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM organization_membership_events WHERE organization_id='org-transfer'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(event_count, 4);
}

#[sqlx::test]
async fn paper_account_and_entitlement_commit_atomically(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let store = owner_context(&pool, "org-paper", "paper-owner").await;
    let (account, state) = paper_account("paper-atomic-success");
    assert_eq!(
        store
            .create_paper_account_for_owner(
                "org-paper",
                "paper-owner",
                &account,
                Decimal::from(100),
                "Atomic",
                &state
            )
            .await
            .unwrap(),
        1
    );
    let (accounts, states, entitlements, events): (i64,i64,i64,i64) = sqlx::query_as("SELECT (SELECT count(*) FROM accounts WHERE id='paper-atomic-success'),(SELECT count(*) FROM account_states WHERE account_id='paper-atomic-success'),(SELECT count(*) FROM organization_account_entitlements WHERE account_id='paper-atomic-success' AND organization_id='org-paper' AND status='ACTIVE' AND revision=1 AND assigned_by='paper-owner'),(SELECT count(*) FROM organization_account_entitlement_events WHERE account_id='paper-atomic-success' AND account_revision=1 AND entitlement_revision=1 AND actor_user_id='paper-owner')")
        .fetch_one(&pool).await.unwrap();
    assert_eq!((accounts, states, entitlements, events), (1, 1, 1, 1));
}

#[sqlx::test]
async fn paper_account_creation_rolls_back_account_insert_failure(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let store = owner_context(&pool, "org-paper-fail", "paper-owner-fail").await;
    sqlx::raw_sql("CREATE FUNCTION qos020_fail_account_insert() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected account insert failure'; END $$; CREATE TRIGGER qos020_fail_account_insert BEFORE INSERT ON accounts FOR EACH ROW EXECUTE FUNCTION qos020_fail_account_insert();")
        .execute(&pool).await.unwrap();
    let (account, state) = paper_account("paper-atomic-account-fail");
    assert!(store
        .create_paper_account_for_owner(
            "org-paper-fail",
            "paper-owner-fail",
            &account,
            Decimal::from(100),
            "Failure",
            &state
        )
        .await
        .is_err());
    sqlx::raw_sql("DROP TRIGGER qos020_fail_account_insert ON accounts; DROP FUNCTION qos020_fail_account_insert();").execute(&pool).await.unwrap();
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM accounts WHERE id='paper-atomic-account-fail'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 0);
}

#[sqlx::test]
async fn paper_account_creation_rolls_back_entitlement_insert_failure(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let store = owner_context(&pool, "org-entitlement-fail", "entitlement-owner").await;
    sqlx::raw_sql("CREATE FUNCTION qos020_fail_entitlement_insert() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected entitlement insert failure'; END $$; CREATE TRIGGER qos020_fail_entitlement_insert BEFORE INSERT ON organization_account_entitlements FOR EACH ROW EXECUTE FUNCTION qos020_fail_entitlement_insert();")
        .execute(&pool).await.unwrap();
    let (account, state) = paper_account("paper-atomic-entitlement-fail");
    assert!(store
        .create_paper_account_for_owner(
            "org-entitlement-fail",
            "entitlement-owner",
            &account,
            Decimal::from(100),
            "Failure",
            &state
        )
        .await
        .is_err());
    sqlx::raw_sql("DROP TRIGGER qos020_fail_entitlement_insert ON organization_account_entitlements; DROP FUNCTION qos020_fail_entitlement_insert();").execute(&pool).await.unwrap();
    let (accounts,states,entitlements,venues):(i64,i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM accounts WHERE id='paper-atomic-entitlement-fail'),(SELECT count(*) FROM account_states WHERE account_id='paper-atomic-entitlement-fail'),(SELECT count(*) FROM organization_account_entitlements WHERE account_id='paper-atomic-entitlement-fail'),(SELECT count(*) FROM venues WHERE id='simulated')").fetch_one(&pool).await.unwrap();
    assert_eq!((accounts, states, entitlements, venues), (0, 0, 0, 0));
}

#[sqlx::test]
async fn paper_account_creation_rolls_back_audit_failure(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let store = owner_context(&pool, "org-audit-fail", "audit-owner").await;
    sqlx::raw_sql("CREATE FUNCTION qos020_fail_entitlement_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected entitlement audit failure'; END $$; CREATE TRIGGER qos020_fail_entitlement_audit BEFORE INSERT ON organization_account_entitlement_events FOR EACH ROW EXECUTE FUNCTION qos020_fail_entitlement_audit();")
        .execute(&pool).await.unwrap();
    let (account, state) = paper_account("paper-atomic-audit-fail");
    assert!(store
        .create_paper_account_for_owner(
            "org-audit-fail",
            "audit-owner",
            &account,
            Decimal::from(100),
            "Failure",
            &state
        )
        .await
        .is_err());
    sqlx::raw_sql("DROP TRIGGER qos020_fail_entitlement_audit ON organization_account_entitlement_events; DROP FUNCTION qos020_fail_entitlement_audit();").execute(&pool).await.unwrap();
    let (accounts,states,entitlements,events):(i64,i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM accounts WHERE id='paper-atomic-audit-fail'),(SELECT count(*) FROM account_states WHERE account_id='paper-atomic-audit-fail'),(SELECT count(*) FROM organization_account_entitlements WHERE account_id='paper-atomic-audit-fail'),(SELECT count(*) FROM organization_account_entitlement_events WHERE account_id='paper-atomic-audit-fail')").fetch_one(&pool).await.unwrap();
    assert_eq!((accounts, states, entitlements, events), (0, 0, 0, 0));
}

#[sqlx::test]
async fn paper_account_creation_rolls_back_duplicate_entitlement_conflict(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let store = owner_context(&pool, "org-duplicate", "duplicate-owner").await;
    sqlx::raw_sql("CREATE FUNCTION qos020_duplicate_entitlement() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN INSERT INTO organization_account_entitlements(account_id,organization_id,status,revision,assigned_by,updated_by,reason) VALUES(NEW.account_id,NEW.organization_id,'ACTIVE',1,NEW.assigned_by,NEW.updated_by,NEW.reason); RETURN NEW; END $$; CREATE TRIGGER qos020_duplicate_entitlement BEFORE INSERT ON organization_account_entitlements FOR EACH ROW EXECUTE FUNCTION qos020_duplicate_entitlement();")
        .execute(&pool).await.unwrap();
    let (account, state) = paper_account("paper-atomic-duplicate-entitlement");
    assert!(store
        .create_paper_account_for_owner(
            "org-duplicate",
            "duplicate-owner",
            &account,
            Decimal::from(100),
            "Failure",
            &state
        )
        .await
        .is_err());
    sqlx::raw_sql("DROP TRIGGER qos020_duplicate_entitlement ON organization_account_entitlements; DROP FUNCTION qos020_duplicate_entitlement();").execute(&pool).await.unwrap();
    let (accounts,entitlements,events):(i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM accounts WHERE id='paper-atomic-duplicate-entitlement'),(SELECT count(*) FROM organization_account_entitlements WHERE account_id='paper-atomic-duplicate-entitlement'),(SELECT count(*) FROM organization_account_entitlement_events WHERE account_id='paper-atomic-duplicate-entitlement')").fetch_one(&pool).await.unwrap();
    assert_eq!((accounts, entitlements, events), (0, 0, 0));
}

#[sqlx::test]
async fn r2_schema_triggers_foreign_keys_and_account_revision_are_present(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let tables:i64=sqlx::query_scalar("SELECT count(*) FROM information_schema.tables WHERE table_schema='public' AND table_name = ANY(ARRAY['organizations','organization_events','organization_memberships','organization_membership_events','organization_account_entitlements','organization_account_entitlement_events'])")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(tables, 6);
    let indexes:i64=sqlx::query_scalar("SELECT count(*) FROM pg_indexes WHERE schemaname='public' AND indexname = ANY(ARRAY['organization_memberships_user_active_idx','organization_memberships_org_role_idx','organization_membership_events_history_idx','organization_events_history_idx','organization_account_entitlements_active_idx','organization_account_entitlements_history_idx','organization_account_entitlement_events_history_idx'])")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(indexes, 7);
    let triggers:i64=sqlx::query_scalar("SELECT count(*) FROM pg_trigger WHERE NOT tgisinternal AND tgname = ANY(ARRAY['accounts_revision_monotonic','organization_memberships_last_owner','organization_events_no_delete','organization_membership_events_no_delete','organization_account_entitlement_events_no_delete'])")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(triggers, 5);
    let constraints:i64=sqlx::query_scalar("SELECT count(*) FROM pg_constraint WHERE contype='f' AND conname = ANY(ARRAY['organizations_created_by_fkey','organization_memberships_organization_id_fkey','organization_memberships_user_id_fkey','organization_account_entitlements_account_id_fkey','organization_account_entitlements_organization_id_fkey'])")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(constraints, 5);
    let store = owner_context(&pool, "org-trigger", "trigger-owner").await;
    let last_owner=sqlx::query("UPDATE organization_memberships SET role='ADMIN',status='REVOKED',revision=revision+1 WHERE organization_id='org-trigger' AND user_id='trigger-owner'")
        .execute(&pool).await;
    assert!(last_owner.is_err());
    let history_update=sqlx::query("UPDATE organization_membership_events SET reason='altered' WHERE organization_id='org-trigger'")
        .execute(&pool).await;
    assert!(history_update.is_err());
    let (account, state) = paper_account("account-revision-test");
    store
        .create_paper_account_for_owner(
            "org-trigger",
            "trigger-owner",
            &account,
            Decimal::from(100),
            "Revision",
            &state,
        )
        .await
        .unwrap();
    sqlx::query("UPDATE accounts SET label='revision changed' WHERE id='account-revision-test'")
        .execute(&pool)
        .await
        .unwrap();
    let revision: i64 = sqlx::query_scalar(
        "SELECT account_revision FROM accounts WHERE id='account-revision-test'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(revision, 2);
}
