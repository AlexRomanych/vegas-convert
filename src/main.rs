#[allow(unused)]
mod constants; // __ Подключаем константы
mod helpers;
mod importers; // __ Подключаем папку как модуль с импортерами из Excel в Postgres
mod structures; // __ Подключаем папку как модуль со структурами данных

use anyhow::{Context, Result};
use dotenvy::dotenv;
use sqlx::postgres::PgPoolOptions;
use std::env;
use std::path::PathBuf;
use constants::{IMPORT_PATH, CODE_1C_LENGTH};
use crate::structures::material::Material;

#[tokio::main]
async fn main() -> Result<()> {
    // __ Инициализация окружения
    dotenv().ok();
    let database_url = env::var("DATABASE_URL").context("DATABASE_URL must be set in .env file")?;

    // __ Настройка пула соединений
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .context("Failed to connect to Postgres")?;

    println!("✅ Связь с базой установлена.");

    // __ Создаем транзакцию (если один файл упадет, база не засорится)
    let mut tx = pool.begin().await?;

    // __ Сами отчеты

    // __ Процедуры
    const PROCEDURES_FILE_NAME: &str = "procedures.xlsx"; // __ Процедуры

    // Явно указываем тип PathBuf, чтобы IDE не терялась
    let file_path = PathBuf::from(IMPORT_PATH).join(PROCEDURES_FILE_NAME); // push — это аналог join, который меняет путь на месте

    // let mut file_path = PathBuf::from(IMPORT_PATH);
    // file_path.push(PROCEDURES_FILE_NAME); // push — это аналог join, который меняет путь на месте

    // Превращаем PathBuf в строку .display().to_string() — самый надежный способ передачи пути в аргумент &str
    let path_str = file_path.display().to_string();

    if !file_path.exists() {
        anyhow::bail!("Файл не найден: {:?}", file_path);
    }

    // __ Вызов импортера процедур. Передаем транзакцию по ссылке (&mut tx)
    println!("🚀 Начинаем импорт процедур из 1С/...");

    importers::procedures::run(&mut tx, &path_str, 4, &CODE_1C_LENGTH)
        .await
        .context("Ошибка при импорте процедур")?;


    // __ Материалы
    let file_path = PathBuf::from(IMPORT_PATH).join(Material::MATERIALS_FILE_NAME); // push — это аналог join, который меняет путь на месте
    let path_str = file_path.display().to_string();

    if !file_path.exists() {
        anyhow::bail!("Файл не найден: {:?}", file_path);
    }

    // __ Вызов импортера материалов. Передаем транзакцию по ссылке (&mut tx)
    println!("🚀 Начинаем импорт материалов из 1С/...");

    importers::materials::run(&mut tx, &path_str)
        .await
        .context("Ошибка при импорте материалов")?;


    // __ Фиксация изменений
    tx.commit().await?;

    println!("🏁 Весь импорт завершен успешно!");
    Ok(())
}
