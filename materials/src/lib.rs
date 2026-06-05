pub mod structures;

use crate::structures::material::Material;
use anyhow::Result;
use constants::MATERIALS_TABLE_NAME;
use regex::Regex;
use serde_json::Value;
use sqlx::PgPool;
use sqlx::types::Json;
use std::collections::HashMap;
use std::sync::{LazyLock, OnceLock};

static MATERIALS_TABLE: OnceLock<HashMap<String, Material>> = OnceLock::new();
static QUERY_GET_MATERIALS: OnceLock<String> = OnceLock::new();

// __ Получаем все Материалы
pub async fn get_materials() -> Result<HashMap<String, Material>> {
    let pool = database::connect().await?;
    let materials = get_materials_pool(&pool).await?;
    Ok(materials)
}

// __ Получаем процедуры + подключение к БД
pub async fn get_materials_pool(pool: &PgPool) -> Result<HashMap<String, Material>> {
    if let Some(cashed_materials) = MATERIALS_TABLE.get() {
        if cashed_materials.len() != 0 {
            return Ok(cashed_materials.clone());
        }
    }

    let query = QUERY_GET_MATERIALS.get_or_init(|| format!("SELECT * FROM {};", MATERIALS_TABLE_NAME));
    let materials_vec = sqlx::query_as::<_, Material>(query.as_str())
        .fetch_all(pool)
        .await?;

    // Превращаем Vec в HashMap
    let materials_map: HashMap<String, Material> = materials_vec
        .into_iter()
        .map(|mut m| {
            m.set_properties_map();
            m.set_properties_map_numeric();
            (m.code_1c.clone(), m)
        }) // Предполагаем, что ключ — это m.code_1c
        .collect();

    if let Ok(_) = MATERIALS_TABLE.set(materials_map.clone()) {
        Ok(materials_map)
    } else {
        // Если set вернул ошибку, значит мапа уже была инициализирована ранее
        if cfg!(debug_assertions) {
            println!("Ошибка: MATERIALS_TABLE уже заполнена!");
        }
        Ok(MATERIALS_TABLE.get().unwrap().clone())
    }

    //

    //
    // let query = QUERY_GET_MATERIALS.get_or_init(|| format!("SELECT * FROM {};", MATERIALS_TABLE_NAME));
    // let materials_vec = sqlx::query_as::<_, Material>(query.as_str())
    //     .fetch_all(pool)
    //     .await?;
    //
    // // Превращаем Vec в HashMap
    // let materials_map: HashMap<String, Material> = materials_vec
    //     .into_iter()
    //     .map(|m| (m.code_1c.clone(), m)) // Предполагаем, что ключ — это m.code_1c
    //     .collect();
    //
    // Ok(materials_map)
}


// __ Тут мы для оптимизации времени поиска создаем матрешку
// __ У Групп Категорий, Категорий Материалов и у материалов есть свойства (опционально)
// __ по которым и будем искать материал. В Спецификациях попадаются только
// __ сами Материалы и Категории Материалов (Группы не встречаются).
// __ Таким образом, сам Материал будет искаться по свойствам материала, который находится
// __ в соответсвующей ему Категории (родитель), которая и указывается в спецификации
// __ и будем туда запихивать материалы у которых есть свойства
pub async fn get_materials_lookup() -> Result<HashMap<String, HashMap<String, Material>>> {
    let mut materials_lookup: HashMap<String, HashMap<String, Material>> = HashMap::new();

    let materials = get_materials().await?;
    for (code, material) in materials {
        if material.is_material() && material.properties.is_some() {
            if let Some(category_code) = &material.material_category_code_1c {
                // ENTRY API:
                // Ищем список для этой КАТЕГОРИИ. Если его нет — создаем пустой.
                // .or_default() вернет &mut HashMap<String, Material>
                materials_lookup
                    .entry(category_code.clone())
                    .or_default()
                    .insert(code.clone(), material.clone());

                // if let Some(category_list) = materials_lookup.get_mut(category_code) {
                //     category_list.insert(code.clone(), material.clone());
                // } else {
                //     // Метод .entry() находит место в памяти, а .or_insert()
                //     // вставляет значение только если ключа там еще нет.
                //     // В любом случае возвращается &mut HashMap.
                //     let category_data = materials_lookup
                //         .entry(category_code.clone())
                //         .or_insert_with(HashMap::new);
                //     category_data.insert(code.clone(), material.clone());
                // }
            }
        }
    }


    // __ Удаляем пустые группы (их по алгоритму не будет)
    // 1. Очищаем мапу. Retain оставляет только те элементы,
    // для которых условие возвращает true.
    // materials_lookup.retain(|code, category| {
    //     if category.len() == 0 {
    //         if cfg!(debug_assertions) {
    //             println!("Удаляем пустую группу: {code}");
    //         }
    //         false // Удалить
    //     } else {
    //         true // Оставить
    //     }
    // });

    // __ Печатаем результат для дебага
    // if cfg!(debug_assertions) {
    //     materials_lookup
    //         .iter()
    //         .for_each(|(category_code, category_list)| {
    //             println!("{category_code}: -->");
    //             category_list
    //                 .iter()
    //                 .for_each(|(code, material)| {
    //                     println!("{code}: {material:?}");
    //                 });
    //             println!("-----------------------------------------");
    //         });
    //
    //     println!("{}", materials_lookup.len());
    // }

    Ok(materials_lookup)
}



// __ Задаем недостающие свойства материалам
pub fn parse_ppu_density_static(code: &str) -> f64 {
    match code {
        "000042336" => 18.0, //	ППУ 1825 конв толщ 2 см
        "000040710" => 20.0, //	ППУ 2036
        "000000698" => 22.0, //	ППУ 2240
        "000000691" => 25.0, //	ППУ 2540
        "000019784" => 28.0, //	ППУ 2836
        "000035758" => 50.0, //	ППУ 5015
        "000038358" => 35.0, //	ППУ HR(3530) конв толщ 3,2 см
        "000018716" => 35.0, //	ППУ HR3530
        "000039872" => 35.0, //	ППУ HR3530 синебел
        "000019088" => 40.0, //	ППУ HR4026
        "000019108" => 50.0, //	ППУ LL5020
        "000044793" => 50.0, //	ППУ LL5020 Gel
        "000031819" => 40.0, //	ППУ LR4020 толщ 1 см
        "000002062" => 45.0, //	ППУ LR4509 (ViscoGel)
        "000000672" => 50.0, //	ППУ VE5015 толщ 3 см
        "000018641" => 50.0, //	ППУ VE5015 толщ 6 см
        "000044345" => 22.0, //	ППУ деталь 2240 (Sandnes)
        _ => 0.0,
    }
}

// __ Задаем недостающие свойства материалам через динамику
pub fn parse_ppu_density_dynamic(name: &str) -> Option<u32> {
    // Регулярное выражение ищет ПЕРВУЮ встречную группу ровно из 2 цифр (\d{2}),
    // перед которой могут быть буквы марки, но которая является началом блока цифр.
    static PPU_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b[A-Za-zА-Яа-я]*(\d{2})\d*").unwrap());

    // Ищем совпадение и берем первую захваченную группу (капчу)
    PPU_REGEX
        .captures(name)
        .and_then(|cap| cap.get(1))
        .and_then(|m| m.as_str().parse::<u32>().ok())
}

// __ Добавляем недостающие свойства (например плотность в ППУ)
pub fn add_properties(materials: &mut HashMap<String, Material>) {
    for (code_1c, material) in materials.iter_mut() {
        if material.is_category() && material.name.contains("ППУ ") {
            let density = parse_ppu_density_static(code_1c);
            if density != 0.0 {
                // 1. Безопасно получаем мутабельную ссылку на внутреннюю HashMap.
                // Если там был None, get_or_insert_with создаст новую мапу и обернет её в Json.
                let Json(map) = material
                    .properties
                    .get_or_insert_with(|| Json(HashMap::new()));

                // 2. Добавляем новые свойства.
                // Важно: значения должны быть приведены к типу serde_json::Value
                map.insert("Плотность".to_string(), Value::from(density.to_string()));
                map.insert("ВидОбъекта".to_string(), Value::from("НастилМатериалы"));
                
                material.set_properties_map();
                material.set_properties_map_numeric();
            }
        }
    }
}
