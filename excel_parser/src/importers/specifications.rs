// #![allow(unused)]

use crate::constants::{
    DATA_SHEET_1C_NAME, MISSING_MATERIALS_CATEGORY_CODE_1C, MISSING_MATERIALS_CATEGORY_NAME, MISSING_MATERIALS_GROUP_CODE_1C,
    MISSING_MATERIALS_GROUP_NAME, PRODUCTION,
};
use crate::helpers::{
    cell_to_generic, cell_to_string_by_option, check_excel_file_structure, get_formatted_1c_code_string, get_formatted_unit_string, truncate_table,
};
use crate::importers::materials;
use crate::structures::material::Material;
use crate::structures::model::Model;
use crate::structures::procedure::ModelConstructProcedure;
use crate::structures::specification::{MissingMaterial, ModelConstruct, ModelConstructItem};
use anyhow::{Context, Result};
use calamine::{Reader, Xlsx, open_workbook};
use rust_decimal::Decimal;
use sqlx::{PgPool, Postgres, Row, Transaction};
use std::collections::HashMap;
use std::str::FromStr;

pub async fn run(tx: &mut Transaction<'_, Postgres>, path: &str, pool_executor: &PgPool) -> Result<()> {
    // __ Очищает данные и сбрасывает счетчики ID (SERIAL) в начальное состояние в таблице спецификаций и их зависимостей
    truncate_table(ModelConstruct::CONSTRUCT_TABLE_NAME, tx).await?;
    truncate_table(ModelConstructItem::CONSTRUCT_ITEM_TABLE_NAME, tx).await?;

    // __ Открываем книгу
    let mut workbook: Xlsx<_> = open_workbook(path).with_context(|| format!("Не удалось открыть файл процедур: {}", path))?;

    // __ Получаем лист
    let range = workbook
        .worksheet_range(DATA_SHEET_1C_NAME)
        .with_context(|| {
            // Добавляем контекст к ошибке, если лист не найден или файл поврежден
            format!("Не удалось прочитать лист '{}' в файле 1С {}", DATA_SHEET_1C_NAME, path)
        })?;

    // __ Проверяем на правильную структуру отчета
    check_excel_file_structure::<ModelConstruct>(&range, pool_executor).await?;

    let mut count = 0;

    // !!! Перед чтением материалов из базы удаляем в Группе и Категории пропущенных
    // __ Проверяем на наличие в таблице материалов Группы и Категории пропущенных
    check_or_insert_missing_materials_group_and_category(tx).await?;

    // __ Удаляем все материалы в Группе и Категории пропущенных
    delete_missing_materials(tx).await?;

    // __ Получаем мапы для сущностей, на которые ссылается таблица со спецификациями для проверки внешних ключей
    let materials_base_map = get_entity(tx, Material::MATERIALS_TABLE_NAME).await?;
    let models_base_map = get_entity(tx, Model::MODELS_TABLE_NAME).await?;
    let procedures_base_map = get_entity(tx, ModelConstructProcedure::PROCEDURES_TABLE_NAME).await?;

    // __ Материалы в спецификациях, которых нет в таблице материалов
    let mut missing_materials: HashMap<String, MissingMaterial> = HashMap::new();
    // let find = procedures_base_map.get("000000086");

    // __ Условие выхода из цикла
    const EMPTY_COUNT_LIMIT: i32 = 200;
    let mut empty_count = 0;

    // __ Создаем итератор явно
    let mut rows_iter = range
        .rows()
        .skip(ModelConstruct::DATA_START_ROW - 1)
        .peekable();

    let mut model_code_1c = "".to_string();
    let mut model_name;
    let mut specification_name = "".to_string();
    let mut specification_code_1c = "".to_string();
    // let mut active: String;

    // __ Используем while, чтобы иметь доступ к rows_iter внутри тела
    while let Some(row) = rows_iter.next() {
        model_name = cell_to_string_by_option(row.get(ModelConstruct::MODEL_NAME_COL - 1));

        if model_name.is_empty() {
            if empty_count > EMPTY_COUNT_LIMIT {
                break;
            }
            empty_count += 1;
            continue;
        }
        empty_count = 0; // Сбрасываем счетчик, если нашли данные

        // __ Проверка на существование модели
        if let Some(row) = rows_iter.next() {
            model_code_1c = get_formatted_1c_code_string(cell_to_string_by_option(row.get(ModelConstruct::MODEL_CODE_1C_COL - 1)));
            if !models_base_map.contains_key(&model_code_1c) {
                continue;
            }
        }

        // __ Получаем название спецификации
        if let Some(row) = rows_iter.next() {
            specification_name = cell_to_string_by_option(row.get(ModelConstruct::SPECIFICATION_NAME_COL - 1));
            if specification_name.is_empty() {
                continue;
            }
        }

        // __ Получаем код 1С спецификации
        if let Some(row) = rows_iter.next() {
            specification_code_1c = get_formatted_1c_code_string(cell_to_string_by_option(row.get(ModelConstruct::SPECIFICATION_CODE_1C_COL - 1)));
            if specification_code_1c.is_empty() {
                continue;
            }
        }

        // __ Проверяем, что спецификация активна
        if let Some(row) = rows_iter.next() {
            let active = cell_to_string_by_option(row.get(ModelConstruct::SPECIFICATION_ACTIVITY_COL - 1))
                .trim()
                .to_lowercase();
            if !active.eq("да") {
                continue;
            }
        }

        count += 1;

        let specification = ModelConstruct {
            code_1c:       specification_code_1c.clone(),
            name:          specification_name.clone(),
            model_code_1c: model_code_1c.clone(),
            model_name:    model_name.clone(),
            element_type:  None, // TODO Разбивать  на типы
        };

        // __ Сохраняем спецификацию
        store_specification(specification, tx).await?;

        // __ Если вдруг там дальше нет материалов и пустота - выходим в начало цикла
        if let Some(next_row) = rows_iter.peek() {
            if cell_to_string_by_option(next_row.get(ModelConstructItem::MATERIAL_CODE_1C_COL - 1)).is_empty() {
                continue;
            }
        }

        // __ Собираем содержимое спецификации
        while let Some(row) = rows_iter.next() {
            let item_code_1c = get_formatted_1c_code_string(cell_to_string_by_option(row.get(ModelConstructItem::MATERIAL_CODE_1C_COL - 1)));
            let item_name = cell_to_string_by_option(row.get(ModelConstructItem::MATERIAL_NAME_COL - 1));
            let item_unit = get_formatted_unit_string(cell_to_string_by_option(row.get(ModelConstructItem::MATERIAL_UNIT_COL - 1)));
            let item_detail: Option<String> = cell_to_generic(row.get(ModelConstructItem::SPECIFICATION_DETAIL_TYPE_COL - 1));
            let item_height = Decimal::from_str(cell_to_string_by_option(row.get(ModelConstructItem::SPECIFICATION_DETAIL_HEIGHT_COL - 1)).as_str());
            let item_proc_code_1c = get_formatted_1c_code_string(cell_to_string_by_option(
                row.get(ModelConstructItem::SPECIFICATION_PROCEDURE_CODE_1C_COL - 1),
            ));
            let item_proc_name: Option<String> = cell_to_generic(row.get(ModelConstructItem::SPECIFICATION_PROCEDURE_NAME_COL - 1));
            let item_count = Decimal::from_str(cell_to_string_by_option(row.get(ModelConstructItem::MATERIAL_COUNT_COL - 1)).as_str());
            let item_position = cell_to_generic(row.get(ModelConstructItem::SPECIFICATION_LINE_POSITION_COL - 1));

            // __ Запоминаем материал, которого нет в таблице материалов
            let mut material_code_1c = Some(item_code_1c.clone());
            let material_unit = if !item_unit.is_empty() { Some(item_unit.clone()) } else { None };

            // __ Проверка на существование Материала
            if !materials_base_map.contains_key(&item_code_1c) {
                let missing_material = MissingMaterial {
                    // code_1c: item_code_1c.clone(),
                    name_1c: item_name.clone(),
                    unit:    material_unit.clone(),
                };
                material_code_1c = None;

                if !(item_name.contains("Чехол ") || item_name.contains("ФЧ.")) {
                    missing_materials.insert(item_code_1c.clone(), missing_material);
                }
            }

            // __ Проверка на существование Процедуры
            let mut procedure_code_1c: Option<String> = None;
            if !item_proc_code_1c.is_empty() && procedures_base_map.contains_key(&item_proc_code_1c) {
                procedure_code_1c = Some(item_proc_code_1c.clone());
            }

            // let procedure_code_1c = if !procedures_base_map.get(&item_proc_code_1c).is_none() {
            //     Some(item_proc_code_1c.clone())
            // } else {
            //     println!("Missing procedure code 1C: {}", item_proc_code_1c);
            //     None
            // };

            let specification_item = ModelConstructItem {
                id: 0,
                construct_code_1c: specification_code_1c.clone(),
                material_code_1c,
                material_code_1c_copy: item_code_1c,
                material_name: item_name,
                material_unit,
                detail: item_detail,
                procedure_code_1c,
                procedure_code_1c_copy: if !item_proc_code_1c.is_empty() {
                    Some(item_proc_code_1c.clone())
                } else {
                    None
                },
                procedure_name: item_proc_name,
                detail_height: item_height.ok(),
                count: item_count.ok(),
                position: item_position,
            };

            // __ Сохраняем запись
            store_specification_item(specification_item, tx).await?;

            // __ Проверка на конец содержимого
            if let Some(next_row) = rows_iter.peek() {
                if cell_to_string_by_option(next_row.get(ModelConstructItem::MATERIAL_CODE_1C_COL - 1)).is_empty() {
                    break;
                }
            }
        }
    }

    // __ Вставляем в БД пропущенные материалы
    for (code_1c, missing_material) in missing_materials {
        let store_material = Material {
            // material_group_code_1c: None, // __ Тут именно None
            material_group_code_1c: Some(MISSING_MATERIALS_GROUP_CODE_1C.to_string()),
            material_category_code_1c: Some(MISSING_MATERIALS_CATEGORY_CODE_1C.to_string()),
            code_1c,
            name: missing_material.name_1c,
            unit: missing_material.unit,
            supplier: None,
            object_name: None,
            properties: None,
        };

        let mut count = 0;
        let mut cloned_material = store_material.clone();
        materials::store_item(&mut cloned_material, tx, &mut count).await?;

        // __ Тут решаем следующую ситуацию:
        // __ Когда мы заполняем первым проходом спецификации, для отсутствующих в таблице materials
        // __ в строке спецификаций заполняем только material_code_1c_copy, а material_code_1c остается в null.
        // __ Заменяем эти null на коды пропущенных материалов, после того, как мы добавили их в таблицу materials.

        update_missing_materials_code_1c(store_material, tx).await?;
    }

    if !PRODUCTION {
        println!("✅ Спецификации: импортировано {count} строк")
    };

    Ok(())
}

// ___ Обновляем внешний ключ (material_code_1c) на пропущенные материалы в таблице model_construct_items
// ___ после вставки пропущенных в спецификациях материалов в таблицу materials.
async fn update_missing_materials_code_1c(material: Material, tx: &mut Transaction<'_, Postgres>) -> Result<()> {
    // __ Создаем строку запроса динамически
    let query_str = format!(
        r#"
            UPDATE {} SET material_code_1c = $1 WHERE material_code_1c_copy = $2
        "#,
        ModelConstructItem::CONSTRUCT_ITEM_TABLE_NAME
    );

    sqlx::query(&query_str)
        .bind(&material.code_1c) // $1
        .bind(&material.code_1c) // $1
        .execute(&mut **tx)
        .await?;

    Ok(())
}


// ___ Получаем мапу из первичных ключей (code_1c) для проверки внешних ключей (для согласования ограничений внешнего ключа)
async fn get_entity(tx: &mut Transaction<'_, Postgres>, table_name: &str) -> Result<HashMap<String, String>> {
    // Используем anyhow::Result
    let mut entity_map_base: HashMap<String, String> = HashMap::new();

    // __ Используем Context, чтобы в логах было понятно, на какой таблице упало
    let select_query = format!("SELECT code_1c FROM {}", table_name);

    let rows = sqlx::query(&select_query)
        .fetch_all(&mut **tx)
        .await
        .with_context(|| format!("Failed to fetch codes from table: {}", table_name))?;

    for row in rows {
        // __ row.get(0) может вернуть ошибку, если колонки нет,
        // __ поэтому тут тоже можно использовать тип String напрямую
        let code: String = row.try_get(0)?;
        entity_map_base.insert(code.clone(), code);
    }

    Ok(entity_map_base)
}

// ___ Сохраняем Спецификацию
async fn store_specification(specification: ModelConstruct, tx: &mut Transaction<'_, Postgres>) -> Result<()> {
    // __ Создаем строку запроса динамически
    let query_str = format!(
        r#"
            INSERT INTO {} (
                code_1c, name, model_code_1c, model_name, type,
                created_at, updated_at
            )
            VALUES (
                $1, $2, $3, $4, $5,
                NOW() AT TIME ZONE 'Europe/Minsk', NOW() AT TIME ZONE 'Europe/Minsk'
            )
            ON CONFLICT (code_1c) DO UPDATE SET
                name          = EXCLUDED.name,
                model_code_1c = EXCLUDED.model_code_1c,
                model_name    = EXCLUDED.model_name,
                type          = EXCLUDED.type

        "#,
        ModelConstruct::CONSTRUCT_TABLE_NAME
    );

    // __ Выполняем вставку с обновлением при конфликте
    sqlx::query(&query_str)
        .bind(&specification.code_1c) // $1
        .bind(&specification.name) // $1
        .bind(&specification.model_code_1c) // $1
        .bind(&specification.model_name) // $1
        .bind(&specification.element_type) // $1
        .execute(&mut **tx)
        .await?;

    Ok(())
}

// ___ Сохраняем Запись (строку/элемент) спецификации
async fn store_specification_item(specification_item: ModelConstructItem, tx: &mut Transaction<'_, Postgres>) -> Result<()> {
    // __ Создаем строку запроса динамически
    let query_str = format!(
        r#"
            INSERT INTO {} (
                construct_code_1c,
                material_code_1c, material_code_1c_copy, material_name, material_unit,
                detail, detail_height,
                procedure_code_1c, procedure_code_1c_copy, procedure_name,
                amount, position,
                created_at, updated_at
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
                NOW() AT TIME ZONE 'Europe/Minsk', NOW() AT TIME ZONE 'Europe/Minsk'
            )
        "#,
        ModelConstructItem::CONSTRUCT_ITEM_TABLE_NAME
    );

    /*let result = */
    sqlx::query(&query_str)
        .bind(&specification_item.construct_code_1c)
        .bind(&specification_item.material_code_1c)
        .bind(&specification_item.material_code_1c_copy)
        .bind(&specification_item.material_name)
        .bind(&specification_item.material_unit)
        .bind(&specification_item.detail)
        .bind(&specification_item.detail_height)
        .bind(&specification_item.procedure_code_1c)
        .bind(&specification_item.procedure_code_1c_copy)
        .bind(&specification_item.procedure_name)
        .bind(&specification_item.count)
        .bind(&specification_item.position)
        .execute(&mut **tx)
        .await?;

    // match result {
    //     Ok(pg_result) => {
    //         // println!("Успешно вставлено строк: {}", pg_result.rows_affected());
    //     },
    //     Err(e) => {
    //         if let Some(db_err) = e.as_database_error() {
    //             eprintln!("Ошибка базы: {}", e);
    //             eprintln!("Item: {:#?}", specification_item);
    //
    //             // Проверка на дубликат ключа (Postgres code 23505)
    //             // if db_err.code() == Some(std::borrow::Cow::Borrowed("23505")) {
    //             //     eprintln!("Ошибка: Спецификация {} уже существует", specification_item);
    //             // } else {
    //             //     eprintln!("Ошибка базы данных: {}", db_err.message());
    //             // }
    //         } else {
    //             eprintln!("Системная ошибка sqlx: {}", e);
    //         }
    //         return Err(e.into()); // Или обрабатываем и идем дальше
    //     }
    // }

    Ok(())
}

// ___ Проверяем на наличие или вставляем Группу и Категорию пропущенных материалов
async fn check_or_insert_missing_materials_group_and_category(tx: &mut Transaction<'_, Postgres>) -> Result<()> {
    // __ Вставляем группу
    let query_str = format!(
        r#"
            INSERT INTO {} (
                code_1c, code_1c_copy, name,
                created_at, updated_at
            )
            VALUES (
                $1, $2, $3,
                NOW() AT TIME ZONE 'Europe/Minsk', NOW() AT TIME ZONE 'Europe/Minsk'
            )
            ON CONFLICT (code_1c) DO NOTHING
        "#,
        Material::MATERIALS_TABLE_NAME
    );

    sqlx::query(&query_str)
        .bind(MISSING_MATERIALS_GROUP_CODE_1C)
        .bind(MISSING_MATERIALS_GROUP_CODE_1C)
        .bind(MISSING_MATERIALS_GROUP_NAME)
        .execute(&mut **tx)
        .await?;

    // __ Вставляем категорию
    let query_str = format!(
        r#"
            INSERT INTO {} (
                code_1c, code_1c_copy, name, material_group_code_1c,
                created_at, updated_at
            )
            VALUES (
                $1, $2, $3, $4,
                NOW() AT TIME ZONE 'Europe/Minsk', NOW() AT TIME ZONE 'Europe/Minsk'
                )
            ON CONFLICT (code_1c) DO NOTHING
        "#,
        Material::MATERIALS_TABLE_NAME
    );

    sqlx::query(&query_str)
        .bind(MISSING_MATERIALS_CATEGORY_CODE_1C)
        .bind(MISSING_MATERIALS_CATEGORY_CODE_1C)
        .bind(MISSING_MATERIALS_CATEGORY_NAME)
        .bind(MISSING_MATERIALS_GROUP_CODE_1C)
        .execute(&mut **tx)
        .await?;

    Ok(())
}

// ___ Удаляем в Группе и Категории пропущенных материалов все записи
async fn delete_missing_materials(tx: &mut Transaction<'_, Postgres>) -> Result<()> {
    let query_str = format!(
        r#"
            DELETE FROM {} WHERE material_group_code_1c = $1 AND material_category_code_1c = $2
        "#,
        Material::MATERIALS_TABLE_NAME,
    );

    sqlx::query(&query_str)
        .bind(MISSING_MATERIALS_GROUP_CODE_1C)
        .bind(MISSING_MATERIALS_CATEGORY_CODE_1C)
        .execute(&mut **tx)
        .await?;

    Ok(())
}
