#![allow(unused)]
use crate::constants::DATA_SHEET_1C_NAME;
use crate::helpers::{
    cell_to_string_by_option,
    get_formatted_1c_code_string,
    get_formatted_unit_string,
    // truncate_table,
};
use crate::structures::material::Material;
use anyhow::{Context, Result};
use calamine::{Reader, Xlsx, open_workbook};
use serde_json::Value;
use sqlx::{Postgres, Transaction, types::Json};
use std::collections::HashMap;

pub async fn run(tx: &mut Transaction<'_, Postgres>, path: &str) -> Result<()> {
    // __ Очищает данные и сбрасывает счетчики ID (SERIAL) в начальное состояние
    //truncate_table(Material::MATERIALS_TABLE_NAME, tx).await?;

    // __ Сбрасываем флаг is_modify
    reset_modify_flag(tx).await?;

    // __ Открываем книгу
    let mut workbook: Xlsx<_> = open_workbook(path)
        .with_context(|| format!("Не удалось открыть файл процедур: {}", path))?;

    // __ Получаем лист
    let range = workbook
        .worksheet_range(DATA_SHEET_1C_NAME)
        .with_context(|| {
            // Добавляем контекст к ошибке, если лист не найден или файл поврежден
            format!(
                "Не удалось прочитать лист '{}' в файле 1С",
                DATA_SHEET_1C_NAME
            )
        })?;

    let mut count = 0;

    let mut properties: HashMap<String, Value> = HashMap::new();
    let mut item: Material = Material::default();
    let mut group_code = String::new();
    let mut category_code = String::new();

    for row in range.rows().skip(Material::DATA_START_ROW - 2) {
        // __ Определяем тип записи в ряду + Проверяем, на тот случай, если попали в пустоту
        if !cell_to_string_by_option(row.get(Material::GROUP_CODE_COL - 1)).is_empty() {
            if !item.is_empty() {
                // __ Парсим свойства
                set_properties(&mut item, &mut properties);
                store_item(&mut item, tx, &mut count).await?; // __ Записываем в базу
            }

            let mut item_code = cell_to_string_by_option(row.get(Material::GROUP_CODE_COL - 1));

            if item_code.eq(Material::STOP_WORD) {
                break;
            }

            item_code = get_formatted_1c_code_string(item_code); // __ Приводим к нормальному виду
            let item_name = cell_to_string_by_option(row.get(Material::GROUP_NAME_COL - 1));

            item = Material::new(item_code.clone(), item_name);

            store_item(&mut item, tx, &mut count).await?; // __ Записываем в базу

            group_code = item_code; // __ Запоминаем код группы
        } else if !cell_to_string_by_option(row.get(Material::CATEGORY_CODE_COL - 1)).is_empty() {
            if !item.is_empty() {
                // __ Парсим свойства
                set_properties(&mut item, &mut properties);
                store_item(&mut item, tx, &mut count).await?; // __ Записываем в базу
            }

            let mut item_code = cell_to_string_by_option(row.get(Material::CATEGORY_CODE_COL - 1));
            item_code = get_formatted_1c_code_string(item_code); // __ Приводим к нормальному виду

            let item_name = cell_to_string_by_option(row.get(Material::CATEGORY_NAME_COL - 1));
            item = Material::new(item_code.clone(), item_name);

            item.material_group_code_1c = Some(group_code.clone()); // __ Ссылаемся на группу

            store_item(&mut item, tx, &mut count).await?; // __ Записываем в базу

            category_code = item_code; // __ Запоминаем код категории
        } else if !cell_to_string_by_option(row.get(Material::MATERIAL_CODE_COL - 1)).is_empty() {
            if !item.is_empty() {
                // __ Парсим свойства
                set_properties(&mut item, &mut properties);
                store_item(&mut item, tx, &mut count).await?; // __ Записываем в базу
            }

            let mut item_code = cell_to_string_by_option(row.get(Material::MATERIAL_CODE_COL - 1));
            item_code = get_formatted_1c_code_string(item_code); // __ Приводим к нормальному виду

            let item_name = cell_to_string_by_option(row.get(Material::MATERIAL_NAME_COL - 1));

            let mut item_unit = cell_to_string_by_option(row.get(Material::UNIT_COL - 1));
            item_unit = get_formatted_unit_string(item_unit);

            item = Material::new(item_code.clone(), item_name);

            item.material_group_code_1c = Some(group_code.clone()); // __ Ссылаемся на группу
            item.material_category_code_1c = Some(category_code.clone()); // __ Ссылаемся на категорию
            item.unit = if item_unit.is_empty() {
                None
            } else {
                Some(item_unit)
            };

            // __ НЕ!!! Записываем в базу, а накапливаем
            // store_item(&item, tx, &mut count).await?;
        } else if !cell_to_string_by_option(row.get(Material::PROPERTY_NAME_COL - 1)).is_empty() {
            properties.insert(
                cell_to_string_by_option(row.get(Material::PROPERTY_NAME_COL - 1)),
                Value::String(cell_to_string_by_option(
                    row.get(Material::PROPERTY_VALUE_COL - 1),
                )),
            );
        }
    }

    // __ Сохраняем последний Item который остался "в памяти"
    if !item.is_empty() {
        set_properties(&mut item, &mut properties);
        store_item(&mut item, tx, &mut count).await?;
    }

    println!("✅ Материалы: импортировано {} строк", count);
    Ok(())
}

// __ Устанавливаем свойства
fn set_properties(material: &mut Material, properties: &mut HashMap<String, Value>) {
    // __ Парсим свойства
    if let Some(Value::String(s)) = properties.get("ВидОбъекта") {
        material.object_name = Some(s.clone());
    }

    material.properties = if properties.is_empty() {
        None
    } else {
        Some(Json(properties.clone()))
    };

    // __ Очищаем накопленные свойства
    properties.clear();
}

async fn store_item(
    material: &mut Material,
    tx: &mut Transaction<'_, Postgres>,
    count: &mut i32,
) -> Result<()> {
    // __ Создаем строку запроса динамически
    let query_str = format!(
        r#"
    INSERT INTO {} (
        code_1c, code_1c_copy,
        material_group_code_1c, material_category_code_1c,
        name, unit, supplier,
        object_name, properties,
        created_at, updated_at,
        is_modify
    )
    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW(), NOW(), true)
    ON CONFLICT (code_1c) DO UPDATE SET
        name = EXCLUDED.name,
        unit = EXCLUDED.unit,
        supplier = EXCLUDED.supplier,
        object_name = EXCLUDED.object_name,
        properties = EXCLUDED.properties,
        material_group_code_1c = EXCLUDED.material_group_code_1c,
        material_category_code_1c = EXCLUDED.material_category_code_1c,
        updated_at = NOW(),

        -- Умное обновление флага:
        -- Если старое значение в базе отличается от нового из Excel, ставим true.
        -- В противном случае оставляем то, что уже было в базе (false).
        is_modify = CASE
            WHEN {}.material_group_code_1c IS DISTINCT FROM EXCLUDED.material_group_code_1c
              OR {}.material_category_code_1c IS DISTINCT FROM EXCLUDED.material_category_code_1c
            THEN true
            ELSE {}.is_modify
        END
    -- Условие WHERE здесь можно оставить широким (чтобы обновлялись тексты/свойства),
    -- либо убрать совсем, так как логика флага теперь живет в CASE.
    "#,
        Material::MATERIALS_TABLE_NAME,
        Material::MATERIALS_TABLE_NAME,
        Material::MATERIALS_TABLE_NAME,
        Material::MATERIALS_TABLE_NAME
    );

    // let query_str = format!(
    //     r#"
    //     INSERT INTO {} (
    //         code_1c, code_1c_copy,
    //         material_group_code_1c, material_category_code_1c,
    //         name, unit, supplier,
    //         object_name, properties,
    //         created_at, updated_at,
    //         is_modify
    //     )
    //     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW(), NOW(), true)
    //     ON CONFLICT (code_1c) DO UPDATE SET
    //         name = EXCLUDED.name,
    //         unit = EXCLUDED.unit,
    //         supplier = EXCLUDED.supplier,
    //         object_name = EXCLUDED.object_name,
    //         properties = EXCLUDED.properties,
    //         material_group_code_1c = EXCLUDED.material_group_code_1c,
    //         material_category_code_1c = EXCLUDED.material_category_code_1c,
    //         updated_at = NOW(),
    //         -- Ставим true при любом обновлении
    //         is_modify = true
    //     WHERE
    //         -- Обновляем (и ставим is_modify = true) только если хоть что-то изменилось
    //            {}.material_group_code_1c IS DISTINCT FROM EXCLUDED.material_group_code_1c
    //         OR {}.material_category_code_1c IS DISTINCT FROM EXCLUDED.material_category_code_1c
    //         OR {}.name IS DISTINCT FROM EXCLUDED.name
    //         OR {}.properties IS DISTINCT FROM EXCLUDED.properties
    //         OR {}.unit IS DISTINCT FROM EXCLUDED.unit
    //         OR {}.object_name IS DISTINCT FROM EXCLUDED.object_name
    // "#,
    //     Material::MATERIALS_TABLE_NAME,
    //     Material::MATERIALS_TABLE_NAME,
    //     Material::MATERIALS_TABLE_NAME,
    //     Material::MATERIALS_TABLE_NAME,
    //     Material::MATERIALS_TABLE_NAME,
    //     Material::MATERIALS_TABLE_NAME,
    //     Material::MATERIALS_TABLE_NAME
    // );

    // let query_str = format!(
    //     r#"
    //         INSERT INTO {} (
    //             code_1c, code_1c_copy,
    //             material_group_code_1c, material_category_code_1c,
    //             name, unit, supplier,
    //             object_name, properties,
    //             created_at, updated_at,
    //             is_modify
    //         )
    //         -- Здесь мы устанавливаем NOW() для обоих полей на случай ПЕРВОЙ вставки
    //         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW(), NOW(), true)
    //         ON CONFLICT (code_1c) DO UPDATE SET
    //             -- Мы НЕ пишем здесь created_at, чтобы оно не изменилось
    //             name = EXCLUDED.name,
    //             unit = EXCLUDED.unit,
    //             supplier = EXCLUDED.supplier,
    //             object_name = EXCLUDED.object_name,
    //             properties = EXCLUDED.properties,
    //             material_group_code_1c = EXCLUDED.material_group_code_1c,
    //             material_category_code_1c = EXCLUDED.material_category_code_1c,
    //             -- Обновляем только дату изменения
    //             updated_at = NOW()
    //     "#,
    //     Material::MATERIALS_TABLE_NAME
    // );

    // __ Выполняем вставку с обновлением при конфликте
    sqlx::query(&query_str)
        .bind(&material.code_1c)
        .bind(&material.code_1c)
        .bind(&material.material_group_code_1c)
        .bind(&material.material_category_code_1c)
        .bind(&material.name)
        .bind(&material.unit)
        .bind(&material.supplier)
        .bind(&material.object_name)
        .bind(&material.properties)
        .execute(&mut **tx)
        .await?;

    *count += 1;

    // __ Очищаем сохраненный материал
    material.clear();
    Ok(())
}

/// **Сбрасывает флаг изменения(обновления) записи при обновлении материалов**
pub async fn reset_modify_flag(tx: &mut Transaction<'_, Postgres>) -> Result<()> {
    let query = format!(
        "UPDATE {} SET is_modify = false",
        Material::MATERIALS_TABLE_NAME
    );

    sqlx::query(&query).execute(&mut **tx).await?;

    Ok(())
}
