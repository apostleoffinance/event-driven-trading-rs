//! CLI: read NDJSON TradeIntents from stdin (Python strategy output).

use std::fs::File;
use std::io::{self, Write};
use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use domain::{AccountId, OrderStatus, RiskPolicy};
use intent_bridge::{BridgeVenueKind, IntentIngestSession};
use rust_decimal::Decimal;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Clone, ValueEnum)]
enum VenueArg {
    Simulated,
    Prop,
}

#[derive(Parser, Debug)]
#[command(name = "intent-bridge")]
#[command(about = "Ingest Python TradeIntent NDJSON into the Rust trading pipeline")]
struct Args {
    /// Account id for the execution session.
    #[arg(long, default_value = "prop-account-001")]
    account_id: String,

    /// Starting capital.
    #[arg(long, default_value = "100000")]
    capital: String,

    /// Venue adapter to use.
    #[arg(long, value_enum, default_value_t = VenueArg::Prop)]
    venue: VenueArg,

    /// Optional NDJSON file (default: stdin).
    #[arg(long)]
    input: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    let args = Args::parse();
    let account_id = AccountId::new(args.account_id)?;
    let capital = Decimal::from_str_exact(&args.capital)?;
    let policy = RiskPolicy::try_new(
        Decimal::from(2),
        Decimal::from(10),
        Decimal::from(50),
        Decimal::from(2),
        Decimal::from(5),
        Decimal::from(10),
        5,
    )?;

    let venue_kind = match args.venue {
        VenueArg::Simulated => BridgeVenueKind::Simulated,
        VenueArg::Prop => BridgeVenueKind::Prop,
    };

    let mut session =
        IntentIngestSession::bootstrap(account_id, venue_kind, capital, policy).await?;

    let outcomes = if let Some(path) = args.input {
        let file = File::open(path)?;
        session.ingest_ndjson(file).await?
    } else {
        session.ingest_ndjson(io::stdin()).await?
    };

    let mut out = io::stdout().lock();
    for outcome in &outcomes {
        let status = outcome.order_status.map(|s| s.as_str()).unwrap_or("NONE");
        writeln!(
            out,
            "{{\"intent_id\":\"{}\",\"executable\":{},\"order_status\":\"{}\"}}",
            outcome.intent.id,
            outcome.decision.is_executable(),
            status
        )?;
        if outcome.order_status == Some(OrderStatus::Filled) {
            tracing::info!(intent_id = %outcome.intent.id, "intent filled via Rust pipeline");
        }
    }

    Ok(())
}
