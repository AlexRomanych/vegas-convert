use crate::structures::order::Order;
use crate::structures::order_line::OrderLine;
use constants::{ORDERS_TABLE_NAME, ORDER_LINES_TABLE_NAME};
use std::sync::OnceLock;
use anyhow::{ Result, Context};
use crate::structures::parsed_tree::OrderProcessRow;
// use sqlx::{PgPool, Postgres, Transaction};

pub mod structures;

// __ Объявляем статическую переменную, чтобы избавиться от проблем с lifetime
static SQL_QUERY_ORDERS: OnceLock<String> = OnceLock::new();
static SQL_QUERY_ORDER_LINES: OnceLock<String> = OnceLock::new();

// __ Получаем Заявку с содержимым (строками)
pub async fn get_order_with_lines(pool: &sqlx::PgPool, order_id: i64) -> Result<Order> {

    // __ Получаем строку из OnceLock. Если там пусто — инициализируем.
    let query = SQL_QUERY_ORDERS.get_or_init(|| {
        format!(r#"SELECT id FROM {} WHERE id = $1"#, ORDERS_TABLE_NAME)
    });

    // __ 1. Получаем сам заказ
    let mut order: Order = sqlx::query_as(query.as_str())
        .bind(order_id)
        .fetch_one(pool)
        .await?;
        // .context("Ошибка Заказа")?;

    let query = SQL_QUERY_ORDER_LINES.get_or_init(|| {
        format!(r#"SELECT id, model_code_1c, size, width, length, height, amount FROM {} WHERE order_id = $1"#, ORDER_LINES_TABLE_NAME)
    });

    // __ 2. Получаем все строки этого заказа
    let lines: Vec<OrderLine> = sqlx::query_as(query.as_str())
        .bind(order_id)
        .fetch_all(pool)
        .await
        .context("Ошибка Содержимого Заказа")?;

    // __ 3. Соединяем
    order.lines = lines;

    Ok(order)
}


// __ Получаем структуру для парсинга заявки на материалы
pub async fn get_order_data_tree(pool: &sqlx::PgPool, order_id: i64) -> Result<Vec<OrderProcessRow>> {
    let rows = sqlx::query_as::<_, OrderProcessRow>(
    r#"
            SELECT 
                -- o.id AS order_id, 
                ol.id AS line_id,
                ol.width AS width,
                ol.length AS length,
                ol.height AS height,
                ol.amount AS amount,
                m.code_1c AS model_code,
                m.name AS model_name,
                m.base_height AS base_height,
                m.cover_height AS cover_height,
                -- 1. БАЗОВАЯ ЧАСТЬ (base)
                (
                    SELECT JSON_AGG(JSON_BUILD_OBJECT(
                        'construct_code', mc.code_1c,
                        -- 'construct_name', mc.model_name,
                        'items', (
                            SELECT JSON_AGG(JSON_BUILD_OBJECT(
                                'm_c', mci.material_code_1c,
                                -- 'material', mci.material_name,
                                'p_c', mci.procedure_code_1c,
                                -- 'proc', mci.procedure_name,
                                'h', mci.detail_height,
                                'a', mci.amount
                            ))
                            FROM model_construct_items mci
                            WHERE mci.construct_code_1c = mc.code_1c
                            -- AND mci.material_code_1c IS NOT NULL
                        )
                    ))
                    FROM model_constructs mc
                    WHERE mc.model_code_1c = m.code_1c
                ) AS base,
                -- 2. ОБЛИЦОВКА (cover) - выполняется только если cover_code_1c не null
                (
                    SELECT JSON_AGG(JSON_BUILD_OBJECT(
                        'construct_code', mcc.code_1c,
                        -- 'construct_name', mcc.model_name,
                        'items', (
                            SELECT JSON_AGG(JSON_BUILD_OBJECT(
                                'm_c', mcic.material_code_1c,
                                -- 'material', mcic.material_name,
                                'p_c', mcic.procedure_code_1c,
                                -- 'proc', mcic.procedure_name,
                                'h', mcic.detail_height,
                                'a', mcic.amount
                            ))
                            FROM model_construct_items mcic
                            WHERE mcic.construct_code_1c = mcc.code_1c
                            -- AND mcic.material_code_1c IS NOT NULL
                        )
                    ))
                    FROM models m_cover
                    JOIN model_constructs mcc ON mcc.model_code_1c = m_cover.code_1c
                    WHERE m_cover.code_1c = m.cover_code_1c
                ) AS cover
            FROM orders AS o
            JOIN order_lines ol ON ol.order_id = o.id
            JOIN models m ON m.code_1c = ol.model_code_1c
            WHERE o.id = $1
            ORDER BY ol.id;
        "#
    )
        .bind(order_id)
        .fetch_all(pool)
        .await?;

    Ok(rows)


}

