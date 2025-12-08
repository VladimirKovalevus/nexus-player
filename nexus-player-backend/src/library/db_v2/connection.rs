use anyhow::{Context, Result};
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use std::time::Duration;

pub async fn establish_connection(database_url: &str) -> Result<DatabaseConnection> {
    let mut opt = ConnectOptions::new(database_url.to_owned());
    opt.max_connections(100)
        .min_connections(5)
        .connect_timeout(Duration::from_secs(8))
        .acquire_timeout(Duration::from_secs(8))
        .idle_timeout(Duration::from_secs(8))
        .max_lifetime(Duration::from_secs(8))
        .sqlx_logging(true);

    let db = Database::connect(opt)
        .await
        .context("Failed to connect to database")?;

    Ok(db)
}

pub async fn establish_connection_with_migrations(database_url: &str) -> Result<DatabaseConnection> {
    let db = establish_connection(database_url).await?;

    Ok(db)
}
