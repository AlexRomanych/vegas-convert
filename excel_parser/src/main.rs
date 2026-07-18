// #[allow(unused)]
mod constants; // __ Подключаем константы
mod helpers;
mod importers; // __ Подключаем папку как модуль с импортерами из Excel в Postgres
mod structures;

use crate::constants::{IMPORT_PATH_LOCAL, IMPORT_PATH_PRODUCTION, SPECIFICATIONS_ADD_FILE_NAME};
use anyhow::{Context, Result};
use constants::{MATERIALS_FILE_NAME, MODELS_FILE_NAME, PROCEDURES_FILE_NAME, SPECIFICATIONS_FILE_NAME};
use dotenvy::dotenv;
use logger::structures::log_message::{LogLevel, LogMessage, LogTarget};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use sqlx::types::Json;
// use std::collections::HashSet;
use std::env;
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::Instant;
// use crate::constants::PRODUCTION;
// use stats_alloc::{Region, StatsAlloc, INSTRUMENTED_SYSTEM};
// use std::alloc::System;
// use dhat::Alloc;

// #[global_allocator]
// static ALLOCATOR: Alloc = Alloc;
// static GLOBAL_ALLOC: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

const UPDATE_MODE: &str = "update";
const ADD_MODE: &str = "add";


#[tokio::main]
async fn main() -> Result<()> {
    // __ Статистические измерения
    let start_time = Instant::now();

    // __ Инициализация профилировщика
    // __ Файл dhat-heap.json запишется автоматически при выходе из main
    // let _profiler = dhat::Profiler::builder().build();
    // В Region мы тоже передаем ссылку на этот же объект
    // let reg = Region::new(&INSTRUMENTED_SYSTEM);

    // __ Получаем режим
    // __ Если не передали параметр, парсим все
    // __ Если передали "add" - добавляем тендерные Спецификации

    let mut mode = UPDATE_MODE;

    if !cfg!(debug_assertions) {
        // __ На проде
        // __ Получаем входной параметр
        // 1. Собираем аргументы командной строки
        let args: Vec<String> = env::args().collect();

        // args[0] — путь к бинарнику, args[1] — наш parametr от Laravel
        if args.len() == 2 {
            if args[1].to_lowercase().eq(ADD_MODE) {
                mode = ADD_MODE;
            };
        }
    } else {
        // __ На dev
        mode = ADD_MODE;
    }

    println!("mode: {}", mode);

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

    if cfg!(debug_assertions) {
        println!("✅ Связь с базой установлена.")
    }

    // __ Создаем транзакцию (если один файл упадет, база не засорится)
    let mut tx = pool.begin().await?;

    let import_path = if cfg!(debug_assertions) { IMPORT_PATH_LOCAL } else { IMPORT_PATH_PRODUCTION };

    // __ Информационное сообщение
    let inform_message = LogMessage {
        level: LogLevel::INFO,
        target: LogTarget::ModelsUpdate,
        message: if mode == ADD_MODE { "Начало добавления".to_string() } else { "Начало обновления".to_string() },
        context: None,
        created_at: None,
    };
    inform_message.write(&pool).await.ok();

    if mode == ADD_MODE {
        // ___ Режим добавления тендерных Спецификаций
        let file_path = PathBuf::from(import_path)
            .join(SPECIFICATIONS_ADD_FILE_NAME);

        let path_str = file_path.display().to_string();
        if !file_path.exists() {
            anyhow::bail!("Файл не найден: {:?}", file_path);
        }

        // __ Вызов импортера Добавления Спецификаций. Передаем транзакцию по ссылке (&mut tx)
        if cfg!(debug_assertions) {
            println!("🚀 Начинаем импорт Дополнительных Спецификаций из 1С...");
        }

        importers::specifications::add(&mut tx, &path_str, &pool).await?;



    } else {
        // ___ Режим Обновления Справочников

        // __ Сами отчеты

        // __ Процедуры
        // __ Явно указываем тип PathBuf, чтобы IDE не терялась
        let file_path = PathBuf::from(import_path).join(PROCEDURES_FILE_NAME); // push — это аналог join, который меняет путь на месте

        // let mut file_path = PathBuf::from(IMPORT_PATH);
        // file_path.push(PROCEDURES_FILE_NAME); // push — это аналог join, который меняет путь на месте

        // Превращаем PathBuf в строку .display().to_string() — самый надежный способ передачи пути в аргумент &str
        let path_str = file_path.display().to_string();

        if !file_path.exists() {
            anyhow::bail!("Файл не найден: {:?}", file_path);
        }

        // __ Вызов импортера Процедур. Передаем транзакцию по ссылке (&mut tx)
        if cfg!(debug_assertions) {
            println!("🚀 Начинаем импорт процедур из 1С...")
        }

        importers::procedures::run(&mut tx, &path_str, &pool).await?;

        // importers::procedures::run(&mut tx, &path_str, &pool)
        //     .await
        //     .context("Ошибка при импорте процедур")?;


        // __ Материалы
        let file_path = PathBuf::from(import_path).join(MATERIALS_FILE_NAME); // push — это аналог join, который меняет путь на месте
        let path_str = file_path.display().to_string();

        if !file_path.exists() {
            anyhow::bail!("Файл не найден: {:?}", file_path);
        }

        // __ Вызов импортера Материалов. Передаем транзакцию по ссылке (&mut tx)
        if cfg!(debug_assertions) {
            println!("🚀 Начинаем импорт материалов из 1С...");
        }

        importers::materials::run(&mut tx, &path_str, &pool).await?;

        // importers::materials::run(&mut tx, &path_str, &pool)
        //     .await
        //     .context("Ошибка при импорте материалов")?;

        // __ Модели
        let file_path = PathBuf::from(import_path).join(MODELS_FILE_NAME);
        let path_str = file_path.display().to_string();

        if !file_path.exists() {
            anyhow::bail!("Файл не найден: {:?}", file_path);
        }

        // __ Вызов импортера Моделей. Передаем транзакцию по ссылке (&mut tx)
        if cfg!(debug_assertions) {
            println!("🚀 Начинаем импорт моделей из 1С/...");
        }

        importers::models::run(&mut tx, &path_str, &pool).await?;

        // importers::models::run(&mut tx, &path_str, &pool)
        //     .await
        //     .context("Ошибка при импорте моделей")?;

        // __ Спецификации
        let file_path = PathBuf::from(import_path).join(SPECIFICATIONS_FILE_NAME);
        let path_str = file_path.display().to_string();

        if !file_path.exists() {
            anyhow::bail!("Файл не найден: {:?}", file_path);
        }

        // __ Вызов импортера Спецификаций. Передаем транзакцию по ссылке (&mut tx)
        if cfg!(debug_assertions) {
            println!("🚀 Начинаем импорт Спецификаций из 1С...");
        }

        importers::specifications::run(&mut tx, &path_str, &pool).await?;

        // importers::specifications::run(&mut tx, &path_str, &pool)
        //     .await
        //     .context("Ошибка при импорте спецификаций")?;

    }


    // __ Фиксация изменений
    tx.commit().await?;

    if cfg!(debug_assertions) {
        println!("🏁 Весь импорт завершен успешно!");
    }

    // __ Статистика по памяти
    // let stats = reg.change();
    // println!("Статистика по куче (heap): {:#?}", stats);
    //
    // __ Пик потребления в байтах:
    // let current_usage = stats.bytes_allocated - stats.bytes_deallocated;
    // println!("Текущее (итоговое) потребление: {} МБ", current_usage / 1024 / 1024);

    // __ Статистика по времени
    let duration = start_time.elapsed();
    if cfg!(debug_assertions) {
        println!("Время выполнения: {:?}", duration);
        println!("Прошло миллисекунд: {}", duration.as_millis());
    }

    let inform_message = LogMessage {
        level: LogLevel::INFO,
        target: LogTarget::ModelsUpdate,
        message: if mode == UPDATE_MODE { "Окончание добавления".to_string() } else { "Окончание обновления".to_string() },
        context: Some(Json(json!({
            "elapsed_time, sec.": format!("{:?}", duration),
        }))),
        created_at: None,
    };
    inform_message.write(&pool).await.ok();

    if cfg!(debug_assertions) {
        println!("0")
    }

    // __ Принудительно толкаем в буфер
    io::stdout().flush()?;
    // io::stdout().flush().unwrap();

    Ok(())
}
