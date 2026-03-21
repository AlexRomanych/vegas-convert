mod importers; // Подключаем папку как модуль

use dotenvy::dotenv;
use sqlx::postgres::PgPoolOptions;
use std::env;
use anyhow::Context;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Загружаем .env
    dotenv().ok();

    // Получаем строку подключения
    let database_url = env::var("DATABASE_URL")
        .context("DATABASE_URL must be set in .env")?;

    // Создаем пул соединений
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .context("Failed to connect to Postgres")?;

    println!("✅ Связь с Postgres установлена. Движок готов.");

    // Здесь позже будет вызов парсера
    // run_import(&pool).await?;

    Ok(())
}

