use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::Json;
use std::collections::HashMap;
use crate::structures::custom_errors::CustomError;
use crate::structures::traits::ExcelPattern;


// pub struct MaterialProperty {
//     name: String,
//     value: String,
// }


#[derive(Default, Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Material {
    /// **Связь с Группой материалов - Группа материала (связь с ячейкой этой же таблицы)**
    pub material_group_code_1c: Option<String>,

    /// **Связь с Категорией материалов - Категория материала (связь с ячейкой этой же таблицы)**
    pub material_category_code_1c: Option<String>,

    /// **Код из 1С**
    pub code_1c: String,

    /// **Название материала**
    pub name: String,

    /// **Единица измерения**
    pub unit: Option<String>,

    /// **Поставщик**
    pub supplier: Option<String>,

    /// **Название объекта, к которому принадлежит материал (например, БлокПружинный)**
    pub object_name: Option<String>,

    /// **Сюда попадут все остальные характеристики: Длина, Ширина и т.д.**
    /// **Используем HashMap, чтобы сохранить оригинальные русские названия ключей**
    pub properties: Option<Json<HashMap<String, Value>>>,
}

impl Material {
    /// **Название таблицы процедур**
    pub const MATERIALS_TABLE_NAME: &'static str = "materials";

    // /// **Название файла Excel с материалами**
    // pub const MATERIALS_FILE_NAME: &'static str = "materials.xlsx"; - // __ Перенесли в constants

    /// **Стоп-слово окончания отчета ("Итого")**
    pub const STOP_WORD: &'static str = "Итого";

    /// **Номер строки с заголовками**
    pub const DATA_CHECK_ROW: usize = 6;

    /// **Номер строки начала данных**
    pub const DATA_START_ROW: usize = 7;

    /// **Номер столбца с кодом из 1С Группы материалов**
    pub const GROUP_CODE_COL: usize = 1;

    /// **Номер столбца с названием Группы материалов**
    pub const GROUP_NAME_COL: usize = 4;

    /// **Номер столбца с кодом из 1С Категории материалов**
    pub const CATEGORY_CODE_COL: usize = 6;

    /// **Номер столбца с названием Категории материалов**
    pub const CATEGORY_NAME_COL: usize = 8;

    /// **Номер столбца с кодом из 1С Материала**
    pub const MATERIAL_CODE_COL: usize = 9;

    /// **Номер столбца с названием Материала**
    pub const MATERIAL_NAME_COL: usize = 10;

    /// **Номер столбца с Единицей измерения**
    pub const UNIT_COL: usize = 11;

    /// **Номер столбца с Кодом Названия Вида свойства**
    pub const PROPERTY_NAME_CODE_COL: usize = 12;

    /// **Номер столбца с названием Вида свойства**
    pub const PROPERTY_NAME_COL: usize = 13;

    /// **Номер столбца со значением Вида свойства**
    pub const PROPERTY_VALUE_COL: usize = 14;

    /// **Конструктор**
    pub fn new(code_1c: String, name: String) -> Self {
        Self {
            code_1c,
            name,
            material_group_code_1c: None,
            material_category_code_1c: None,
            unit: None,
            supplier: None,
            object_name: None,
            properties: None,
        }
    }

    /// **Проверяет, является ли объект пустым**
    pub fn is_empty(&self) -> bool {
        self.code_1c.is_empty() && self.name.is_empty()
    }

    /// **Сбрасывает объект**
    pub fn clear(&mut self) {
        self.code_1c = "".to_string();
        self.name = "".to_string();
        self.material_group_code_1c = None;
        self.material_category_code_1c = None;
        self.unit = None;
        self.supplier = None;
        self.object_name = None;
        self.properties = None;
    }
}

// __ Описываем правила для Материала
impl ExcelPattern for Material {
    
    #[rustfmt::skip] // Запрещаем форматеру трогать этот массив
    const CHECK_PATTERN: &'static [(usize, &'static str)] = &[
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

    fn get_check_row() -> usize {
        Self::DATA_CHECK_ROW - 2
    }
    
    fn get_error() -> CustomError {
        CustomError::ErrorStructureMaterialsFile
    }
}



