// #![allow(unused)]

use anyhow::{Context, Result};
use interpreter::helpers::functions::delete_materials_by_order_ids_tx;
use interpreter::parse_procedures;
use interpreter::structures::expense_material::{ExpenseMaterial, ScopeItem};
use interpreter::structures::parser::Parser;
use logger::structures::log_message::{LogLevel, LogMessage, LogTarget};
use materials::structures::material::Material;
use materials::{add_properties, get_materials_lookup, get_materials_pool};
use orders::get_order_data_tree_pool;
use orders::structures::parsed_tree::OrderProcessRow;
use procedures::get_procedures_by_list_code_1c_pool;
use procedures::structures::procedure::Procedure;
use serde_json::json;
use sqlx::types::Json;
use std::collections::{BTreeMap, HashSet};
use std::io::Write;
use std::time::Instant;
use std::{env, io};

// use interpreter::helpers::maps::{get_keywords, get_operators, get_token_map};
// use interpreter::structures::parsed_procedure::ParsedProcedure;
// use interpreter::structures::tokens::{Token, TokenType};
// use log::log;
// use regex::Regex;
// use std::env;
// use std::sync::OnceLock;

// const PRODUCTION: bool = false;

#[tokio::main]
async fn main() -> Result<()> {
    // __ Статистические измерения
    let start_time = Instant::now();

    let order_ids: HashSet<i64>;


    if !cfg!(debug_assertions) {
        // __ На проде
        // __ Получаем входной параметр
        // 1. Собираем аргументы командной строки
        let args: Vec<String> = env::args().collect();

        // args[0] — путь к бинарнику, args[1] — наш JSON от Laravel
        if args.len() < 2 {
            anyhow::bail!("Отсутствует аргумент JSON с order_ids");
        }

        let json_data = &args[1];

        // 2. Десериализуем строку напрямую в коллекцию для быстрой выборки
        order_ids = serde_json::from_str(json_data).context("Не удалось распарсить переданный из PHP JSON массив")?;
    } else {
        // __ На dev
        let order_ids_arr = [
            449, 450, 467, 445, 456, 462, 438, 461, 452, 475, 442, 444, 446, 447, 443, 451, 454, 459, 470, 476, 436, 455, 463, 440, 472, 439, 448,
            457, 453, 473, 441, 464, 468, 474, 466, 471, 460, 465, 458, 469, 437,
        ];
        order_ids = HashSet::from(order_ids_arr);
    }


    // println!("ids: {:?}", order_ids);

    // // args() возвращает итератор. Первый элемент (индекс 0) — это всегда путь к самой программе.
    // let args: Vec<String> = env::args().collect();
    //
    // if args.len() > 1 {
    //     let first_param = &args[1];
    //     println!("Получен параметр: {}", first_param);
    // } else {
    //     println!("Параметры не переданы");
    // }



    // __ Соединяемся с базой
    let pool = database::connect().await?;

    // __ Пишем лог
    let inform_message = LogMessage {
        level:      LogLevel::INFO,
        target:     LogTarget::Expense,
        message:    "Начало расчета сырья".to_string(),
        context:    Some(Json(json!({
            "order ids": format!("{:?}", order_ids),
        }))),
        created_at: None,
    };
    inform_message.write(&pool).await.ok();

    // __ Получаем структуру Заявок для парсинга Расхода
    let orders = get_data(&pool, order_ids).await?;

    // __ Получаем материалы
    let mut materials = get_materials_pool(&pool).await?; // Все материалы с ключем по коду 1с
    add_properties(&mut materials); // Добавляем свойства, которые пока не можем добыть
    let materials_lookup = get_materials_lookup(&pool).await?; // Структура для поиска Материалов в Категории, где ключ - код 1с Категории

    // __ Собираем уникальные процедуры
    let mut procedures_unique: HashSet<String> = HashSet::new();

    for (_, order_lines) in &orders {
        for order_line in order_lines {
            // __ Два прохода: МЭ + Чехол
            for i in 0..=1 {
                let source_vec = if i == 0 { &order_line.base } else { &order_line.cover };

                if let Some(base) = source_vec {
                    if let Some(items) = &base.items {
                        for item in items.iter() {
                            if let Some(procedure_code) = &item.pc {
                                procedures_unique.insert(procedure_code.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    // __ Получаем процедуры с сервера
    let procedures = get_procedures_by_list_code_1c_pool(&pool, &procedures_unique).await?; // из списка
    // let procedures = get_procedures().await?; // все
    // let procedures = _get_procedures_local(); // для разработки

    // println!("{}", procedures.len());

    // __ Создаем объект Парсера
    let mut parser = Parser::new();

    // __ Парсим все процедуры
    let prepare_procedures = parse_procedures(&pool, &procedures).await?;

    // println!("Parsed procedures: {:#?}", prepare_procedures);

    // __ Создаем транзакцию
    let mut tx = pool.begin().await?;

    // __ Очищаем таблицу с Расходом материалов для переданных заказов
    let orders_id_vec: Vec<i64> = orders.keys().copied().collect();
    delete_materials_by_order_ids_tx(&mut tx, &orders_id_vec).await?;

    // __ Расчитываем материалы
    // rustfmt:skip
    for (id, order_lines) in orders {
        for order_line in order_lines {
            // !!! Debug
            // println!(
            //     "{}x{}x{} : {} - {}",
            //     order_line.get_width() * 100f64,
            //     order_line.get_length() * 100f64,
            //     order_line.get_height() * 100f64,
            //     order_line.model_name,
            //     order_line.amount
            // );
            // println!("===============================================");

            // if !order_line.model_name.contains("F5") {
            //     continue;
            // }

            // __ Проверяем, является ли Модель Average или нет
            // __ В зависимости от этого, либо рассчитываем, либо делаем что-то еще
            if order_line.is_average() {
                println!("average");
                continue;
            }

            // println!("reached");

            let mut has_cover = false; // __ Маяк наличия чехла

            // __ Два прохода: База и Чехол
            for i in 0..=1 {
                // __ Проверяем, что в спецификации есть чехол по процедуре расчета
                // __ Потому, что может быть ситуация, когда в Таблице Models есть привязка к Чехлу
                // __ например в Подушках, а в расчетах-то он и не нужен
                // __ или вообще Заказ на Чехол отдельно
                let source_data = if i == 0 {
                    if let Some(construct) = &order_line.base {
                        if let Some(items) = &construct.items {
                            for item in items.iter() {
                                if let Some(procedure_name) = &item.pn {
                                    if procedure_name.contains("ПодборЧехла") {
                                        has_cover = true;
                                        // println!("{}", procedure_name);
                                    }
                                    // if procedure_name.to_lowercase().contains("ПодборЧехла".to_lowercase().as_str())  {}
                                }
                            }
                        }
                    }

                    &order_line.base
                } else {
                    &order_line.cover
                };

                // __ Пропускаем то, что без чехла
                if i == 1 && !has_cover {
                    continue;
                }

                if let Some(construct) = &source_data {
                    // !!! Debug
                    // println!("{} {}", order_line.model_name, base_vec.len());
                    // // println!("{:#?}", base_vec);
                    // println!("===============================================");

                    // __ МЭ
                    // for construct in construct_vec.iter() {
                    if let Some(items) = &construct.items {
                        for item in items.iter() {
                            // __ Пропускаем материал, которого по какой-то причине нет в материалах
                            if let Some(material_code) = &item.mc {
                                // __ Получаем материал из базы материалов
                                if let Some(material) = materials.get(material_code) {
                                    // println!("{}", material.name);
                                    // __ Смотрим, есть ли у материала процедура расчета или нет
                                    if let Some(procedure_code) = &item.pc {
                                        // !!! Debug
                                        // if !material.name.contains("ПС ") {
                                        //     continue;
                                        // };

                                        // __ Есть процедура
                                        let mut procedure = prepare_procedures
                                            .get(procedure_code)
                                            .unwrap()
                                            .clone();

                                        // __ Если не прошла парсинг, пропускаем
                                        if procedure.has_parse_error {
                                            continue;
                                        }

                                        // __ Формируем Входящий Scope Добавляем размеры элементов
                                        // !!! Порядок важен
                                        let in_scope: Vec<(String, f64)> = Vec::from([
                                            ("Высота".to_string(), order_line.get_height()),
                                            ("ВысотаИзСпецификации".to_string(), item.h.unwrap_or_default()),
                                            ("Длина".to_string(), order_line.get_length()),
                                            ("Ширина".to_string(), order_line.get_width()),
                                        ]);

                                        // !!! Debug
                                        // println!("item: {:?}", item);

                                        // __ Дополняем Входящий Scope
                                        if let Some(properties_map) = &material.properties_map_numeric {
                                            let properties_vec: Vec<(String, f64)> = properties_map
                                                .iter()
                                                .map(|(k, v)| (k.clone(), *v)) // Клонируем String, копируем f64
                                                .collect();
                                            procedure.add_properties_to_scopes(&properties_vec);
                                        }

                                        // __ Устанавливаем Scopes в процедуре
                                        procedure.set_scopes(&in_scope);

                                        // __ Сбрасываем парсер
                                        parser.reset();

                                        // __ Устанавливаем Scope в парсере
                                        parser.set_parser_in_scope(&procedure.parameters_raw, &procedure.properties_raw);

                                        // __ Запускаем Парсер
                                        parser.run(&procedure.expressions_node);

                                        // __ Пишем все ошибки, если они есть
                                        // std::mem::take забирает вектор из parser, а на его место кладет пустой Vec::new()
                                        let errors = std::mem::take(&mut parser.runtime_errors);
                                        for error in errors {
                                            // __ Пишем Ошибки После выполнения процедуры в Лог
                                            let inform_message = LogMessage {
                                                level:      LogLevel::ERROR,
                                                target:     LogTarget::Expense,
                                                message:    "Ошибка процедуры расчета сырья".to_string(),
                                                context:    Some(Json(json!({
                                                    "error": error.message,
                                                    "order_id": id,
                                                    "procedure_name": procedure.procedure.name,
                                                    "procedure_code_1c": procedure.procedure.code_1c,
                                                    "line_id": order_line.line_id,
                                                    "size": format!(
                                                        "{}x{}x{}",
                                                        order_line.length.map(|v| v.to_string()).unwrap_or_else(|| "0".to_string()),
                                                        order_line.width.map(|v| v.to_string()).unwrap_or_else(|| "0".to_string()),
                                                        order_line.height.map(|v| v.to_string()).unwrap_or_else(|| "0".to_string()),
                                                    ),
                                                    "model": order_line.model_name,
                                                    "amount": order_line.amount,
                                                }))),
                                                created_at: None,
                                            };
                                            inform_message.write(&pool).await.ok();
                                        }

                                        // __ Парсим результат выполнения функции
                                        let (result, rest) = &procedure.set_results(&parser.scope);

                                        // !!! Debug
                                        // if procedure
                                        //     .procedure
                                        //     .name
                                        //     .contains("020_БлокПружинныйMedR2200")
                                        // {
                                        //     &procedure.set_outputs(&parser.scope);
                                        //     println!("Мат: {:?}", material);
                                        //     println!("Проц: {:?}", procedure);
                                        //     println!("Парсер: {:?}", parser);
                                        //     let a = 0;
                                        // }

                                        // __ Смотрим, это материал или категория. Если материал - возвращаем его
                                        if material.is_material() {
                                            // __ Парсим выходные параметры. Здесь не надо, потому что не ищем материал
                                            // &procedure.set_outputs(&parser.scope);

                                            // __ Пропускаем нули
                                            let total_expense = result * &item.a.unwrap_or(1.0) * order_line.amount as f64;
                                            if total_expense == 0.0 {
                                                continue;
                                            }

                                            // __ Превращаем в объект сохранения
                                            let scope_items: Vec<ScopeItem> = parser
                                                .scope
                                                .iter() // Итерируемся по ссылкам (&String, &f64)
                                                .map(|(name, &value)| ScopeItem {
                                                    n: name.clone(), // Клонируем строку, чтобы создать новую структуру
                                                    v: value,        // f64 копируется автоматически
                                                })
                                                .collect();

                                            let expense_material = ExpenseMaterial {
                                                order_line_id:               order_line.line_id,
                                                material_code_1c:            Some(material.code_1c.clone()),
                                                material_code_1c_copy:       Some(material.code_1c.clone()),
                                                expense_per_pic:             result * &item.a.unwrap_or(1.0),
                                                expense:                     result * &item.a.unwrap_or(1.0) * order_line.amount as f64,
                                                rest_per_pic:                rest.unwrap_or_default() * &item.a.unwrap_or(1.0),
                                                rest:                        rest.unwrap_or_default()
                                                    * &item.a.unwrap_or(1.0)
                                                    * order_line.amount as f64,
                                                unit:                        material.unit.clone(),
                                                detail:                      item.d.clone(),
                                                procedure:                   Some(procedure.procedure.name.clone()),
                                                object_name:                 procedure.procedure.object_name.clone(),
                                                position:                    item.p,
                                                scopes:                      scope_items,
                                                outputs:                     Vec::new(),
                                                material_name_expense:       Some(material.name.clone()),
                                                material_name_specification: Some(material.name.clone()),
                                            };
                                            expense_material
                                                .save_record(&mut tx)
                                                .await?;
                                        } else {
                                            // __ Если это категория - тогда ищем материал в базе

                                            // !!! Debug
                                            // if material.name.contains("ППУ 2540") {
                                            //     println!("Мат: {:?}", material);
                                            // }

                                            // __ Если это категория, то у нее должны быть выходные параметры
                                            if procedure.outputs_raw.len() != 0 {
                                                // __ Парсим выходные параметры, чтобы найти материал
                                                let _ = procedure.set_outputs(&parser.scope);

                                                // __ Ищем сам материал
                                                let mut target_material: Option<Material> = None;
                                                let mut err_message = "".to_string();
                                                // __ Получаем категорию
                                                if let Some(target_material_category) = materials_lookup.get(&material.code_1c) {
                                                    // __ Перебираем все материалы в этой категории
                                                    for (_, mat) in target_material_category {
                                                        // __ Задаем два сравнивающих массива
                                                        if let Some(output_mat) = &mat.properties_map_numeric {
                                                            // Свойства, которые есть в материале
                                                            // let output_mat_debug = output_mat.clone();

                                                            let output_proc = &procedure.outputs; // Свойства, которые вернула процедура

                                                            // !!! Debug
                                                            // let output_mat_debug = output_mat.clone();
                                                            // let output_proc_debug = output_proc
                                                            //     .clone()
                                                            //     .iter()
                                                            //     .map(|(k, v)| (k.clone(), v.clone()))
                                                            //     .collect::<HashMap<String, f64>>();

                                                            // __ Сравниваем методом перебора.
                                                            // __ Предполагаем, что количество свойств, которые вернула процедура
                                                            // __ не больше, чем свойст у материала, поэотму внешний цикл по свойствам процедуры
                                                            let mut is_find = true;
                                                            for (proc_prop_key, proc_prop_value) in output_proc {
                                                                let mut find_assign = false;
                                                                for (mat_prop_key, mat_prop_value) in output_mat {
                                                                    if *proc_prop_key.to_lowercase() == *mat_prop_key.to_lowercase()
                                                                        && (*proc_prop_value - *mat_prop_value).abs() < 1e-10
                                                                    {
                                                                        find_assign = true;
                                                                        break;
                                                                    }
                                                                }

                                                                if !find_assign {
                                                                    is_find = false;
                                                                    break;
                                                                }
                                                            }

                                                            if is_find {
                                                                target_material = Some(mat.clone());
                                                                break;
                                                            }
                                                        }
                                                    }
                                                } else {
                                                    if cfg!(debug_assertions) {
                                                        err_message.push_str("Не найдена категория: ");
                                                        err_message.push_str(&material.code_1c);
                                                    }
                                                    // __ Пишем Ошибки Не найденной категории в Лог
                                                    let inform_message = LogMessage {
                                                        level:      LogLevel::ERROR,
                                                        target:     LogTarget::Expense,
                                                        message:    "Не найдена категория".to_string(),
                                                        context:    Some(Json(json!({
                                                            "error": format!("Категория: {} ({})", &material.name, &material.code_1c),
                                                            "order_id": id,
                                                            "procedure_name": procedure.procedure.name,
                                                            "procedure_code_1c": procedure.procedure.code_1c,
                                                            "line_id": order_line.line_id,
                                                            "size": format!(
                                                                "{}x{}x{}",
                                                                order_line.length.map(|v| v.to_string()).unwrap_or_else(|| "0".to_string()),
                                                                order_line.width.map(|v| v.to_string()).unwrap_or_else(|| "0".to_string()),
                                                                order_line.height.map(|v| v.to_string()).unwrap_or_else(|| "0".to_string()),
                                                            ),
                                                            "model": order_line.model_name,
                                                            "amount": order_line.amount,
                                                        }))),
                                                        created_at: None,
                                                    };
                                                    inform_message.write(&pool).await.ok();
                                                }

                                                let material_code_1c: Option<String>;
                                                let material_code_1c_copy: Option<String>;
                                                let material_name_expense: Option<String>;
                                                let material_name_specification: Option<String>;
                                                let unit: Option<String>;
                                                let mut has_error = false;

                                                let category = materials
                                                    .get(&material.code_1c)
                                                    .cloned() // Превращает Option<&Material> в Option<Material>
                                                    .unwrap_or_default(); // Теперь Default сработает отлично!
                                                let category_name = category.name;

                                                if let Some(res_material) = target_material {
                                                    material_code_1c = Some(res_material.code_1c.clone());
                                                    material_code_1c_copy = Some(res_material.code_1c.clone());
                                                    material_name_expense = Some(res_material.name.clone());
                                                    unit = res_material.unit.clone();
                                                    material_name_specification = Some(category_name);
                                                } else {
                                                    has_error = true;
                                                    material_code_1c = None;
                                                    material_code_1c_copy = None;
                                                    unit = None;
                                                    material_name_expense = Some("Не найден материал".to_string());
                                                    if !err_message.is_empty() {
                                                        material_name_specification = Some(err_message);
                                                    } else {
                                                        material_name_specification = Some(category_name);
                                                    }
                                                }

                                                // __ Пропускаем нули и пишем ошибки
                                                let total_expense = result * &item.a.unwrap_or(1.0) * order_line.amount as f64;
                                                if total_expense == 0.0 && !has_error {
                                                    continue;
                                                }

                                                // __ Превращаем в объект сохранения
                                                let scope_items: Vec<ScopeItem> = parser
                                                    .scope
                                                    .iter() // Итерируемся по ссылкам (&String, &f64)
                                                    .map(|(name, &value)| ScopeItem {
                                                        n: name.clone(), // Клонируем строку, чтобы создать новую структуру
                                                        v: value,        // f64 копируется автоматически
                                                    })
                                                    .collect();

                                                let outputs_items: Vec<ScopeItem> = procedure
                                                    .outputs
                                                    .iter() // Итерируемся по ссылкам (&String, &f64)
                                                    .map(|(name, &value)| ScopeItem {
                                                        n: name.clone(), // Клонируем строку, чтобы создать новую структуру
                                                        v: value,        // f64 копируется автоматически
                                                    })
                                                    .collect();

                                                let expense_material = ExpenseMaterial {
                                                    order_line_id: order_line.line_id,
                                                    material_code_1c,
                                                    material_code_1c_copy,
                                                    expense_per_pic: result * &item.a.unwrap_or(1.0),
                                                    expense: result * &item.a.unwrap_or(1.0) * order_line.amount as f64,
                                                    rest_per_pic: rest.unwrap_or_default() * &item.a.unwrap_or(1.0),
                                                    rest: rest.unwrap_or_default() * &item.a.unwrap_or(1.0) * order_line.amount as f64,
                                                    unit,
                                                    detail: item.d.clone(),
                                                    procedure: Some(procedure.procedure.name.clone()),
                                                    object_name: procedure.procedure.object_name.clone(),
                                                    position: item.p,
                                                    scopes: scope_items,
                                                    outputs: outputs_items,
                                                    material_name_expense,
                                                    material_name_specification,
                                                };
                                                expense_material
                                                    .save_record(&mut tx)
                                                    .await?;
                                            } else {
                                                // !!! Debug
                                                println!("Категория {:?}", material.name);
                                                println!("Outputs: {:?}", procedure.outputs_raw);
                                            }
                                        }
                                    } else {
                                        // __ Нет процедуры

                                        // __ Пропускаем нули (хотя их тут быть не должно)
                                        let total_expense = &item.a.unwrap_or(1.0) * order_line.amount as f64;
                                        if total_expense == 0.0 {
                                            continue;
                                        }

                                        let expense_material = ExpenseMaterial {
                                            order_line_id:               order_line.line_id,
                                            material_code_1c:            Some(material.code_1c.clone()),
                                            material_code_1c_copy:       Some(material.code_1c.clone()),
                                            expense_per_pic:             item.a.unwrap_or(1.0),
                                            expense:                     &item.a.unwrap_or(1.0) * order_line.amount as f64,
                                            rest_per_pic:                0.0,
                                            rest:                        0.0,
                                            unit:                        material.unit.clone(),
                                            detail:                      item.d.clone(),
                                            procedure:                   None,
                                            object_name:                 None,
                                            position:                    item.p,
                                            scopes:                      Vec::new(),
                                            outputs:                     Vec::new(),
                                            material_name_expense:       Some(material.name.clone()),
                                            material_name_specification: Some(material.name.clone()),
                                        };
                                        expense_material
                                            .save_record(&mut tx)
                                            .await?;

                                        // !!! Debug
                                        // let expense = order_line.amount as f64 * &item.a.unwrap_or(1.0);
                                        // println!("{}: {} {}", material.name, expense, &item.u.as_ref().unwrap_or(&String::new()));
                                    }
                                }
                            }
                        }
                    }
                    // }
                }
            }
        }
    }

    // __ Закрываем Транзакцию
    tx.commit().await?;

    // println!("max_tokens: {}", max_tokens);

    // __ Статистика по времени
    let duration = start_time.elapsed();
    if cfg!(debug_assertions) {
        println!("Время выполнения: {:?}", duration);
        println!("Прошло миллисекунд: {}", duration.as_millis());
    };

    let inform_message = LogMessage {
        level:      LogLevel::INFO,
        target:     LogTarget::Expense,
        message:    "Окончание расчета сырья".to_string(),
        context:    Some(Json(json!({
            "elapsed_time, sec.": format!("{:?}", duration),
        }))),
        created_at: None,
    };
    inform_message.write(&pool).await.ok();

    if !cfg!(debug_assertions) {
        println!("0")
    };

    // __ Принудительно толкаем в буфер
    io::stdout().flush()?;
    // io::stdout().flush().unwrap();

    Ok(())
}


// __ Получаем данные
async fn get_data(pool: &sqlx::PgPool, order_ids: HashSet<i64>) -> Result<BTreeMap<i64, Vec<OrderProcessRow>>> {
    let order_tree = get_order_data_tree_pool(&pool, order_ids)
        .await
        .context("Ошибка получения Заявки")?;

    Ok(order_tree)
}


// __ Для дебага
fn _get_procedures_local() -> Vec<Procedure> {
    let text = String::from(
        r#"
            [ШвейныеМатериалы].[Ширина] = [ШвейныеМатериалы].{Ширина};
            ШиринаПолотна  =  [ШвейныеМатериалы].{Ширина};
            Захват = 0.02;

            Если [Подушка].[Длина]=0.7 и [Подушка].[Ширина]= 0.7 тогда
                ПолезныйРасход = 0.7 ;
                ОбщийРасход = 0.72 + Захват;
                Отход = ОбщийРасход - ПолезныйРасход;

            ИначеЕсли [Подушка].[Длина]=0.7 и [Подушка].[Ширина]= 0.5 тогда
                ПолезныйРасход = 0.51 ;
                ОбщийРасход = 0.52 + Захват;
                Отход = ОбщийРасход - ПолезныйРасход;

            ИначеЕсли [Подушка].[Длина]=0.6 и [Подушка].[Ширина]= 0.4 тогда
                ПолезныйРасход = 0.35 ;
                ОбщийРасход = 0.42 + Захват;
                Отход = ОбщийРасход - ПолезныйРасход;

            ИначеЕсли [Подушка].[Длина]=0.6 и [Подушка].[Ширина]= 0.6 тогда
                ПолезныйРасход = 0.52 ;
                ОбщийРасход = 0.62 + Захват;
                Отход = ОбщийРасход - ПолезныйРасход;

            ИначеЕсли [Подушка].[Длина]=0.5 и [Подушка].[Ширина]= 0.5 тогда
                ПолезныйРасход = 0.37 ;
                ОбщийРасход = 0.52 + Захват;
                Отход = ОбщийРасход - ПолезныйРасход;


            Иначе ОбщийРасход = 0;

            КонецЕсли;
            [ШвейныеМатериалы] =  ПолезныйРасход;
            [ШвейныеМатериалыОтход] = Отход;
            "#,
    );

    let procedure = Procedure {
        code_1c:        "_".to_string(),
        name:           "".to_string(),
        text_vba:       None,
        object_code_1c: None,
        object_name:    Some("ШвейныеМатериалы".to_string()),
        text:           Some(text),
    };

    vec![procedure]
}
