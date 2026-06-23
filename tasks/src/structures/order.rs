use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use crate::structures::order_line::OrderLine;
// use crate::structures::cutting_task::CuttingTask;

// 1. Корневая структура, которая полностью мэтчится на плоскую строку из SQL
#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Order {
    pub order_id: i64,
    pub cutting_task_id: i64,

    // __ Это поле наполняет sqlx сырым JSON из базы
    pub order_lines_raw: sqlx::types::Json<Vec<OrderLine>>,

    // __ Сюда мы переложим чистый Vec. sqlx пропустит это поле при парсинге строк БД
    #[sqlx(skip)]
    pub order_lines: Vec<OrderLine>,

    // #[sqlx(skip)]
    // pub tasks: Vec<CuttingTask>,
}
