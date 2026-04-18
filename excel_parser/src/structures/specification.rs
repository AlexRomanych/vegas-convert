use rust_decimal::Decimal;
use sqlx::FromRow;
use crate::structures::custom_errors::CustomError;
use crate::structures::traits::ExcelPattern;

#[derive(Debug, FromRow, serde::Serialize, serde::Deserialize)]
pub struct ModelConstruct {
    #[sqlx(rename = "code_1c")]
    pub code_1c: String, // __ Код из 1С
    pub name: String,         // __ Название спецификации
    pub model_code_1c: String,         // Relations: Связь с Моделью
    pub model_name: String,         // __ Название модели (избыточно, так как есть в модели, пока оставляем для информативности)
    pub element_type: Option<String>, // __ Тип спецификации (чехол, мэ и т.д.) /pub r#type: Option<String>, // r# используется, так как type — ключевое слово в Rust/
}

#[derive(Debug, FromRow, serde::Serialize, serde::Deserialize)]
pub struct ModelConstructItem {
    pub id: i64, // __ Идентификатор

    // __ Связи (все nullable в миграции, значит везде Option)
    pub construct_code_1c: String,         // Relations: Связь со Спецификацией
    pub material_code_1c: Option<String>, // Relations: Связь с Материалами
    pub material_code_1c_copy: String,         // __ Копия кода 1С Материала (При обновлении Материала и каскадном nullOnDelete, след материала)

    // __ NOT NULL в миграции
    pub material_name: String, // __ Название материала (избыточно, так как есть в материале, пока оставляем)

    pub material_unit: Option<String>, // __ Единица измерения (избыточно, так как есть в материале, пока оставляем)
    pub detail: Option<String>, // __ Деталь

    pub procedure_code_1c: Option<String>, // Relations: Связь с Процедурой расчета
    pub procedure_code_1c_copy: Option<String>, // __ Копия кода 1С Процедуры расчета (При обновлении Процедуры расчета и каскадном nullOnDelete, след процедуры)
    pub procedure_name: Option<String>, // __ Название процедуры расчета (избыточно, так как есть в Процедуре, пока оставляем)

    // __ Используем Decimal для точности (detail_height, amount)
    pub detail_height: Option<Decimal>, // __ Высота детали
    pub count: Option<Decimal>, // __ Количество

    // __ Но SQLx для SMALLINT в Postgres требует i16
    pub position: Option<i16>, // __ Позиция
}

// __ Пропущенный материал при заполнении спецификаций
// #[derive(Debug)]
pub struct MissingMaterial {
    pub code_1c: String,
    pub name_1c: String,
    pub unit: Option<String>,
}


impl ModelConstruct {
    /// **Название таблицы спецификаций**
    pub const CONSTRUCT_TABLE_NAME: &'static str = "model_constructs";

    /// **Номер строки с заголовками**
    pub const DATA_CHECK_ROW: usize = 1;

    /// **Номер строки начала данных**
    pub const DATA_START_ROW: usize = 2;

    pub const MODEL_NAME_COL: usize = 1;
    pub const MODEL_CODE_1C_COL: usize = 2;
    pub const SPECIFICATION_NAME_COL: usize = 3;
    pub const SPECIFICATION_CODE_1C_COL: usize = 4;
    pub const SPECIFICATION_ACTIVITY_COL: usize = 5;
}

impl ModelConstructItem {
    /// **Название таблицы записей (строк) спецификаций**
    pub const CONSTRUCT_ITEM_TABLE_NAME: &'static str = "model_construct_items";

    pub const MATERIAL_CODE_1C_COL: usize = 6;
    pub const MATERIAL_NAME_COL: usize = 7;
    pub const MATERIAL_UNIT_COL: usize = 8;
    pub const SPECIFICATION_DETAIL_TYPE_COL: usize = 9;
    pub const SPECIFICATION_DETAIL_HEIGHT_COL: usize = 10;
    pub const SPECIFICATION_PROCEDURE_CODE_1C_COL: usize = 11;
    pub const SPECIFICATION_PROCEDURE_NAME_COL: usize = 12;
    pub const MATERIAL_COUNT_COL: usize = 13;
    pub const SPECIFICATION_LINE_POSITION_COL: usize = 15;
}


// __ Описываем правила для Спецификаций
impl ExcelPattern for ModelConstruct {
    #[rustfmt::skip] // Запрещаем форматеру трогать этот массив
    const CHECK_PATTERN: &'static [(usize, &'static str)] = &[
        (ModelConstruct::MODEL_NAME_COL,                            "Ссылка.Выходные изделия.Модель.Наименование"),
        (ModelConstruct::MODEL_CODE_1C_COL,                         "Ссылка.Выходные изделия.Модель.Код"),
        (ModelConstruct::SPECIFICATION_NAME_COL,                    "Ссылка.Наименование"),
        (ModelConstruct::SPECIFICATION_CODE_1C_COL,                 "Ссылка.Код"),
        (ModelConstruct::SPECIFICATION_ACTIVITY_COL,                "Ссылка.Активная"),
        (ModelConstructItem::MATERIAL_CODE_1C_COL,                  "Номенклатура.Код"),
        (ModelConstructItem::MATERIAL_NAME_COL,                     "Номенклатура.Наименование"),
        (ModelConstructItem::MATERIAL_UNIT_COL,                     "Единица измерения"),
        (ModelConstructItem::SPECIFICATION_DETAIL_TYPE_COL,         "Деталь"),
        (ModelConstructItem::SPECIFICATION_DETAIL_HEIGHT_COL,       "Высота детали"),
        (ModelConstructItem::SPECIFICATION_PROCEDURE_CODE_1C_COL,   "Процедура расчета.Код"),
        (ModelConstructItem::SPECIFICATION_PROCEDURE_NAME_COL,      "Процедура расчета.Наименование"),
        (ModelConstructItem::MATERIAL_COUNT_COL,                    "Количество"),
        (ModelConstructItem::SPECIFICATION_LINE_POSITION_COL,       "Номер строки"),
    ];

    fn get_check_row() -> usize {
        Self::DATA_CHECK_ROW - 1
    }

    fn get_error() -> CustomError {
        CustomError::ErrorStructureSpecificationsFile
    }
}




