#![allow(unused)]
use crate::constants::{
    DATA_SHEET_1C_NAME, MODEL_COLLECTIONS_TABLE_NAME, MODEL_MANUFACTURE_GROUPS_TABLE_NAME, MODEL_MANUFACTURE_STATUSES_TABLE_NAME,
    MODEL_MANUFACTURE_TYPES_TABLE_NAME, MODEL_TYPES_TABLE_NAME,
};
use crate::helpers::{cell_to_generic, cell_to_string_by_option, get_formatted_1c_code_string};
use crate::structures::model::Model;
use anyhow::{Context, Result};
use calamine::{Reader, Xlsx, open_workbook};
use rust_decimal::Decimal;
use sqlx::{Postgres, Row, Transaction};
use std::collections::HashMap;
use std::fmt::Display;
use std::hash::Hash;
use std::str::FromStr;

pub async fn run(tx: &mut Transaction<'_, Postgres>, path: &str) -> Result<()> {
    // __ Очищает данные и сбрасывает счетчики ID (SERIAL) в начальное состояние
    //truncate_table(Material::MATERIALS_TABLE_NAME, tx).await?;

    // __ Открываем книгу
    let mut workbook: Xlsx<_> = open_workbook(path).with_context(|| format!("Не удалось открыть файл процедур: {}", path))?;

    // __ Получаем лист
    let range = workbook
        .worksheet_range(DATA_SHEET_1C_NAME)
        .with_context(|| {
            // Добавляем контекст к ошибке, если лист не найден или файл поврежден
            format!("Не удалось прочитать лист '{}' в файле 1С", DATA_SHEET_1C_NAME)
        })?;

    // __ Список всех данных - Собираем все сюда
    let mut excel_data: HashMap<String, Model> = HashMap::new();

    // __ Список всех попавшихся Статусов
    let mut statuses_map: HashMap<i64, String> = HashMap::new();

    // __ Список всех попавшихся Коллекций
    let mut collections_map: HashMap<String, String> = HashMap::new();

    // __ Список всех попавшихся Типов изделий
    let mut models_types_map: HashMap<String, String> = HashMap::new();

    // __ Список всех попавшихся Видов производства
    let mut manufacture_types_map: HashMap<String, String> = HashMap::new();

    // __ Список всех попавшихся Групп сортировки производства (FMX, Обшивка-Скрутка, ...)
    let mut groups_map: HashMap<i64, i64> = HashMap::new();

    // __ Чтение Список всех Групп сортировки производства из базы для проверки ключа
    let mut groups_map_base: HashMap<i64, i64> = HashMap::new();
    let select_query = format!("SELECT id FROM {}", MODEL_MANUFACTURE_GROUPS_TABLE_NAME);
    let rows = sqlx::query(&select_query)
        .fetch_all(&mut **tx)
        .await?;
    for row in rows {
        groups_map_base.insert(row.get(0), row.get(0));
    }


    let mut count = 0;

    for row in range
        .rows()
        .skip(Model::DATA_START_ROW - 1)
    {
        if cell_to_string_by_option(row.get(Model::CODE_1C_COL - 1)).is_empty() {
            break;
        }

        let code_1c = get_formatted_1c_code_string(cell_to_string_by_option(row.get(Model::CODE_1C_COL - 1)));

        // __ Парсим Статус (Выпускается, Архив, ...)
        let model_manufacture_status = cell_to_string_by_option(row.get(Model::MODEL_MANUFACTURE_STATUS_COL - 1));
        let model_manufacture_status_id: Option<i64>;
        let model_manufacture_status_name: Option<String>;
        let status = model_manufacture_status.split_once(", ");
        if let Some((id, name)) = status {
            let parsed_id = id.parse::<i64>()?;
            model_manufacture_status_id = Some(parsed_id);
            model_manufacture_status_name = Some(name.to_string());
            statuses_map.insert(parsed_id, name.to_string()); // __ Обновляем список Статусов
        } else {
            model_manufacture_status_id = None;
            model_manufacture_status_name = None;
        }

        // __ Парсим Коллекции
        let model_collection_code_1c = get_formatted_1c_code_string(cell_to_string_by_option(row.get(Model::MODEL_COLLECTION_CODE_1C_COL - 1)));
        let model_collection_name = cell_to_string_by_option(row.get(Model::MODEL_COLLECTION_NAME_COL - 1));
        if !model_collection_code_1c.is_empty() && !model_collection_name.is_empty() {
            collections_map.insert(model_collection_code_1c.clone(), model_collection_name.clone());
        }

        // __ Парсим Типы изделий
        let model_type_code_1c = get_formatted_1c_code_string(cell_to_string_by_option(row.get(Model::MODEL_TYPE_CODE_1C_COL - 1)));
        let model_type_name = cell_to_string_by_option(row.get(Model::MODEL_TYPE_NAME_COL - 1));
        if !model_type_code_1c.is_empty() && !model_type_name.is_empty() {
            models_types_map.insert(model_type_code_1c.clone(), model_type_name.clone());
        }

        // __ Парсим Виды производства
        let model_manufacture_type_code_1c =
            get_formatted_1c_code_string(cell_to_string_by_option(row.get(Model::MODEL_MANUFACTURE_TYPE_CODE_1C_COL - 1)));
        let model_manufacture_type_name = cell_to_string_by_option(row.get(Model::MODEL_MANUFACTURE_TYPE_NAME_COL - 1));
        if !model_manufacture_type_code_1c.is_empty() && !model_manufacture_type_name.is_empty() {
            manufacture_types_map.insert(model_manufacture_type_code_1c.clone(), model_manufacture_type_name.clone());
        }

        let cover_code_1c_str = get_formatted_1c_code_string(cell_to_string_by_option(row.get(Model::COVER_CODE_1C_COL - 1)));
        let cover_code_1c = if !cover_code_1c_str.is_empty() { Some(cover_code_1c_str) } else { None };

        // __ Проверяем внешний ключ на группу сортировки и при необходимости корректируем ее
        let mut model_manufacture_group_id = cell_to_string_by_option(row.get(Model::MODEL_MANUFACTURE_GROUP_ID_COL - 1))
            .parse::<i64>()
            .unwrap_or_default();
        if !groups_map_base.contains_key(&model_manufacture_group_id) {
            model_manufacture_group_id = 0
        }
        groups_map.insert(model_manufacture_group_id, model_manufacture_group_id);

        let lamit_str = cell_to_string_by_option(row.get(Model::LAMIT_COL - 1));
        let lamit = match lamit_str.trim().to_lowercase().as_str() {
            "да" => Some(true),
            "нет" => Some(false),
            _ => None,
        };

        let temp_model = Model {
            code_1c: code_1c.clone(),
            model_manufacture_status_id,
            model_manufacture_status_name,
            model_collection_code_1c: if !model_collection_code_1c.is_empty() {
                Some(model_collection_code_1c)
            } else {
                None
            },
            model_collection_name: if !model_collection_name.is_empty() {
                Some(model_collection_name)
            } else {
                None
            },
            model_type_code_1c: if !model_type_code_1c.is_empty() { Some(model_type_code_1c) } else { None },
            model_type_name: if !model_type_name.is_empty() { Some(model_type_name) } else { None },
            serial: cell_to_generic(row.get(Model::MODEL_SERIAL_COL - 1)),
            name: cell_to_string_by_option(row.get(Model::NAME_COL - 1)),
            name_short: cell_to_generic(row.get(Model::NAME_SHORT_COL - 1)),
            name_common: cell_to_generic(row.get(Model::NAME_COMMON_COL - 1)),
            name_report: cell_to_generic(row.get(Model::NAME_REPORT_COL - 1)),
            cover_code_1c_copy: cover_code_1c.clone(),
            cover_code_1c: None,
            // cover_code_1c,
            cover_name_1c: cell_to_generic(row.get(Model::COVER_NAME_1C_COL - 1)),
            base_height: Decimal::from_str(cell_to_string_by_option(row.get(Model::BASE_HEIGHT_COL - 1)).as_str()).unwrap_or_default(),
            cover_height: Decimal::from_str(cell_to_string_by_option(row.get(Model::COVER_HEIGHT_COL - 1)).as_str()).unwrap_or_default(),
            textile: cell_to_generic(row.get(Model::TEXTILE_COL - 1)),
            textile_composition: cell_to_generic(row.get(Model::TEXTILE_COMPOSITION_COL - 1)),
            cover_type: cell_to_generic(row.get(Model::COVER_TYPE_COL - 1)),
            zipper: cell_to_generic(row.get(Model::ZIPPER_COL - 1)),
            spacer: cell_to_generic(row.get(Model::SPACER_COL - 1)),
            stitch_pattern: cell_to_generic(row.get(Model::STITCH_PATTERN_COL - 1)),
            pack_type: cell_to_generic(row.get(Model::PACK_TYPE_COL - 1)),
            base_composition: cell_to_generic(row.get(Model::BASE_COMPOSITION_COL - 1)),
            side_foam: cell_to_generic(row.get(Model::SIDE_FOAM_COL - 1)),
            base_block: cell_to_generic(row.get(Model::BASE_BLOCK_COL - 1)),
            load: cell_to_generic(row.get(Model::LOAD_COL - 1)),
            guarantee: cell_to_generic(row.get(Model::GUARANTEE_COL - 1)),
            life: cell_to_generic(row.get(Model::LIFE_COL - 1)),
            cover_mark: cell_to_generic(row.get(Model::COVER_MARK_COL - 1)),
            model_mark: cell_to_generic(row.get(Model::MODEL_MARK_COL - 1)),
            model_manufacture_group_id,
            owner: cell_to_generic(row.get(Model::OWNER_COL - 1)),
            lamit,
            sewing_machine: cell_to_generic(row.get(Model::SEWING_MACHINE_COL - 1)),
            kant: cell_to_generic(row.get(Model::KANT_COL - 1)),
            tkch: cell_to_generic(row.get(Model::TKCH_COL - 1)),
            pack_density: Decimal::from_str(cell_to_string_by_option(row.get(Model::PACK_DENSITY_COL - 1)).as_str()).ok(),
            side_height: cell_to_generic(row.get(Model::SIDE_HEIGHT_COL - 1)),
            pack_weight_rb: Decimal::from_str(cell_to_string_by_option(row.get(Model::PACK_WEIGHT_RB_COL - 1)).as_str()).ok(),
            pack_weight_ex: Decimal::from_str(cell_to_string_by_option(row.get(Model::PACK_WEIGHT_EX_COL - 1)).as_str()).ok(),
            model_manufacture_type_code_1c: if !model_manufacture_type_code_1c.is_empty() {
                Some(model_manufacture_type_code_1c)
            } else {
                None
            },
            model_manufacture_type_name: if !model_manufacture_type_name.is_empty() {
                Some(model_manufacture_type_name)
            } else {
                None
            },
            weight: Decimal::from_str(cell_to_string_by_option(row.get(Model::WEIGHT_COL - 1)).as_str()).unwrap_or_default(),
            barcode: cell_to_generic(row.get(Model::BARCODE_COL - 1)),
            kdch: cell_to_generic(row.get(Model::KDCH_COL - 1)),
            // active: false,
            // description: None,
            // comment: None,
            // note: None,
            // status: None,
            // base/: None,
            // cover: None,
            // meta: None,
            // created_at: None,
            // updated_at: None,
        };

        // __ Сохраняем в мапу
        excel_data.insert(code_1c, temp_model);
        count += 1;
    }

    // __ Сбрасываем флаг Active
    reset_active_flag(tx).await?;

    // __ Блок проверок на внешние ключи
    check_entity(tx, collections_map, MODEL_COLLECTIONS_TABLE_NAME).await?; // __ Коллекции
    check_entity(tx, statuses_map, MODEL_MANUFACTURE_STATUSES_TABLE_NAME).await?; // __ Статусы производства
    check_entity(tx, manufacture_types_map, MODEL_MANUFACTURE_TYPES_TABLE_NAME).await?; // __ Типы производства
    check_entity(tx, models_types_map, MODEL_TYPES_TABLE_NAME).await?; // __ Типы изделий

    // __ Только теперь, после всех проверок вставляем в базу
    for (code_1c, temp_model) in excel_data {
        store_item(&temp_model, tx).await?; // __ Сохраняем в бд
    }

    // __ Делаем проверку на то, чтобы существовал элемент чехла (запись) по рекурсивной ссылке cover_code_1c
    // __ Чтение всех Моделей из базы для проверки ключа
    let mut models_map_base: HashMap<String, Option<String>> = HashMap::new();
    let select_query = format!("SELECT code_1c, cover_code_1c_copy FROM {}", Model::MODELS_TABLE_NAME);
    let rows = sqlx::query(&select_query)
        .fetch_all(&mut **tx)
        .await?;
    for row in rows {
        models_map_base.insert(row.get(0), row.get(1));
    }

    // println!("Ключи: {:#?}", models_map_base);

    // __ Обновляем данные по чехлам
    for (code_model, code_cover) in models_map_base.iter() {
        if let Some(code_cover_str) = code_cover {
            if models_map_base.contains_key(code_cover_str) {
                let select_query = format!("UPDATE {} SET cover_code_1c = $1 WHERE code_1c = $2", Model::MODELS_TABLE_NAME);
                sqlx::query(&select_query)
                    .bind(code_cover_str)
                    .bind(code_model)
                    .execute(&mut **tx)
                    .await?;
            }
        }
    }


    // println!("Коллекции {:#?} models", collections_map);
    // println!("Коллекции_Бд {:#?} models", collections_map_base);

    println!("✅ Модели: импортировано {} строк", count);
    Ok(())
}

// ___ Записываем Модель в базу
async fn store_item(model: &Model, tx: &mut Transaction<'_, Postgres>) -> Result<()> {
    // __ Создаем строку запроса динамически
    let query_str = format!(
        r#"
            INSERT INTO {} (
                code_1c, model_manufacture_status_id, model_manufacture_group_id,
                model_collection_code_1c, model_type_code_1c, model_manufacture_type_code_1c,
                cover_code_1c, cover_code_1c_copy, serial, name, name_short, name_common,
                name_report, textile, textile_composition, cover_type, zipper, spacer,
                stitch_pattern, pack_type, base_composition, side_foam, base_block,
                cover_mark, model_mark, owner, sewing_machine, kant, tkch, side_height,
                barcode, base_height, cover_height, pack_density, pack_weight_rb,
                pack_weight_ex, weight, load, guarantee, life, lamit, cover_name_1c, kdch,
                created_at, updated_at
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
                $21, $22, $23, $24, $25, $26, $27, $28, $29, $30,
                $31, $32, $33, $34, $35, $36, $37, $38, $39, $40, $41, $42, $43,
                NOW() AT TIME ZONE 'Europe/Minsk', NOW() AT TIME ZONE 'Europe/Minsk'
            )
            ON CONFLICT (code_1c) DO UPDATE SET
                model_manufacture_status_id     = EXCLUDED.model_manufacture_status_id,
                model_manufacture_group_id      = EXCLUDED.model_manufacture_group_id,
                model_collection_code_1c        = EXCLUDED.model_collection_code_1c,
                model_type_code_1c              = EXCLUDED.model_type_code_1c,
                model_manufacture_type_code_1c  = EXCLUDED.model_manufacture_type_code_1c,
                cover_code_1c                   = EXCLUDED.cover_code_1c,
                cover_code_1c_copy              = EXCLUDED.cover_code_1c_copy,
                serial                          = EXCLUDED.serial,
                name                            = EXCLUDED.name,
                name_short                      = EXCLUDED.name_short,
                name_common                     = EXCLUDED.name_common,
                name_report                     = EXCLUDED.name_report,
                textile                         = EXCLUDED.textile,
                textile_composition             = EXCLUDED.textile_composition,
                cover_type                      = EXCLUDED.cover_type,
                zipper                          = EXCLUDED.zipper,
                spacer                          = EXCLUDED.spacer,
                stitch_pattern                  = EXCLUDED.stitch_pattern,
                pack_type                       = EXCLUDED.pack_type,
                base_composition                = EXCLUDED.base_composition,
                side_foam                       = EXCLUDED.side_foam,
                base_block                      = EXCLUDED.base_block,
                cover_mark                      = EXCLUDED.cover_mark,
                model_mark                      = EXCLUDED.model_mark,
                owner                           = EXCLUDED.owner,
                sewing_machine                  = EXCLUDED.sewing_machine,
                kant                            = EXCLUDED.kant,
                tkch                            = EXCLUDED.tkch,
                side_height                     = EXCLUDED.side_height,
                barcode                         = EXCLUDED.barcode,
                base_height                     = EXCLUDED.base_height,
                cover_height                    = EXCLUDED.cover_height,
                pack_density                    = EXCLUDED.pack_density,
                pack_weight_rb                  = EXCLUDED.pack_weight_rb,
                pack_weight_ex                  = EXCLUDED.pack_weight_ex,
                weight                          = EXCLUDED.weight,
                load                            = EXCLUDED.load,
                guarantee                       = EXCLUDED.guarantee,
                life                            = EXCLUDED.life,
                lamit                           = EXCLUDED.lamit,
                cover_name_1c                   = EXCLUDED.cover_name_1c,
                kdch                            = EXCLUDED.kdch,
                updated_at                      = NOW() AT TIME ZONE 'Europe/Minsk'
        "#,
        Model::MODELS_TABLE_NAME
    );

    // __ Выполняем вставку с обновлением при конфликте
    let result = sqlx::query(&query_str)
        // 1-8: Ключи и связи (Strings & IDs)
        .bind(&model.code_1c) // $1
        .bind(model.model_manufacture_status_id) // $2 (Option<i64>)
        .bind(model.model_manufacture_group_id) // $3 (i64)
        .bind(&model.model_collection_code_1c) // $4 (Option<String>)
        .bind(&model.model_type_code_1c) // $5 (Option<String>)
        .bind(&model.model_manufacture_type_code_1c) // $6 (Option<String>)
        .bind(&model.cover_code_1c) // $7 (Option<String>)
        .bind(&model.cover_code_1c_copy) // $8 (Option<String>)
        // 9-13: Имена и серии
        .bind(&model.serial) // $9
        .bind(&model.name) // $10
        .bind(&model.name_short) // $11
        .bind(&model.name_common) // $12
        .bind(&model.name_report) // $13
        // 14-23: Характеристики чехла и состава
        .bind(&model.textile) // $14
        .bind(&model.textile_composition) // $15
        .bind(&model.cover_type) // $16
        .bind(&model.zipper) // $17
        .bind(&model.spacer) // $18
        .bind(&model.stitch_pattern) // $19
        .bind(&model.pack_type) // $20
        .bind(&model.base_composition) // $21
        .bind(&model.side_foam) // $22
        .bind(&model.base_block) // $23
        // 24-31: Технические пометки и разное
        .bind(&model.cover_mark) // $24
        .bind(&model.model_mark) // $25
        .bind(&model.owner) // $26
        .bind(&model.sewing_machine) // $27
        .bind(&model.kant) // $28
        .bind(&model.tkch) // $29
        .bind(&model.side_height) // $30
        .bind(&model.barcode) // $31
        // 32-37: Decimal (Высоты и Веса)
        .bind(model.base_height) // $32
        .bind(model.cover_height) // $33
        .bind(model.pack_density) // $34
        .bind(model.pack_weight_rb) // $35
        .bind(model.pack_weight_ex) // $36
        .bind(model.weight) // $37
        // 38-41: Числа и Флаги
        .bind(model.load) // $38 (Option<i32>)
        .bind(model.guarantee) // $39 (Option<i32>)
        .bind(model.life) // $40 (Option<i32>)
        .bind(model.lamit) // $41 (Option<bool>)
        .bind(&model.cover_name_1c)
        .bind(&model.kdch)
        .execute(&mut **tx)
        .await;

    match result {
        Ok(res) => {},
        Err(err) => {
            println!("Model: {:#?}", model);
            panic!();
        },
    }

    // *count += 1;

    Ok(())
}

/// ___ Сбрасываем флаг Active в зависимых таблицах
pub async fn reset_active_flag(tx: &mut Transaction<'_, Postgres>) -> Result<()> {
    let table_names: [&str; 4] = [
        MODEL_COLLECTIONS_TABLE_NAME,
        MODEL_MANUFACTURE_STATUSES_TABLE_NAME,
        MODEL_MANUFACTURE_TYPES_TABLE_NAME,
        MODEL_TYPES_TABLE_NAME,
    ];

    for name in table_names {
        let query = format!("UPDATE {} SET active = false", name);
        sqlx::query(&query)
            .execute(&mut **tx)
            .await?;
    }

    Ok(())
}


trait EntityMetadata {
    fn primary_key_name() -> &'static str;
}

// __ Если ключ — String (например, код 1С), ищем по code_1c
impl EntityMetadata for String {
    fn primary_key_name() -> &'static str {
        "code_1c"
    }
}

// __ Если ключ — u32 (или i32/i64), ищем по id
impl EntityMetadata for i64 {
    fn primary_key_name() -> &'static str {
        "id"
    }
}

/// ___ Поверка на существование Сущностей + вставка, если найдена новая Сущность
/// **Проверка на существование Сущностей + вставка/обновление**
async fn check_entity<'a, T>(
    tx: &mut Transaction<'a, Postgres>,
    // Добавляем 'a здесь: данные в мапе должны жить столько же, сколько транзакция
    entity_map: HashMap<T, String>,
    table_name: &str,
) -> Result<()>
where
    T: Eq + Hash + Clone + Send + Sync + EntityMetadata + Display + 'a,
    T: for<'r> sqlx::Decode<'r, Postgres> + sqlx::Type<Postgres> + sqlx::Encode<'a, Postgres>,
{
    let pk = T::primary_key_name();

    // __ Чтение (тут всё обычно)
    let mut entity_map_base: HashMap<T, String> = HashMap::new();
    let select_query = format!("SELECT {}, name FROM {}", pk, table_name);
    let rows = sqlx::query(&select_query)
        .fetch_all(&mut **tx)
        .await?;

    for row in rows {
        entity_map_base.insert(row.get(0), row.get(1));
    }

    // __ Создаем строки-шаблоны ДО начала цикла + замораживаем жестко, потому что происходит борьба
    // __ с компилятором в асинхронной функции с await и времени жизни ссылки на строку запроса
    let update_with_name_sql: &'static str =
        Box::leak(format!("UPDATE {} SET active = true, name = $1 WHERE {} = $2", table_name, pk).into_boxed_str());
    let insert_sql: &'static str = Box::leak(
        format!(
            r#"
                INSERT INTO {} ({}, name, active, updated_at)
                VALUES ($1, $2, true, NOW() AT TIME ZONE 'Europe/Minsk')
            "#,
            table_name, pk
        )
        .into_boxed_str(),
    );

    // __ Цикл обновления
    for (key, name) in entity_map {
        if let Some(_ /*base_name*/) = entity_map_base.get(&key) {
            sqlx::query(update_with_name_sql) // Теперь ссылка &sql живет до конца итерации
                .bind(name)
                .bind(key)
                .execute(&mut **tx)
                .await?; // await успеет отработать, пока 'sql' еще в памяти
        } else {
            sqlx::query(insert_sql)
                .bind(key)
                .bind(name)
                .execute(&mut **tx)
                .await?;
        }
    }

    Ok(())
}


// ___ Поверка на существование Коллекций + вставка, если найдена новая Коллекция
// async fn check_collections(tx: &mut Transaction<'_, Postgres>, collections_map: HashMap<String, String>) -> Result<()> {
//     // __ Список Коллекций в БД
//     let mut collections_map_base: HashMap<String, String> = HashMap::new();
//
//     // __ Коллекции
//     let query = format!("SELECT code_1c, name FROM {}", MODEL_COLLECTIONS_TABLE_NAME);
//     let rows = sqlx::query(&query)
//         .fetch_all(&mut **tx)
//         .await?;
//
//     for row in rows {
//         collections_map_base.insert(row.get("code_1c"), row.get("name"));
//     }
//
//     for (code_1c, name) in &collections_map {
//         if collections_map_base.contains_key(code_1c) {
//             // __ Устанавливаем флаг Active, если запись существует
//             let query = format!("UPDATE {} SET active = true WHERE code_1c = '{}'", MODEL_COLLECTIONS_TABLE_NAME, code_1c);
//             sqlx::query(&query)
//                 .execute(&mut **tx)
//                 .await?;
//             if !name.eq(collections_map_base
//                 .get(code_1c)
//                 .unwrap())
//             {
//                 // __ Если имена различаются, обновляем еще и имя коллекции
//                 let query = format!(
//                     "UPDATE {} SET name = {} WHERE code_1c = '{}'",
//                     MODEL_COLLECTIONS_TABLE_NAME, name, code_1c
//                 );
//                 sqlx::query(&query)
//                     .execute(&mut **tx)
//                     .await?;
//             }
//         } else {
//             // __ Вставляем запись, если запись не существует
//
//             // 1. Убираем вообще всё лишнее, оставляем только проблемное поле
//             let query_str = format!(
//                 "INSERT INTO {} (code_1c, name, updated_at) VALUES ($1, $2, NOW()) RETURNING updated_at",
//                 MODEL_COLLECTIONS_TABLE_NAME
//             );
//
//             // 2. Используем fetch_one, чтобы дождаться ответа от базы
//             match sqlx::query_scalar::<_, chrono::NaiveDateTime>(&query_str)
//                 .bind(&code_1c)
//                 .bind(&name)
//                 .fetch_one(&mut **tx)
//                 .await
//             {
//                 Ok(db_time) => println!("УСПЕХ! База записала время: {:?}", db_time),
//                 Err(e) => {
//                     println!("ОШИБКА SQL: {:?}", e);
//                     // Если здесь будет ошибка "column updated_at does not exist" или типа того - мы нашли вора.
//                 },
//             }
//         }
//     }
//
//     Ok(())
// }
