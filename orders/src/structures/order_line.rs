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
}
