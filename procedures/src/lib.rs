use crate::structures::procedure::Procedure;
use anyhow::Result;
use constants::PROCEDURES_TABLE_NAME;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

pub mod structures;

static QUERY_GET_PROCEDURES: OnceLock<String> = OnceLock::new();

// __ Получаем процедуры по списку id(code_1c) без подключения к БД
pub async fn get_procedures_by_list_code_1c(list_code_1c: &HashSet<String>) -> Result<Vec<Procedure>> {
    let pool = database::connect().await?;
    let procedures = get_procedures_by_list_code_1c_pool(&pool, list_code_1c).await?;
    Ok(procedures)
}

// __ Получаем процедуры по списку id(code_1c) + подключение к БД
pub async fn get_procedures_by_list_code_1c_pool(pool: &PgPool, list_code_1c: &HashSet<String>) -> Result<Vec<Procedure>> {
    let list = list_code_1c
        .iter()
        .map(|s| format!("'{}'", s))
        .collect::<Vec<_>>()
        .join(", ");

    let query = QUERY_GET_PROCEDURES.get_or_init(|| format!("SELECT * FROM {} WHERE code_1c IN ({});", PROCEDURES_TABLE_NAME, list));

    let procedures = sqlx::query_as::<_, Procedure>(query.as_str())
        .fetch_all(pool)
        .await?;

    Ok(procedures)
}


// __ Получаем все процедуры
pub async fn get_procedures() -> Result<Vec<Procedure>> {
    let pool = database::connect().await?;
    let procedures = get_procedures_pool(&pool).await?;
    Ok(procedures)
}


// __ Получаем процедуры + подключение к БД
pub async fn get_procedures_pool(pool: &PgPool) -> Result<Vec<Procedure>> {
    let query = QUERY_GET_PROCEDURES.get_or_init(|| format!("SELECT * FROM {};", PROCEDURES_TABLE_NAME));
    let procedures = sqlx::query_as::<_, Procedure>(query.as_str())
        .fetch_all(pool)
        .await?;

    Ok(procedures)
}
