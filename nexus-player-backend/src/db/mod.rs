use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::path::Path;
use std::str::FromStr;
use tracing::info;

pub async fn create_pool(db_path: &Path) -> anyhow::Result<SqlitePool> {
    info!("Initializing database: {:?}", db_path);

    let db_url = format!("sqlite://{}", db_path.display());

    let connect_options = SqliteConnectOptions::from_str(&db_url)?
        .create_if_missing(true)
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(connect_options)
        .await?;

    info!("Database connected, running migrations...");

    sqlx::migrate!("./src/db/migrations").run(&pool).await?;

    info!("Migrations applied successfully");

    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_create_pool() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let pool = create_pool(&db_path).await.unwrap();

        assert!(db_path.exists());

        let result: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks")
            .fetch_one(&pool)
            .await
            .unwrap();

        assert_eq!(result.0, 0);
    }
}
