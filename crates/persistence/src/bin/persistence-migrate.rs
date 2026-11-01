use std::env;
use std::error::Error;
use std::io;

use persistence::{connect, migrate};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let database_url = env::var("DATABASE_URL").map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "DATABASE_URL must be set to the intended migration target",
        )
    })?;
    if database_url.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "DATABASE_URL must not be empty",
        )
        .into());
    }

    let pool = connect(&database_url).await?;
    migrate(&pool).await?;
    pool.close().await;
    println!("database migrations applied successfully");
    Ok(())
}
