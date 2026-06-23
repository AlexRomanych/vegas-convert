// use crate::structures::cutting_task::CuttingTask;
use crate::structures::order::Order;
use std::collections::{HashSet};

pub mod structures;


// __ Получаем структуру для парсинга СЗ на Раскрой на детальки раскроя + подключение к БД
pub async fn get_cutting_tasks_with_details(pool: &sqlx::PgPool, order_ids: &HashSet<i64>) -> Result<Vec<Order>, sqlx::Error> {
    // Конвертируем HashSet в вектор для передачи в Postgres ANY($1)
    let ids: Vec<i64> = order_ids.iter().map(|&id| id).collect();

    let mut results = sqlx::query_as::<_, Order>(
        r#"
                SELECT
                    ct.order_id,
                    ct.id AS cutting_task_id,
                    COALESCE(
                        json_agg(
                            json_build_object(
                                'order_line_id', ol.id,
                                'model_code_1c', ol.model_code_1c,
                                'width', ol.width,
                                'length', ol.length,
                                'height', ol.height,
                                'model', CASE
                                    WHEN m.code_1c IS NOT NULL THEN
                                        json_build_object(
                                            'base_height', m.base_height,
                                            'name', m.name,
                                            'cover_height', m.cover_height,
                                            'angle', m.angle,
                                            'cover_up_proc_id', m.cover_up_proc_id,
                                            'cover_down_proc_id', m.cover_down_proc_id,
                                            'side_proc_id', m.side_proc_id
                                        )
                                    -- ELSE NULL
                                 END,
                                'cutting_task_lines', ctl_grouped.lines
                            )
                        ) FILTER (WHERE ol.id IS NOT NULL),
                        '[]'::json
                    ) AS order_lines_raw
                FROM cutting_tasks ct
                LEFT JOIN order_lines ol ON ol.order_id = ct.order_id
                LEFT JOIN models m ON m.code_1c = ol.model_code_1c
                LEFT JOIN (
                    SELECT
                        ctl.order_line_id,
                        ctl.cutting_task_id,
                        json_agg(
                            json_build_object(
                                'id', ctl.id,
                                'cut_length', ctl.cut_length,
                                'cut_width', ctl.cut_width,
                                'cut_detail_amount', ctl.cut_detail_amount,
                                'angle', ctl.angle,
                                'detail', ctl.detail
                            )
                        ) AS lines
                    FROM cutting_task_lines ctl
                    GROUP BY ctl.order_line_id, ctl.cutting_task_id
                ) ctl_grouped ON ctl_grouped.order_line_id = ol.id AND ctl_grouped.cutting_task_id = ct.id
                WHERE ct.order_id = ANY($1)
                GROUP BY ct.order_id, ct.id;
        "#,
    )
        .bind(&ids)
        .fetch_all(pool)
        .await?;

    // 2. Преобразование: переносим данные из Json(Vec) в чистый Vec
    for order in results.iter_mut() {
        // .order_lines_data.0 — это взятие по деструктуризации внутреннего вектора без клонирования
        // Используем std::mem::take, чтобы забрать данные, оставив пустой Json(Vec) взамен
        let raw_json_data = std::mem::take(&mut order.order_lines_raw);
        order.order_lines = raw_json_data.0;
    }

    Ok(results)
}


// // __ Получаем структуру для парсинга СЗ на Раскрой на детальки раскроя + подключение к БД
// pub async fn _get_cutting_tasks_with_details(pool: &sqlx::PgPool, order_ids: &HashSet<i64>) -> Result<Vec<Order>, sqlx::Error> {
//     // Конвертируем HashSet в вектор для передачи в Postgres ANY($1)
//     let ids: Vec<i64> = order_ids.iter().map(|&id| id).collect();
//
//     let rows = sqlx::query_as::<_, Order>(
//         r#"
//                 SELECT
//                     ct.order_id,
//                     ct.id AS cutting_task_id,
//                     COALESCE(
//                         json_agg(
//                             json_build_object(
//                                 'order_line_id', ol.id,
//                                 'model_code_1c', ol.model_code_1c,
//                                 'model', CASE
//                                     WHEN m.code_1c IS NOT NULL THEN
//                                         json_build_object(
//                                             'cover_up_proc_id', m.cover_up_proc_id,
//                                             'cover_down_proc_id', m.cover_down_proc_id,
//                                             'side_proc_id', m.side_proc_id
//                                         )
//                                     ELSE NULL
//                                  END,
//                                 'cutting_task_lines', ctl_grouped.lines
//                             )
//                         ) FILTER (WHERE ol.id IS NOT NULL),
//                         '[]'::json
//                     ) AS order_lines_raw
//                 FROM cutting_tasks ct
//                 LEFT JOIN order_lines ol ON ol.order_id = ct.order_id
//                 LEFT JOIN models m ON m.code_1c = ol.model_code_1c
//                 LEFT JOIN (
//                     SELECT
//                         ctl.order_line_id,
//                         ctl.cutting_task_id,
//                         json_agg(
//                             json_build_object(
//                                 'id', ctl.id,
//                                 'cut_length', ctl.cut_length,
//                                 'cut_width', ctl.cut_width,
//                                 'cut_detail_amount', ctl.cut_detail_amount,
//                                 'detail', ctl.detail
//                             )
//                         ) AS lines
//                     FROM cutting_task_lines ctl
//                     GROUP BY ctl.order_line_id, ctl.cutting_task_id
//                 ) ctl_grouped ON ctl_grouped.order_line_id = ol.id AND ctl_grouped.cutting_task_id = ct.id
//                 WHERE ct.order_id = ANY($1)
//                 GROUP BY ct.order_id, ct.id;
//         "#,
//     )
//         .bind(&ids)
//         .fetch_all(pool)
//         .await?;
//
//     // 2. Группируем таски по order_id в мапу
//     // Используем BTreeMap, чтобы сохранить сортировку по order_id, либо HashMap для скорости
//     let mut orders_map: BTreeMap<i64, Order> = BTreeMap::new();
//
//     for row in rows {
//         let order_id = row.order_id;
//
//         // Извлекаем линии раскроя (cutting_task_lines) из пришедших данных ордера
//         let mut task_lines = Vec::new();
//         let mut order_lines = row.order_lines_raw.0;
//
//         // Собираем все CuttingTaskLine, которые база нашла для ЭТОГО конкретного cutting_task_id
//         for ol in &mut order_lines {
//             if let Some(mut lines) = ol.cutting_task_lines.take() {
//                 task_lines.append(&mut lines);
//             }
//         }
//
//         // Формируем структуру CuttingTask
//         let cutting_task = CuttingTask {
//             id: row.cutting_task_id,
//             cutting_task_lines: task_lines,
//         };
//
//         // Вставляем или обновляем запись ордера в мапе
//         orders_map
//             .entry(order_id)
//             .and_modify(|order| {
//                 order.tasks.push(cutting_task.clone());
//             })
//             .or_insert_with(|| Order {
//                 order_id,
//                 cutting_task_id: cutting_task.id,
//                 order_lines_raw: sqlx::types::Json(Vec::new()), // оставляем пустым, заглушка для FromRow
//                 order_lines, // сохраняем линии самого заказа
//                 tasks: vec![cutting_task],
//             });
//     }
//
//     // Превращаем мапу обратно в плоский вектор Vec<Order>
//     let result: Vec<Order> = orders_map.into_values().collect();
//
//     Ok(result)
// }




// // __ Получаем структуру для парсинга СЗ на Раскрой на детальки раскроя + подключение к БД
// pub async fn get_order_data_tree_pool(pool: &sqlx::PgPool, order_ids: HashSet<i64>) -> Result<Vec<CuttingTask>> {
//     let list = order_ids
//         .iter()
//         .map(|s| format!("'{}'", s))
//         .collect::<Vec<_>>()
//         .join(", ");
//
//
//
//
//     // __ Спецификация берется из строки (order_line: construct_code_1c) и возвращается не в виде масства, а в виде объекта
//     let query = MAIN_QUERY.get_or_init(|| {
//         format!(
//             r#"
//                 SELECT
//                     o.id AS order_id,
//                     ol.id AS line_id,
//                     ol.width AS width,
//                     ol.length AS length,
//                     ol.height AS height,
//                     ol.amount AS amount,
//                     ol.construct_code_1c AS spec_code_1c, 	-- Спецификация в order_line, по которой происходит расчет (можно комментировать)
//                     m.code_1c AS model_code,				-- Код модели из 1С (можно комментировать)
//                     m.name AS model_name,					-- Название модели (можно комментировать)
//                     m.base_height AS base_height,
//                     m.cover_height AS cover_height,
//
//                     -- 1. БАЗОВАЯ ЧАСТЬ (base)
//                     -- Теперь ищем конструкцию напрямую по ol.construct_code_1c, минуя m.code_1c
//                     (
//                         -- SELECT JSON_AGG() - выдает массив
//                         -- SELECT JSON_BUILD_OBJECT() - выдает объект
//                         SELECT JSON_BUILD_OBJECT(
//                             'construct_code', mc.code_1c,
//                             'items', (
//                                 SELECT JSON_AGG(JSON_BUILD_OBJECT(
//                                     'mc', mci.material_code_1c,
//                                     'pc', mci.procedure_code_1c,
//                                     'pn', mci.procedure_name,
//                                     'h', mci.detail_height,
//                                     'a', mci.amount,
//                                     'u', mci.material_unit,
//                                     'p', mci.position,
//                                     'd', mci.detail
//                                 ))
//                                 FROM model_construct_items mci
//                                 WHERE mci.construct_code_1c = mc.code_1c
//                             )
//                         )
//                         FROM model_constructs mc
//                         -- Тут Спецификация привязана напрямую к строке заказа
//                         WHERE mc.code_1c = ol.construct_code_1c -- СВЯЗЬ НАПРЯМУЮ С КОРНЕВОЙ СТРОКОЙ ЗАКАЗА
//                         -- Тут Спецификация привязана к ссылке на спецификацию в Модели
//                         -- WHERE mci.construct_code_1c = mc.code_1c
//                     ) AS base,
//
//                     -- 2. Чехол (cover)
//                     -- Осталась без изменений, так как завязана на код чехла из модели m.cover_code_1c
//                     (
//                         -- SELECT JSON_AGG() - выдает массив
//                         -- SELECT JSON_BUILD_OBJECT() - выдает объект
//                         SELECT JSON_BUILD_OBJECT(
//                             'construct_code', mcc.code_1c,
//                             'items', (
//                                 SELECT JSON_AGG(JSON_BUILD_OBJECT(
//                                     'mc', mcic.material_code_1c,
//                                     'pc', mcic.procedure_code_1c,
//                                     'pn', mcic.procedure_name,
//                                     'h', mcic.detail_height,
//                                     'a', mcic.amount,
//                                     'u', mcic.material_unit,
//                                     'p', mcic.position,
//                                     'd', mcic.detail
//                                 ))
//                                 FROM model_construct_items mcic
//                                 WHERE mcic.construct_code_1c = mcc.code_1c
//                             )
//                         )
//                         FROM models m_cover
//                         JOIN model_constructs mcc ON mcc.model_code_1c = m_cover.code_1c
//                         -- Тут Спецификация привязана к ссылке на спецификацию Чехла в Модели
//                         WHERE m_cover.code_1c = m.cover_code_1c
//                     ) AS cover
//                 FROM orders AS o
//                 JOIN order_lines ol ON ol.order_id = o.id
//                 JOIN models m ON m.code_1c = ol.model_code_1c
//                 WHERE o.id IN ({})
//                 ORDER BY ol.id;
//             "#,
//             list
//         )
//     });
//
//     let rows = sqlx::query_as::<_, OrderProcessRow>(query.as_str())
//         // .bind(list)
//         .fetch_all(pool)
//         .await?;
//
//
//     // 2. Группируем программно
//     let mut orders_map: BTreeMap<i64, Vec<OrderProcessRow>> = BTreeMap::new();
//
//     for row in rows {
//         orders_map
//             .entry(row.order_id)
//             .or_insert_with(Vec::new)
//             .push(row);
//     }
//
//     // Теперь есть структура, где ключ — ID заказа,
//     // а значение — массив всех его строк со всеми вложенными материалами.
//
//     // println!("Orders: {:#?}", orders_map);
//     // orders_map.iter().for_each(|(order_id, _)| println!("{:?}", order_id));
//
//     Ok(orders_map)
// }
