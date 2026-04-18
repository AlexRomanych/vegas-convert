use anyhow::{Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value; // __ Если ты сам определяешь LogLevel, импорт не нужен, но обычно делают так
use sqlx::types::Json; // __ Если Json берется из sqlx (для автоматического маппинга в PostgreSQL)
use sqlx::{PgPool};
use std::fmt;
use std::fmt::Display;

// __ Модуль ошибки
#[derive(Serialize, Deserialize, Debug, sqlx::Type)]
#[sqlx(type_name = "varchar")]
pub enum LogTarget {
    Default,
    ModelsUpdate,
    Compiler,
}

impl Display for LogTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogTarget::Default => write!(f, "default"),
            LogTarget::ModelsUpdate => write!(f, "update_models"),
            LogTarget::Compiler => write!(f, "compile_procedures"),
        }
    }
}

impl Default for LogTarget {
    fn default() -> Self {
        Self::Default
    }
}

impl From<LogTarget> for String {
    fn from(target: LogTarget) -> String {
        match target {
            LogTarget::Default => String::from("default"),
            LogTarget::ModelsUpdate => String::from("update_models"),
            LogTarget::Compiler => String::from("compile_procedures"),
        }
    }
}


// __ Уровень лога
#[derive(Serialize, Deserialize, Debug, sqlx::Type)]
// Полезно для логирования и передачи в Laravel
// Если ты не хочешь вручную конвертировать его в строку перед каждым запросом INSERT, можно заставить sqlx делать это автоматически. Для этого нужно добавить дериватив Type.
#[sqlx(type_name = "varchar")] // Говорим sqlx, что в БД это будет обычная строка
pub enum LogLevel {
    WARN,
    INFO,
    ERROR,
    DEBUG,
}


// Добавь импорт std::fmt и реализуй трейт. Это позволит тебе использовать .to_string() или макрос format!("{}", level).
// let level_str = message.level.to_string(); // Получишь "INFO"
impl Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            LogLevel::WARN => write!(f, "WARN"),
            LogLevel::INFO => write!(f, "INFO"),
            LogLevel::ERROR => write!(f, "ERROR"),
            LogLevel::DEBUG => write!(f, "DEBUG"),
        }
    }
}

// Обычно для enum, который нужно превращать в строку, реализуют From<LogLevel> for String или From<LogLevel> for &'static str.
// let my_level = LogLevel::INFO;
//
// // Вариант А: Явный вызов
// let s = String::from(my_level);
//
// // Вариант Б: Использование .into()
// // (Rust сам поймет, что нужно превратить в String, если указан тип)
// let level_string: String = my_level.into();
//
// // В SQL запросе:
// sqlx::query!(
//     "INSERT INTO logs (level, message) VALUES ($1, $2)",
//     String::from(message.level), // Быстро и понятно
//     message.message
// );


impl From<LogLevel> for String {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::WARN => String::from("WARN"),
            LogLevel::INFO => String::from("INFO"),
            LogLevel::ERROR => String::from("ERROR"),
            LogLevel::DEBUG => String::from("DEBUG"),
        }
    }
}

impl Default for LogLevel {
    fn default() -> Self {
        LogLevel::INFO
    }
}

// __ Структура для записи ошибок
#[derive(Default, Debug)]
pub struct LogMessage {
    pub level: LogLevel,
    pub target: LogTarget,
    pub message: String,
    pub context: Option<Json<Value>>,
    pub created_at: Option<DateTime<Utc>>,
}


impl LogMessage {
    // __ Таблица с логами
    pub const EVENT_LOG_TABLE_NAME: &'static str = "event_logs";

    // __ Создаем объект лога
    pub fn new(level: LogLevel, target: LogTarget, message: String, context: Option<Json<Value>>) -> Self {
        // В Rust структура создается внутри фигурных скобок с указанием имен полей
        Self {
            level,
            target,
            message,
            context,
            created_at: Some(Utc::now()), // __ У chrono текущее время берется через Utc::now()
        }
    }

    // __ Записываем в базу
    pub async fn write(&self, executor: &PgPool) -> Result<()> {
        let query_str = format!(
            r#"
                INSERT INTO {} (
                    level, target, message, context,
                    created_at
                )
                VALUES (
                    $1, $2, $3,
                    COALESCE($4, '{}'::jsonb),
                    NOW() AT TIME ZONE 'Europe/Minsk'
                )
            "#,
            Self::EVENT_LOG_TABLE_NAME,
            "{}" // Это подставится в COALESCE как строка для Postgres
        );

        // let query_str = format!(
        //     r#"
        //     INSERT INTO {} (
        //         level, target, message, context,
        //         created_at
        //     )
        //     VALUES (
        //         $1, $2, $3, COALESCE($4, DEFAULT),
        //         NOW() AT TIME ZONE 'Europe/Minsk'
        //     )
        //     "#,
        //     Self::EVENT_LOG_TABLE_NAME
        // );

        // __ Превращаем строку в статическую ссылку. Теперь компилятор не боится её потерять.
        let static_query: &'static str = Box::leak(query_str.into_boxed_str());

        sqlx::query(static_query)
            .bind(self.level.to_string())
            .bind(&self.target)
            .bind(&self.message)
            .bind(&self.context)
            .execute(executor)
            .await?; // Rust гарантирует, что query_str живет до этой точки

        Ok(())
    }
}
