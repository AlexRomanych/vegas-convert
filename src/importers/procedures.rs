use calamine::{Reader, Xlsx, open_workbook, DataType};
use sqlx::{Postgres, Transaction};
use anyhow::{Context, Result};
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ModelConstructProcedure {
    pub code_1c: String,             // Первичный ключ
    pub name: String,                // Название процедуры
    pub text: Option<String>,        // Текст (может быть пустым)
    pub object_code_1c: Option<String>,
    pub object_name: Option<String>,
}

pub async fn run(
    tx: &mut Transaction<'_, Postgres>,
    path: &str,
    start_row: usize,
    code_1c_length: &usize,
) -> Result<()> {

    // println!("code_1c_length: {}", code_1c_length);

    let mut workbook: Xlsx<_> = open_workbook(path)
        .with_context(|| format!("Не удалось открыть файл процедур: {}", path))?;

    // Предполагаем, что данные на первом листе
    let range = workbook.worksheet_range_at(0)
        .context("Лист в файле процедур не найден")??;

    let mut count = 0;

    for row in range.rows().skip(start_row) { // Пропускаем заголовок Excel
        // Маппинг колонок (индексы 0, 1, 2... должны соответствовать порядку в Excel)
        let procedure = ModelConstructProcedure {

            // Используем .map вместо .and_then(|c| Some(c...)), так короче
            code_1c: {
                let raw = row.get(0).map(|c| c.to_string()).unwrap_or_default();
                if raw.is_empty() {
                    raw // возвращает пустую String
                } else {
                    format!("{:0>width$}", raw, width = code_1c_length) // возвращает отформатированную String
                }
            },

            name: row.get(1)
                .map(|c| c.to_string())
                .unwrap_or_else(|| "Без названия".to_string()),

            text: row.get(2).map(|c| c.to_string()),
            // text: Some("".to_string()),

            object_code_1c: {
                let raw =  row.get(3).map(|c| c.to_string()).unwrap_or_default();
                if raw.is_empty() {
                    None // Если пусто, возвращаем Option::None (в БД будет NULL)
                } else {
                    // Форматируем и оборачиваем в Some
                    Some(format!("{:0>width$}", raw, width = code_1c_length))
                }
            },

            object_name: row.get(4).map(|c| c.to_string()),

            // code_1c: row.get(0).and_then(|c| Some(c.to_string())).unwrap_or_default(),
            // name: row.get(1).and_then(|c| Some(c.to_string())).unwrap_or_else(|| "Без названия".into()),
            // text: row.get(2).map(|c| c.to_string()),
            // object_code_1c: row.get(3).map(|c| c.to_string()),
            // object_name: row.get(4).map(|c| c.to_string()),
        };

        if procedure.code_1c.is_empty() { continue; }

        // Выполняем вставку с обновлением при конфликте (Upsert)
        sqlx::query(
            r#"
                INSERT INTO model_construct_procedures (
                    code_1c, name, text, object_code_1c, object_name, updated_at, created_at
                )
                VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
                ON CONFLICT (code_1c) DO UPDATE SET
                    name = EXCLUDED.name,
                    text = EXCLUDED.text,
                    object_code_1c = EXCLUDED.object_code_1c,
                    object_name = EXCLUDED.object_name,
                    updated_at = NOW()
           "#
        )
            .bind(&procedure.code_1c)
            .bind(&procedure.name)
            .bind(&procedure.text)
            .bind(&procedure.object_code_1c)
            .bind(&procedure.object_name)
            .execute(&mut **tx)
            .await?;

        count += 1;
    }

    println!("✅ Процедуры: импортировано {} строк", count);
    Ok(())
}


/*
Основные моменты реализации:
Типы данных: * CODE_1C из Laravel стал String, так как это первичный ключ в виде строки.
Поля nullable() в миграции стали Option<String> в Rust.
Upsert (ON CONFLICT): Поскольку 1С часто выгружает данные повторно, мы используем DO UPDATE. Это обновит существующие записи, если code_1c уже есть в базе.
Производительность: Функция принимает &mut Transaction. Это позволяет вызвать импорт процедур в рамках одного большого процесса в main.rs. Если один файл упадет — откатится всё.
Безопасность: Использование unwrap_or_default() и map() предотвращает "панику" программы, если в Excel встретится пустая ячейка в середине данных.
 */