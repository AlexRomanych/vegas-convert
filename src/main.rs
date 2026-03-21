#[allow(unused)]

mod importers; // Подключаем папку как модуль

use std::path::PathBuf;
use std::env;
use dotenvy::dotenv;
use sqlx::postgres::PgPoolOptions;
use anyhow::{Context, Result};

#[tokio::main]
async fn main() -> Result<()> {

    // __ Инициализация окружения
    dotenv().ok();
    let database_url = env::var("DATABASE_URL")
        .context("DATABASE_URL must be set in .env file")?;

    // __ Настройка пула соединений
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .context("Failed to connect to Postgres")?;

    println!("✅ Связь с базой установлена.");

    // __ Создаем транзакцию (если один файл упадет, база не засорится)
    let mut tx = pool.begin().await?;

    println!("🚀 Начинаем импорт файлов из 1С/...");


    const CODE_1C_LENGTH: usize = 9;                        // __ Количество символов в коде в 1С
    const IMPORT_PATH: &str = "storage/app/1c_imports";     // __ Путь к отчетам

    // __ Сами отчеты
    const PROCEDURES_FILE_NAME: &str = "procedures.xlsx";           // __ Процедуры

    // Явно указываем тип PathBuf, чтобы IDE не терялась
    let procedures_path = PathBuf::from(IMPORT_PATH).join(PROCEDURES_FILE_NAME); // push — это аналог join, который меняет путь на месте

    // let mut procedures_path = PathBuf::from(IMPORT_PATH);
    // procedures_path.push(PROCEDURES_FILE_NAME); // push — это аналог join, который меняет путь на месте

    // Превращаем PathBuf в строку
    // .display().to_string() — самый надежный способ передачи пути в аргумент &str
    let path_str = procedures_path.display().to_string();

    if !procedures_path.exists() {
        anyhow::bail!("Файл не найден: {:?}", procedures_path);
    }

    // __ Вызов импортера процедур.
    // __ Передаем транзакцию по ссылке (&mut tx)

    importers::procedures::run(&mut tx, &path_str, 4, &CODE_1C_LENGTH)
        .await
        .context("Ошибка при импорте процедур")?;

    // 5. Фиксация изменений
    tx.commit().await?;

    println!("🏁 Весь импорт завершен успешно!");
    Ok(())
}