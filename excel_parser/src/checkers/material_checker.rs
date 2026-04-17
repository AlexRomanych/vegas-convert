use calamine::{Data, Range};
use sqlx::{PgPool};
use crate::helpers::{cell_to_string_by_enum};
use crate::structures::custom_errors::CustomError;
use crate::structures::material::Material;
use anyhow::{Result};
use serde_json::json;
use sqlx::types::Json;

#[rustfmt::skip] // Запрещаем форматеру трогать этот массив
const CHECK_FIELDS: &[(usize, &str)] = &[
    (Material::GROUP_CODE_COL,      "Родитель.Родитель.Код"),           // **Номер столбца с кодом из 1С Группы материалов**
    (Material::GROUP_NAME_COL,      "Родитель.Родитель.Наименование"),  // **Номер столбца с названием Группы материалов**
    (Material::CATEGORY_CODE_COL,   "Родитель.Код"),                    // **Номер столбца с кодом из 1С Категории материалов**
    (Material::CATEGORY_NAME_COL,   "Родитель.Наименование"),           // **Номер столбца с названием Категории материалов**
    (Material::MATERIAL_CODE_COL,   "Код"),                             // **Номер столбца с кодом из 1С Материала**
    (Material::MATERIAL_NAME_COL,   "Наименование"),                    // **Номер столбца с названием Материала**
    (Material::UNIT_COL,            "Единица измерения"),               // **Номер столбца с Единицей измерения**
    (Material::PROPERTY_NAME_COL,   "Вид свойства"),                    // **Номер столбца с названием Вида свойства**
    (Material::PROPERTY_VALUE_COL,  "Значение"),                        // **Номер столбца со значением Вида свойства**
];


// __ Проверяет на вшивость структуру Excel файла Материалов
pub async fn check_materials_file_structure(range: &Range<Data>, executor: &PgPool) -> Result<()> {
    let mut has_error= false;
    let mut context = json!({});

    for (col, expected_name) in CHECK_FIELDS {

        if let Some(cell_value) = range.get((Material::DATA_CHECK_ROW - 2, col - 1)) {

            let actual_name = cell_to_string_by_enum(cell_value);

            if !actual_name.eq(expected_name) {
                has_error = true;
                context = json!({
                    "row": Material::DATA_CHECK_ROW,
                    "col": col,
                    "expected": expected_name,
                    "actual": actual_name,
                });
                break;
            }
        } else {
            has_error = true;
            context = json!({
                    "row": Material::DATA_CHECK_ROW,
                    "col": col,
                    "expected": expected_name,
                    "actual": "not found",
                });
            break;
        }

    }
    if has_error {
        let error = CustomError::ErrorStructureMaterialsFile;
        let mut message = error.get_log_message();
        message.context = Some(Json(context));
        message.write(executor).await?; // ok() игнорирует ошибку записи лога, чтобы не плодить вложенные Result// КРИТИЧНО: Конвертируем CustomError в anyhow::Error
        // println!("Message {:#?}", message);
    }
    Ok(())
}
