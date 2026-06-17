use serde::Deserialize;
use sqlx::types::{Decimal, Json};

// Самый нижний уровень: Материал/Процедура
#[derive(Deserialize, Debug, Clone)]
pub struct ItemDetail {
    pub mc: Option<String>, // material_code
    pub pc: Option<String>, // procedure_code
    pub pn: Option<String>, // procedure_name
    pub h:  Option<f64>,    // detail_height (используй f64 для размеров)
    pub a:  Option<f64>,    // amount
    pub u:  Option<String>, // единица измерения
    pub d:  Option<String>, // деталь
    pub p:  Option<i16>,    // position
}

// Средний уровень: Конструкция
#[derive(Deserialize, Debug)]
pub struct Construct {
    pub construct_code: Option<String>,
    pub items:          Option<Vec<ItemDetail>>,
}

// __ Главная структура строки заказа
#[derive(sqlx::FromRow, Debug)]
pub struct OrderProcessRow {
    pub order_id:     i64, // id Заявки к которой относится строка
    pub line_id:      i64,
    pub model_name:   String,
    pub width:        Option<i16>,
    pub length:       Option<i16>,
    pub height:       Option<i16>,
    pub base_height:  Option<Decimal>,
    pub cover_height: Option<Decimal>,
    pub amount:       i32,
    // sqlx::types::Json автоматически десериализует строку из Postgres в Vec<Construct>
    pub base:         Option<Json<Construct>>,
    pub cover:        Option<Json<Construct>>,
}

impl OrderProcessRow {
    // __ Это в code_1c
    // const CLIENT_AVERAGE_MATTRESS_PREFIX: &'static str = "CMID_"; // CLIENT MATTRESS ID, должен быть 5 символов
    // const CLIENT_AVERAGE_ACCESSORY_PREFIX: &'static str = "CAID_"; // CLIENT ACCESSORY ID, должен быть 5 символов

    // __ Это в имени модели
    const AVERAGE_M_PREFIX: &'static str = "AVGM_"; // Универсальный префикс для средних значений, должен быть 5 символов
    const AVERAGE_A_PREFIX: &'static str = "AVGA_"; // Универсальный префикс для средних значений, должен быть 5 символов

    // __ Проверяем, является ли модель в строке Заявки расчетной или нет
    #[rustfmt::skip]
    pub fn is_average(&self) -> bool {
        self.model_name.contains(Self::AVERAGE_M_PREFIX) ||
        self.model_name.contains(Self::AVERAGE_A_PREFIX)
    }

    pub fn get_length(&self) -> f64 {
        if let Some(length) = self.length { (length as f64) / 100.0 } else { 0.0 }
    }
    pub fn get_width(&self) -> f64 {
        if let Some(width) = self.width { (width as f64) / 100.0 } else { 0.0 }
    }
    pub fn get_height(&self) -> f64 {
        if let Some(height) = self.height { (height as f64) / 100.0 } else { 0.0 }
    }
}
