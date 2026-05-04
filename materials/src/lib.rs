pub mod structures;

use std::collections::HashMap;
use crate::structures::material::Material;
use anyhow::Result;
use constants::MATERIALS_TABLE_NAME;
use sqlx::PgPool;
use std::sync::OnceLock;

static QUERY_GET_MATERIALS: OnceLock<String> = OnceLock::new();

// __ Получаем все Материалы
pub async fn get_materials() -> Result<HashMap<String, Material>> {
    let pool = database::connect().await?;
    let materials = get_materials_pool(&pool).await?;
    Ok(materials)
}

// __ Получаем процедуры + подключение к БД
pub async fn get_materials_pool(pool: &PgPool) -> Result<HashMap<String, Material>> {
    let query = QUERY_GET_MATERIALS.get_or_init(|| format!("SELECT * FROM {};", MATERIALS_TABLE_NAME));
    let materials_vec = sqlx::query_as::<_, Material>(query.as_str())
        .fetch_all(pool)
        .await?;

    // Превращаем Vec в HashMap
    let materials_map: HashMap<String, Material> = materials_vec
        .into_iter()
        .map(|m| (m.code_1c.clone(), m)) // Предполагаем, что ключ — это m.code_1c
        .collect();

    Ok(materials_map)
}
