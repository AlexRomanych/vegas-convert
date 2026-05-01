//! **Модуль для описания структуры данных процедур.**

use serde::{Deserialize, Serialize};
// use crate::structures::custom_errors::CustomError;
// use crate::structures::traits::ExcelPattern;


/// **Структура процедуры расчета**
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone, Default)]
pub struct Procedure {
    /// **Код 1C**
    pub code_1c: String, // __ Первичный ключ

    /// **Название процедуры**
    pub name: String, // __ Название процедуры

    /// **Текст процедуры**
    pub text: Option<String>, // __ Текст (может быть пустым)

    /// **Текст процедуры, адаптированный под VBA (может быть пустым)**
    #[sqlx(skip)]
    pub text_vba: Option<String>, // __ Адаптированный под VBA (может быть пустым)

    /// **1С: Вид объекта. Код - Код объекта, к которому относится процедура расчета**
    pub object_code_1c: Option<String>, // __

    /// **1С: Вид объекта. Наименование - Наименование объекта, к которому относится процедура расчета**
    pub object_name: Option<String>,
}
//
// impl ModelConstructProcedure {
//     /// **Название таблицы процедур**
//     pub const PROCEDURES_TABLE_NAME: &'static str = "model_construct_procedures";
//
//     /// **Номер строки с заголовками**
//     pub const DATA_CHECK_ROW: usize = 4;
//
//     /// **Номер строки начала данных**
//     pub const DATA_START_ROW: usize = 5;
//
//     /// **Номер столбца с кодом из 1С**
//     pub const CODE_COL: usize = 1;
//
//     /// **Номер столбца с названием из 1С**
//     pub const NAME_COL: usize = 2;
//
//     /// **Номер столбца с текстом процедуры**
//     pub const TEXT_COL: usize = 3;
//
//     /// **Номер столбца с кодом из 1С объекта вида процедуры**
//     pub const OBJECT_CODE_COL: usize = 4;
//
//     /// **Номер столбца с названием объекта вида процедуры (например, "БлокПружинный")**
//     pub const OBJECT_NAME_COL: usize = 5;
// }

// __ Описываем правила для Процедуры расчета
// impl ExcelPattern for ModelConstructProcedure {
//
//     #[rustfmt::skip] // Запрещаем форматеру трогать этот массив
//     const CHECK_PATTERN: &'static [(usize, &'static str)] = &[
//         (ModelConstructProcedure::CODE_COL, "Код"),
//         (ModelConstructProcedure::NAME_COL, "Наименование"),
//         (ModelConstructProcedure::TEXT_COL, "Текст процедуры"),
//         (ModelConstructProcedure::OBJECT_CODE_COL, "Вид объекта.Код"),
//         (ModelConstructProcedure::OBJECT_NAME_COL, "Вид объекта.Наименование"),
//     ];
//
//     fn get_check_row() -> usize {
//         Self::DATA_CHECK_ROW - 1
//     }
//
//     fn get_error() -> CustomError {
//         CustomError::ErrorStructureProceduresFile
//     }
// }
