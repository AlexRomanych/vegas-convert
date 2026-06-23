use serde::{Deserialize, Serialize};
use crate::structures::cutting_task_line::CuttingTaskLine;
use crate::structures::model::Model;
// use sqlx::FromRow;

// 2. Структура для объектов внутри массива order_lines_data
#[derive(Debug, Serialize, Deserialize)]
pub struct OrderLine {
    pub order_line_id: i64,
    pub model_code_1c: String,
    pub model: Option<Model>,
    pub width: Option<i16>,
    pub length: Option<i16>,
    pub height: Option<i16>,
    pub cutting_task_lines: Option<Vec<CuttingTaskLine>>, // Option на случай, если FILTER вернет null/пустоту
}

impl OrderLine {
    pub fn get_length(&self) -> f64 {
        if let Some(length) = self.length { length as f64 } else { 0.0 }
        // if let Some(length) = self.length { (length as f64) / 100.0 } else { 0.0 }
    }
    pub fn get_width(&self) -> f64 {
        if let Some(width) = self.width { width as f64 } else { 0.0 }
        // if let Some(width) = self.width { (width as f64) / 100.0 } else { 0.0 }
    }
}
