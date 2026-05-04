use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::Json;
use std::collections::HashMap;


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
    // pub supplier: Option<String>,

    /// **Название объекта, к которому принадлежит материал (например, БлокПружинный)**
    pub object_name: Option<String>,

    /// **Сюда попадут все остальные характеристики: Длина, Ширина и т.д.**
    /// **Используем HashMap, чтобы сохранить оригинальные русские названия ключей**
    pub properties: Option<Json<HashMap<String, Value>>>,
}


impl Material {

    // __ Проверка на то, что это базовый Материал
    pub fn is_material(&self) -> bool {
        self.material_group_code_1c.is_some() && self.material_category_code_1c.is_some()
    }

    // __ Проверка на то, что это Категория
    pub fn is_category(&self) -> bool {
        self.material_group_code_1c.is_some() && self.material_category_code_1c.is_none()
    }

    // __ Проверка на то, что это Группа
    pub fn is_group(&self) -> bool {
        self.material_group_code_1c.is_none() && self.material_category_code_1c.is_none()
    }
}
