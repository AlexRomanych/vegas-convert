use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Serialize, FromRow)]
pub struct OrderLine {
    id: i64,
    model_code_1c: String,
    size: String,
    width: i16,
    length: i16,
    height: i16,
    amount: i32,
    specification: Option<String>,
}

impl OrderLine {
    // __ Это в code_1c
    const CLIENT_AVERAGE_MATTRESS_PREFIX: &'static str = "CMID_"; // CLIENT MATTRESS ID, должен быть 5 символов
    const CLIENT_AVERAGE_ACCESSORY_PREFIX: &'static str = "CAID_"; // CLIENT ACCESSORY ID, должен быть 5 символов

    // __ Это в имени модели
    // const AVERAGE_M_PREFIX: &'static str = "AVGM_"; // Универсальный префикс для средних значений, должен быть 5 символов
    // const AVERAGE_A_PREFIX: &'static str = "AVGA_"; // Универсальный префикс для средних значений, должен быть 5 символов

    // __ Проверяем, является ли модель в строке Заявки расчетной или нет
    #[rustfmt::skip]
    pub fn is_average(&self) -> bool {
        self.model_code_1c.contains(Self::CLIENT_AVERAGE_MATTRESS_PREFIX) || 
        self.model_code_1c.contains(Self::CLIENT_AVERAGE_ACCESSORY_PREFIX)
    }
}
