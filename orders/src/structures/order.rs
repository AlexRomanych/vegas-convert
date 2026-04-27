use serde::Serialize;
use sqlx::FromRow;
use crate::structures::order_line::OrderLine;

#[derive(Debug, Serialize, FromRow)]
pub struct Order {
    id: i64,

    #[sqlx(skip)] // SQLx пропустит это поле при чтении из таблицы Orders
    pub lines: Vec<OrderLine>,
}
