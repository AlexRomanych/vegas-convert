#![allow(unused)]
use crate::constants::CODE_1C_LENGTH;
use calamine::{Data, Range};
use sqlx::{PgPool, Postgres, Transaction};
use std::str::FromStr;
use serde_json::json;
use sqlx::types::Json;
use crate::structures::custom_errors::CustomError;
use crate::structures::material::Material;
use crate::structures::traits::{ExcelPattern};

const REMOVABLE_BP: &str = "БП";

/// **Возвращает приведенную к нормальному виду строку кода 1С**
pub fn get_formatted_1c_code_string(raw_code: String) -> String {
    if raw_code.is_empty() {
        raw_code // __ возвращает пустую String
    } else {
        let chars_count = raw_code.chars().count();
        if chars_count > CODE_1C_LENGTH {
            return raw_code
                .chars()
                .rev() // Разворачиваем строку задом наперед
                .take(CODE_1C_LENGTH) // Берем 9 символов
                .collect::<String>() // Собираем обратно
                .chars() // Снова превращаем в символы
                .rev() // Разворачиваем в правильный порядок
                .collect(); // Итог: "000000086"

            // return raw_code[raw_code.len() - CODE_1C_LENGTH - 1..].to_string()

            // __ Если строка может быть короче 9 символов, а тебе нужно строго забрать то, что есть, можно сделать чуть проще через skip
            // return raw_code
            //     .chars()
            //     .skip(count.saturating_sub(CODE_1C_LENGTH)) // Пропускаем всё, кроме последних 9
            //     .collect();
        };


        // __ Убираем БП
        // let mut result_str = String::from(raw_code.clone());
        // if result_str.len() > CODE_1C_LENGTH && result_str.contains(REMOVABLE_BP) {
        //     result_str = result_str.strip_suffix(REMOVABLE_BP).unwrap_or(&result_str).to_string();
        // }


        format!("{:0>width$}", raw_code, width = CODE_1C_LENGTH) // __ возвращает отформатированную String
    }
}

/// **Возвращает приведенную к нормальному виду единицу измерения**
pub fn get_formatted_unit_string(raw_code: String) -> String {
    match raw_code.as_str() {
        "кг" => "кг".to_string(),
        "м2" => "м2".to_string(),
        "м п" | "пог. м" => "мп".to_string(),
        "шт" => "шт.".to_string(),
        "упак" => "упак.".to_string(),
        "м" => "м".to_string(),
        "рул" => "рул.".to_string(),
        "боб" => "боб.".to_string(),
        "л" => "л".to_string(),
        "компл" => "компл.".to_string(),
        "мл" => "".to_string(),
        "м3" => "м3".to_string(),
        _ => raw_code,
    }
}

/// **Возвращает данные Excel ячейки в строковом формате по Enum Data**
pub fn cell_to_string_by_enum(data: &Data) -> String {
    match data {
        Data::String(s) => s.clone(),
        Data::Int(i) => i.to_string(),
        Data::Float(f) => f.to_string(),
        _ => String::new(),
    }
}

/// **Возвращает данные Excel ячейки в строковом формате по Option<Data>**
pub fn cell_to_string_by_option(data_option: Option<&Data>) -> String {
    match data_option {
        Some(data) => cell_to_string_by_enum(data),
        None => String::new(),
    }
}

/// **Возвращает данные Excel ячейки в строковом формате по Option<Data>**
pub fn cell_to_generic<T>(data_option: Option<&Data>) -> Option<T>
where
    T: FromStr,
{
    let s = match data_option {
        Some(Data::String(s)) => s.to_string(),
        Some(Data::Int(i)) => i.to_string(),
        Some(Data::Float(f)) => f.to_string(),
        Some(Data::Bool(b)) => b.to_string(),
        _ => return None,
    };

    // Пытаемся распарсить строку в целевой тип T
    s.parse::<T>().ok()
}

/// **Очищает таблицу**
pub async fn truncate_table(table_name: &str, tx: &mut Transaction<'_, Postgres>) -> anyhow::Result<()> {
    let query = format!("TRUNCATE TABLE {} RESTART IDENTITY CASCADE", table_name);

    sqlx::query(&query)
        .execute(&mut **tx)
        .await?; // __ Если здесь будет ошибка, она уйдет в вызывающий код

    Ok(()) // __ Возвращаем "пустой" успех
}


// __ Проверяет на вшивость структуру Excel файла
pub async fn check_excel_file_structure<T>(
    range: &Range<Data>,
    executor: &PgPool,
) -> anyhow::Result<()>
where
    T: ExcelPattern,
{
    let mut has_error = false;
    let mut context = json!({});

    let check_row = T::get_check_row();

    for (col, expected_name) in T::CHECK_PATTERN {
        if let Some(cell_value) = range.get((check_row, col - 1)) {
            let actual_name = cell_to_string_by_enum(cell_value);

            if !actual_name.eq(expected_name) {
                has_error = true;
                context = json!({
                    "row": check_row,
                    "col": col,
                    "expected": expected_name,
                    "actual": actual_name,
                });
                break;
            }
        } else {
            has_error = true;
            context = json!({
                    "row": check_row,
                    "col": col,
                    "expected": expected_name,
                    "actual": "not found",
                });
            break;
        }
    }
    if has_error {
        let error = T::get_error();
        let mut message = error.get_log_message();
        message.context = Some(Json(context));
        message.write(executor).await?;
        return Err(anyhow::anyhow!(message.message));
    }
    Ok(())
}






