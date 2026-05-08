use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::Json;
use std::collections::{BTreeMap, BTreeSet, HashMap};


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

    // __ Свойства в упорядоченном дереве для максимально быстрого поиска
    #[sqlx(skip)]
    properties_tree_map: Option<BTreeMap<String, String>>,
    // properties_tree_map: Option<HashMap<String, String>>,

    // __ Тут только те Свойства, у которых значение можно представить в f64
    #[sqlx(skip)]
    pub properties_map_numeric: Option<HashMap<String, f64>>,
}


impl Material {
    pub fn set_properties_map(&mut self) -> Option<BTreeMap<String, String>> {
        if let Some(properties) = &self.properties_tree_map {
            return Some(properties.clone());
        }

        // 1. Используем паттерн-матчинг, чтобы "пробиться" через Option и Json обертку
        if let Some(Json(props)) = &self.properties {
            let tree_map: BTreeMap<String, String> = props
                .iter()
                .map(|(k, v)| {
                    // 2. Превращаем Value в "чистую" строку без лишних кавычек
                    let val_str = match v {
                        Value::String(s) => s.clone(), // Для строк берем содержимое
                        Value::Number(n) => n.to_string(),
                        Value::Bool(b) => b.to_string(),
                        Value::Null => "".to_string(), // В 1С Null часто пустая строка
                        _ => v.to_string(),            // Массивы/объекты как JSON-текст
                    };
                    (k.clone(), val_str)
                })
                .collect();

            self.properties_tree_map = Some(tree_map.clone());
            return Some(tree_map);
        }

        None
    }

    pub fn set_properties_map_numeric(&mut self) -> Option<HashMap<String, f64>> {
        if let Some(properties) = &self.properties_map_numeric {
            return Some(properties.clone());
        }

        if let Some(Json(props)) = &self.properties {
            let map: HashMap<String, f64> = props
                .iter()
                .filter_map(|(k, v)| {
                    // Пытаемся получить числовое значение напрямую или через парсинг строки
                    let numeric_val = match v {
                        Value::Number(n) => n.as_f64(),
                        Value::String(s) => s.parse::<f64>().ok(), // Пробуем распарсить строку "123.45"
                        // Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }), // Истина = 1, Ложь = 0 (как в 1С)
                        _ => None, // Массивы, объекты и Null игнорируем
                    };

                    // Если число получено, возвращаем кортеж для HashMap, иначе — пропускаем
                    numeric_val.map(|val| (k.clone(), val))
                })
                .collect();

            if !map.is_empty() {
                self.properties_map_numeric = Some(map.clone());
                return Some(map);
            }
        }

        None
    }


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
