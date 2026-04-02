#![allow(unused)]
use crate::constants::CODE_1C_LENGTH;
use sqlx::{Postgres, Transaction};
use std::str::FromStr;
use calamine::Data;

/// **Возвращает приведенную к нормальному виду строку кода 1С**
pub fn get_formatted_1c_code_string(raw_code: String) -> String {
    if raw_code.is_empty() {
        raw_code // __ возвращает пустую String
    } else {
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
    T: FromStr
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
pub async fn truncate_table(
    table_name: &str,
    tx: &mut Transaction<'_, Postgres>,
) -> anyhow::Result<()> {
    let query = format!("TRUNCATE TABLE {} RESTART IDENTITY CASCADE", table_name);

    sqlx::query(&query).execute(&mut **tx).await?; // __ Если здесь будет ошибка, она уйдет в вызывающий код

    Ok(()) // __ Возвращаем "пустой" успех
}
