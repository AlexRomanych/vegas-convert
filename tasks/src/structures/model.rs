use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use sqlx::types::{Decimal /*Json*/};
// use sqlx::FromRow;
// use crate::structures::cutting_task_line::CuttingTaskLine;

// 3. Данные технологических процессов модели
#[derive(Debug, Serialize, Deserialize)]
pub struct Model {
    pub cover_up_proc_id:   i64,
    pub cover_down_proc_id: i64,
    pub side_proc_id:       i64,
    pub name:               String,
    pub angle:              Option<String>,
    pub base_height:        Option<Decimal>,
    pub cover_height:       Option<Decimal>,
}


impl Model {
    // __ Возвращаем высоту МЭ
    pub fn get_base_height(&self) -> f64 {
        self.base_height
            .map(|h| h * rust_decimal::Decimal::from(100)) // Умножаем как Decimal
            .and_then(|h| h.to_f64())
            .unwrap_or(0.0)
    }

    // __ Возвращаем высоту Чехла
    pub fn get_cover_height(&self) -> f64 {
        self.cover_height
            .map(|h| h * rust_decimal::Decimal::from(100)) // Умножаем как Decimal
            .and_then(|h| h.to_f64())
            .unwrap_or(0.0)
    }
}
