// use std::sync::LazyLock;
use sqlx::{Pool, Postgres, /*Error,*/ Transaction};
use anyhow::{/*Context,*/ Result};

use crate::structures::expense_material::ExpenseMaterial;




// **Возвращаем транзакцию**
pub async fn transaction<'a>(pool: &'a Pool<Postgres>) -> Result<Transaction<'a, Postgres>> {
    let tx = pool.begin().await?;
    Ok(tx)
}


/// **Удаляет все записи из пивот-таблицы материалов для заданных ID заказов.**
/// **Принимает срез (slice) интов, что позволяет передавать как Vec<i32>, так и &[i32].**
pub async fn delete_materials_by_order_ids(
    pool: &Pool<Postgres>,
    order_ids: &[i64],
) -> Result<()> {
    let mut tx = pool.begin().await?;
    Ok(delete_materials_by_order_ids_tx(&mut tx, order_ids).await?)
}


/// **Удаляет все записи из пивот-таблицы материалов для заданных ID заказов по транзакции.**
/// **Принимает срез (slice) интов, что позволяет передавать как Vec<i32>, так и &[i32].**
pub async fn delete_materials_by_order_ids_tx(
    tx: &mut Transaction<'_, Postgres>,
    order_ids: &[i64],
) -> Result<()> {

    // __ Если входной список пуст, сразу выходим, чтобы не делать холостой запрос в базу
    if order_ids.is_empty() {
        return Ok(());
    }

    // __ Выполняем удаление через единый SQL-запрос
    // __ Используем ANY($1) для передачи Rust-вектора в SQL-массив Postgres'а
    let query = format!(
        r#"
            DELETE FROM {}
            WHERE order_line_id IN (
                SELECT id
                FROM order_lines
                WHERE order_id = ANY($1)
            )
        "#,
        ExpenseMaterial::EXPENSE_MATERIALS_TABLE_NAME
    );

    let static_query: &'static str = Box::leak(query.into_boxed_str());

    sqlx::query(static_query)
        .bind(order_ids)
        .execute(&mut **tx)
        .await?;

    Ok(())
}


// __ Округление
pub fn round_to_precision(val: f64, precision: u32) -> f64 {
    let base = 10u32.pow(precision);
    (val * base as f64).round() / (base as f64)
}



