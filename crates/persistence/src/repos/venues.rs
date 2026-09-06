use domain::{Venue, VenueType};
use sqlx::PgPool;

use crate::error::PersistenceResult;

pub async fn upsert(pool: &PgPool, venue: &Venue) -> PersistenceResult<()> {
    let venue_type = match venue.venue_type {
        VenueType::Simulated => "simulated",
        VenueType::Prop => "prop",
        VenueType::Cex => "cex",
        VenueType::Onchain => "onchain",
    };
    sqlx::query(
        r#"
        INSERT INTO venues (id, venue_type, name, enabled, created_at)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (id) DO UPDATE SET
            venue_type = EXCLUDED.venue_type,
            name = EXCLUDED.name,
            enabled = EXCLUDED.enabled
        "#,
    )
    .bind(venue.id.as_str())
    .bind(venue_type)
    .bind(&venue.name)
    .bind(venue.enabled)
    .bind(venue.created_at)
    .execute(pool)
    .await?;
    Ok(())
}
