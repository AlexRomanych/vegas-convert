use anyhow::{Context, Result};
use dotenvy::dotenv;
use sqlx::postgres::PgPoolOptions;
use std::env;
use sqlx::{Pool, Postgres};

/// **Соединяемся с базой**
pub async fn connect() -> Result<Pool<Postgres>> {
    // __ Инициализация окружения
    dotenv().ok();
    let database_url = env::var("DATABASE_URL").context("DATABASE_URL must be set in .env file")?;

    // __ Настройка пула соединений
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .context("Ошибка соединения с базой данных")?;

    Ok(pool)
}
