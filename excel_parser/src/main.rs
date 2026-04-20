// #[allow(unused)]
mod constants; // __ Подключаем константы
mod helpers;
mod importers; // __ Подключаем папку как модуль с импортерами из Excel в Postgres
mod structures;


use crate::constants::PRODUCTION;
use anyhow::{Context, Result};
use constants::{IMPORT_PATH, MATERIALS_FILE_NAME, MODELS_FILE_NAME, PROCEDURES_FILE_NAME, SPECIFICATIONS_FILE_NAME};
use dotenvy::dotenv;
use logger::structures::log_message::{LogLevel, LogMessage, LogTarget};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use sqlx::types::Json;
use std::env;
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::Instant;
// use stats_alloc::{Region, StatsAlloc, INSTRUMENTED_SYSTEM};
// use std::alloc::System;
// use dhat::Alloc;

// #[global_allocator]
// static ALLOCATOR: Alloc = Alloc;
// static GLOBAL_ALLOC: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;


#[tokio::main]
async fn main() -> Result<()> {
    // __ Статистические измерения
    let start_time = Instant::now();

    // __ Инициализация профилировщика
    // __ Файл dhat-heap.json запишется автоматически при выходе из main
    // let _profiler = dhat::Profiler::builder().build();
    // В Region мы тоже передаем ссылку на этот же объект
    // let reg = Region::new(&INSTRUMENTED_SYSTEM);

    // __ Инициализация окружения
    dotenv().ok();
    let database_url = env::var("DATABASE_URL").context("DATABASE_URL must be set in .env file")?;

    // __ Настройка пула соединений
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .context("Ошибка соединения с базой данных")?;
    // .context("Failed to connect to Postgres")?;

    if !PRODUCTION {
        println!("✅ Связь с базой установлена.")
    };

    let inform_message = LogMessage {
        level:      LogLevel::INFO,
        target:     LogTarget::ModelsUpdate,
        message:    "Начало обновления".to_string(),
        context:    None,
        created_at: None,
    };
    inform_message.write(&pool).await.ok();


    // __ Создаем транзакцию (если один файл упадет, база не засорится)
    let mut tx = pool.begin().await?;

    // __ Сами отчеты

    // __ Процедуры
    // __ Явно указываем тип PathBuf, чтобы IDE не терялась
    let file_path = PathBuf::from(IMPORT_PATH).join(PROCEDURES_FILE_NAME); // push — это аналог join, который меняет путь на месте

    // let mut file_path = PathBuf::from(IMPORT_PATH);
    // file_path.push(PROCEDURES_FILE_NAME); // push — это аналог join, который меняет путь на месте

    // Превращаем PathBuf в строку .display().to_string() — самый надежный способ передачи пути в аргумент &str
    let path_str = file_path.display().to_string();

    if !file_path.exists() {
        anyhow::bail!("Файл не найден: {:?}", file_path);
    }

    // __ Вызов импортера Процедур. Передаем транзакцию по ссылке (&mut tx)
    if !PRODUCTION {
        println!("🚀 Начинаем импорт процедур из 1С/...")
    };

    importers::procedures::run(&mut tx, &path_str, &pool)
        .await?;

    // importers::procedures::run(&mut tx, &path_str, &pool)
    //     .await
    //     .context("Ошибка при импорте процедур")?;


    // __ Материалы
    let file_path = PathBuf::from(IMPORT_PATH).join(MATERIALS_FILE_NAME); // push — это аналог join, который меняет путь на месте
    let path_str = file_path.display().to_string();

    if !file_path.exists() {
        anyhow::bail!("Файл не найден: {:?}", file_path);
    }

    // __ Вызов импортера Материалов. Передаем транзакцию по ссылке (&mut tx)
    if !PRODUCTION {
        println!("🚀 Начинаем импорт материалов из 1С/...")
    };

    importers::materials::run(&mut tx, &path_str, &pool)
        .await?;

    // importers::materials::run(&mut tx, &path_str, &pool)
    //     .await
    //     .context("Ошибка при импорте материалов")?;

    // __ Модели
    let file_path = PathBuf::from(IMPORT_PATH).join(MODELS_FILE_NAME);
    let path_str = file_path.display().to_string();

    if !file_path.exists() {
        anyhow::bail!("Файл не найден: {:?}", file_path);
    }

    // __ Вызов импортера Моделей. Передаем транзакцию по ссылке (&mut tx)
    if !PRODUCTION {
        println!("🚀 Начинаем импорт моделей из 1С/...")
    };

    importers::models::run(&mut tx, &path_str, &pool)
        .await?;

    // importers::models::run(&mut tx, &path_str, &pool)
    //     .await
    //     .context("Ошибка при импорте моделей")?;

    // __ Спецификации
    let file_path = PathBuf::from(IMPORT_PATH).join(SPECIFICATIONS_FILE_NAME);
    let path_str = file_path.display().to_string();

    if !file_path.exists() {
        anyhow::bail!("Файл не найден: {:?}", file_path);
    }

    // __ Вызов импортера Спецификаций. Передаем транзакцию по ссылке (&mut tx)
    if !PRODUCTION {
        println!("🚀 Начинаем импорт Спецификаций из 1С/...")
    };

    importers::specifications::run(&mut tx, &path_str, &pool)
        .await?;

    // importers::specifications::run(&mut tx, &path_str, &pool)
    //     .await
    //     .context("Ошибка при импорте спецификаций")?;


    // __ Фиксация изменений
    tx.commit().await?;

    if !PRODUCTION {
        println!("🏁 Весь импорт завершен успешно!")
    };

    // __ Статистика по памяти
    // let stats = reg.change();
    // println!("Статистика по куче (heap): {:#?}", stats);
    //
    // __ Пик потребления в байтах:
    // let current_usage = stats.bytes_allocated - stats.bytes_deallocated;
    // println!("Текущее (итоговое) потребление: {} МБ", current_usage / 1024 / 1024);


    // __ Статистика по времени
    let duration = start_time.elapsed();
    if !PRODUCTION {
        println!("Время выполнения: {:?}", duration);
        println!("Прошло миллисекунд: {}", duration.as_millis());
    }

    let inform_message = LogMessage {
        level:      LogLevel::INFO,
        target:     LogTarget::ModelsUpdate,
        message:    "Окончание обновления".to_string(),
        context:    Some(Json(json!({
            "elapsed_time, sec.": format!("{:?}", duration),
        }))),
        created_at: None,
    };
    inform_message.write(&pool).await.ok();

    // __ Принудительно толкаем в буфер
    io::stdout().flush()?;
    // io::stdout().flush().unwrap();

    if PRODUCTION {
        println!("0")
    };
    Ok(())
}
