#![allow(clippy::unwrap_used)]

use chrono::Utc;
use domain::{EntitlementStatus, MembershipStatus, Organization, OrganizationId, OrganizationRole};
use persistence::{migrate, TradingStore};
use sqlx::PgPool;

fn org(id: &str) -> Organization {
    Organization::new(OrganizationId::new(id).unwrap(), id, Utc::now()).unwrap()
}

async fn user(pool: &PgPool, id: &str) {
    sqlx::query("INSERT INTO identity_users(id,email,display_name,password_hash) VALUES($1,$2,$1,'test-only')")
        .bind(id).bind(format!("{id}@example.test")).execute(pool).await.unwrap();
}

async fn account(pool: &PgPool, id: &str) {
    sqlx::query("INSERT INTO venues(id,venue_type,name,created_at) VALUES($1,'simulated',$1,NOW()) ON CONFLICT(id) DO NOTHING")
        .bind(format!("venue-{id}")).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO accounts(id,account_type,status,venue_id,starting_capital,created_at,environment) VALUES($1,'simulated','active',$2,100,NOW(),'PAPER')")
        .bind(id).bind(format!("venue-{id}")).execute(pool).await.unwrap();
}

#[sqlx::test]
async fn duplicate_membership_revision_and_owner_transfer_races_serialize(pool: PgPool) {
    migrate(&pool).await.unwrap();
    for id in ["race-owner", "race-a", "race-b", "race-member"] {
        user(&pool, id).await;
    }
    let store = TradingStore::new(pool.clone());
    store
        .create_organization(&org("org-race"), "race-owner")
        .await
        .unwrap();

    let (m1, m2) = tokio::join!(
        store.create_organization_membership(
            "org-race",
            "race-member",
            "race-owner",
            OrganizationRole::Viewer,
            "race one"
        ),
        store.create_organization_membership(
            "org-race",
            "race-member",
            "race-owner",
            OrganizationRole::Viewer,
            "race two"
        ),
    );
    assert_ne!(m1.is_ok(), m2.is_ok());
    let memberships:i64=sqlx::query_scalar("SELECT count(*) FROM organization_memberships WHERE organization_id='org-race' AND user_id='race-member'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(memberships, 1);

    for target in ["race-a", "race-b"] {
        store
            .create_organization_membership(
                "org-race",
                target,
                "race-owner",
                OrganizationRole::Trader,
                "owner transfer target",
            )
            .await
            .unwrap();
    }
    let (a, b) = tokio::join!(
        store.transfer_organization_owner("org-race", "race-owner", "race-a", 1, 1, "transfer A"),
        store.transfer_organization_owner("org-race", "race-owner", "race-b", 1, 1, "transfer B"),
    );
    assert_ne!(a.is_ok(), b.is_ok());
    let owners:i64=sqlx::query_scalar("SELECT count(*) FROM organization_memberships WHERE organization_id='org-race' AND role='OWNER' AND status='ACTIVE'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(owners, 1);
    let transfer_events: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM organization_membership_events WHERE organization_id='org-race'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(transfer_events, 6);

    user(&pool, "revision-actor").await;
    user(&pool, "revision-target").await;
    store
        .create_organization(&org("org-revision-race"), "revision-actor")
        .await
        .unwrap();
    store
        .create_organization_membership(
            "org-revision-race",
            "revision-target",
            "revision-actor",
            OrganizationRole::Viewer,
            "revision fixture",
        )
        .await
        .unwrap();
    let (left, right) = tokio::join!(
        store.revise_organization_membership(
            "org-revision-race",
            "revision-target",
            "revision-actor",
            1,
            OrganizationRole::Trader,
            MembershipStatus::Active,
            "role race"
        ),
        store.revise_organization_membership(
            "org-revision-race",
            "revision-target",
            "revision-actor",
            1,
            OrganizationRole::Viewer,
            MembershipStatus::Suspended,
            "status race"
        ),
    );
    assert_ne!(left.is_ok(), right.is_ok());
    let revision:i64=sqlx::query_scalar("SELECT revision FROM organization_memberships WHERE organization_id='org-revision-race' AND user_id='revision-target'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(revision, 2);
    let revision_events:i64=sqlx::query_scalar("SELECT count(*) FROM organization_membership_events WHERE organization_id='org-revision-race' AND user_id='revision-target'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(revision_events, 2);
}

#[sqlx::test]
async fn last_owner_race_and_competing_account_claim_are_database_safe(pool: PgPool) {
    migrate(&pool).await.unwrap();
    for id in ["owner-one", "owner-two", "owner-a", "owner-b"] {
        user(&pool, id).await;
    }
    let store = TradingStore::new(pool.clone());
    store
        .create_organization(&org("org-last-owner"), "owner-one")
        .await
        .unwrap();
    store
        .create_organization_membership(
            "org-last-owner",
            "owner-two",
            "owner-one",
            OrganizationRole::Trader,
            "second owner fixture",
        )
        .await
        .unwrap();
    sqlx::query("UPDATE organization_memberships SET role='OWNER',revision=2,updated_by='owner-one',updated_at=NOW() WHERE organization_id='org-last-owner' AND user_id='owner-two'")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO organization_membership_events(organization_id,user_id,actor_user_id,revision,previous_role,next_role,previous_status,next_status,reason) VALUES('org-last-owner','owner-two','owner-one',2,'TRADER','OWNER','ACTIVE','ACTIVE','second owner fixture')")
        .execute(&pool).await.unwrap();
    let (one,two)=tokio::join!(
        sqlx::query("UPDATE organization_memberships SET role='ADMIN',revision=revision+1,updated_by='owner-one',updated_at=NOW() WHERE organization_id='org-last-owner' AND user_id='owner-one'").execute(&pool),
        sqlx::query("UPDATE organization_memberships SET role='ADMIN',revision=revision+1,updated_by='owner-one',updated_at=NOW() WHERE organization_id='org-last-owner' AND user_id='owner-two'").execute(&pool),
    );
    assert_ne!(one.is_ok(), two.is_ok());
    let active_owners:i64=sqlx::query_scalar("SELECT count(*) FROM organization_memberships WHERE organization_id='org-last-owner' AND role='OWNER' AND status='ACTIVE'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(active_owners, 1);

    account(&pool, "account-claim-race").await;
    store
        .create_organization(&org("org-claim-a"), "owner-a")
        .await
        .unwrap();
    store
        .create_organization(&org("org-claim-b"), "owner-b")
        .await
        .unwrap();
    let (a, b) = tokio::join!(
        store.assign_account_entitlement(
            "org-claim-a",
            "account-claim-race",
            "owner-a",
            1,
            0,
            "claim A"
        ),
        store.assign_account_entitlement(
            "org-claim-b",
            "account-claim-race",
            "owner-b",
            1,
            0,
            "claim B"
        ),
    );
    assert_ne!(a.is_ok(), b.is_ok());
    let (assignments,events):(i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM organization_account_entitlements WHERE account_id='account-claim-race'),(SELECT count(*) FROM organization_account_entitlement_events WHERE account_id='account-claim-race')")
        .fetch_one(&pool).await.unwrap();
    assert_eq!((assignments, events), (1, 1));
}

#[sqlx::test]
async fn concurrent_entitlement_revision_and_account_revision_races_are_serialized(pool: PgPool) {
    migrate(&pool).await.unwrap();
    user(&pool, "shared-owner").await;
    let store = TradingStore::new(pool.clone());
    store
        .create_organization(&org("org-shared-a"), "shared-owner")
        .await
        .unwrap();
    store
        .create_organization(&org("org-shared-b"), "shared-owner")
        .await
        .unwrap();
    account(&pool, "account-entitlement-race").await;
    store
        .assign_account_entitlement(
            "org-shared-a",
            "account-entitlement-race",
            "shared-owner",
            1,
            0,
            "initial",
        )
        .await
        .unwrap();

    let (reassign, suspend) = tokio::join!(
        store.assign_account_entitlement(
            "org-shared-b",
            "account-entitlement-race",
            "shared-owner",
            1,
            1,
            "race reassignment"
        ),
        store.set_account_entitlement_status(
            "org-shared-a",
            "account-entitlement-race",
            "shared-owner",
            1,
            1,
            EntitlementStatus::Suspended,
            "race suspension"
        ),
    );
    assert_ne!(reassign.is_ok(), suspend.is_ok());
    let (revision,events):(i64,i64)=sqlx::query_as("SELECT (SELECT revision FROM organization_account_entitlements WHERE account_id='account-entitlement-race'),(SELECT count(*) FROM organization_account_entitlement_events WHERE account_id='account-entitlement-race')")
        .fetch_one(&pool).await.unwrap();
    assert_eq!((revision, events), (2, 2));

    account(&pool, "account-revision-race").await;
    let (assign, modify) = tokio::join!(
        store.assign_account_entitlement(
            "org-shared-a",
            "account-revision-race",
            "shared-owner",
            1,
            0,
            "account revision race"
        ),
        sqlx::query(
            "UPDATE accounts SET label='changed concurrently' WHERE id='account-revision-race'"
        )
        .execute(&pool),
    );
    assert!(modify.is_ok());
    let (account_revision,entitlements):(i64,i64)=sqlx::query_as("SELECT (SELECT account_revision FROM accounts WHERE id='account-revision-race'),(SELECT count(*) FROM organization_account_entitlements WHERE account_id='account-revision-race')")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(account_revision, 2);
    if assign.is_ok() {
        let captured:i64=sqlx::query_scalar("SELECT account_revision FROM organization_account_entitlement_events WHERE account_id='account-revision-race'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(captured, 1);
        assert_eq!(entitlements, 1);
    } else {
        assert_eq!(entitlements, 0);
    }
}
