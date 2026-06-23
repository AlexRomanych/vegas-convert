//! **Модуль для описания структуры данных процедур.**

use serde::{Deserialize, Serialize};
use crate::structures::procedure::Procedure;
// use crate::structures::custom_errors::CustomError;
// use crate::structures::traits::ExcelPattern;


/// **Структура процедуры расчета**
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone, Default)]
pub struct ProcedureCutting {
    /// **id**
    pub id: i64, // __ Первичный ключ

    /// **Название процедуры**
    pub name: String, // __ Название процедуры

    /// **Текст процедуры**
    pub text: Option<String>, // __ Текст (может быть пустым)

    /// **1С: Вид объекта. Наименование - Наименование объекта, к которому относится процедура расчета**
    pub object_name: Option<String>,

    // #[sqlx(skip)]
    // pub code_1c: String, // __ Первичный ключ
}

// Пишем простую конвертацию для ProcedureCutting
impl ProcedureCutting {
    pub fn into_procedure(self) -> Procedure {
        Procedure {
            code_1c: self.id.to_string(), // Вот тут превращаем id в код 1С
            name: self.name,
            text: self.text,
            text_vba: None,
            object_code_1c: None,
            object_name: self.object_name,
        }
    }
}