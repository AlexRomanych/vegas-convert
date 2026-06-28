#![allow(unused)]

use anyhow::{Context, Result};
use interpreter::helpers::functions::delete_materials_by_order_ids_tx;
use interpreter::parse_procedures;
use interpreter::structures::expense_material::{ExpenseMaterial, ScopeItem};
use interpreter::structures::parsed_procedure::ParsedProcedure;
use interpreter::structures::parser::Parser;
use logger::structures::log_message::{LogLevel, LogMessage, LogTarget};
use materials::structures::material::Material;
use materials::{add_properties, get_materials_lookup, get_materials_pool};
use orders::get_order_data_tree_pool;
use orders::structures::parsed_tree::OrderProcessRow;
use procedures::structures::procedure::Procedure;
use procedures::{get_procedures_by_list_code_1c_pool, get_procedures_cutting_by_list_code_1c_pool};
use serde_json::json;
use sqlx::types::Json;
use std::collections::{BTreeMap, HashSet};
use std::io::Write;
use std::time::Instant;
use std::{env, io};
use tasks::get_cutting_tasks_with_details;
use tasks::structures::cutting_task_line::CuttingTaskLine;
// use interpreter::helpers::maps::{get_keywords, get_operators, get_token_map};
// use interpreter::structures::parsed_procedure::ParsedProcedure;
// use interpreter::structures::tokens::{Token, TokenType};
// use log::log;
// use regex::Regex;
// use std::env;
// use std::sync::OnceLock;

// const PRODUCTION: bool = false;

//noinspection DuplicatedCode
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
        let order_ids_arr = [1449];
        order_ids = HashSet::from(order_ids_arr);
    }

    // __ Соединяемся с базой
    let pool = database::connect().await?;

    // __ Пишем лог
    let inform_message = LogMessage {
        level:      LogLevel::INFO,
        target:     LogTarget::Cut,
        message:    "Начало расчета Раскроя".to_string(),
        context:    Some(Json(json!({
            "order ids": format!("{:?}", order_ids),
        }))),
        created_at: None,
    };
    inform_message.write(&pool).await.ok();

    let orders = get_cutting_tasks_with_details(&pool, &order_ids).await?;

    // __ Пишем лог
    let inform_message = LogMessage {
        level:      LogLevel::INFO,
        target:     LogTarget::Cut,
        message:    "Количество".to_string(),
        context:    Some(Json(json!({
            "order amount": format!("{}", orders.len()),
        }))),
        created_at: None,
    };
    inform_message.write(&pool).await.ok();


    // !!! Debug
    // println!("orders: {:#?}", orders);

    // __ Собираем уникальные процедуры
    let mut procedures_unique: HashSet<i64> = HashSet::new();

    for order in &orders {
        for order_line in &order.order_lines {
            if let Some(model) = &order_line.model {
                procedures_unique.insert(model.cover_up_proc_id);
                procedures_unique.insert(model.cover_down_proc_id);
                procedures_unique.insert(model.side_proc_id);
            }
        }
    }

    // __ Получаем процедуры с сервера
    let procedures = get_procedures_cutting_by_list_code_1c_pool(&pool, &procedures_unique).await?; // из списка

    // !!! Debug
    // println!("{:#?}", procedures);

    // __ Превращаем ProcedureCutting в Procedure
    // __ Конвертируем "на лету" вектор Процедур Раскроя в вектор стандартных процедур
    let cutting_procedures: Vec<Procedure> = procedures
        .into_iter()
        .map(|p| p.into_procedure())
        .collect();

    // __ Парсим все процедуры
    let prepare_procedures = parse_procedures(&pool, &cutting_procedures).await?;

    // println!("Parsed procedures: {:#?}", prepare_procedures);
    // return Ok(());

    // __ Создаем объект Парсера
    let mut parser = Parser::new();

    // __ Создаем транзакцию
    let mut tx = pool.begin().await?;

    for order in &orders {
        for order_line in &order.order_lines {
            let mut procedure_panel_up: Option<ParsedProcedure> = None;
            let mut procedure_panel_down: Option<ParsedProcedure> = None;
            let mut procedure_panel_side: Option<ParsedProcedure> = None;
            let mut height: f64 = 0.0;
            let mut angle: Option<String> = None;
            let mut model_name: String = "".to_string();

            // __ Если у модели есть технологические процессы, их тоже можно прочитать здесь:
            if let Some(model) = &order_line.model {
                // __ Доступны: model.cover_up_proc_id, model.cover_down_proc_id, etc.

                // __ Если вдруг, по какой-то причине высота Чехла 0, страхуемся и берем высоту МЭ
                height = model.get_cover_height();
                if height == 0.0 {
                    height = model.get_base_height()
                }

                // __ Запоминаем Угол
                angle = model.angle.clone();
                // println!("angle: {:?}", angle);

                // __ Запоминаем Название Модели
                model_name = model.name.clone();

                if model.cover_up_proc_id != 0 {
                    let cover_up_proc_id_str = model.cover_up_proc_id.to_string();
                    procedure_panel_up = prepare_procedures
                        .get(&cover_up_proc_id_str)
                        .cloned();
                }
                if model.cover_down_proc_id != 0 {
                    let cover_down_proc_id_str = model.cover_down_proc_id.to_string();
                    procedure_panel_down = prepare_procedures
                        .get(&cover_down_proc_id_str)
                        .cloned();
                }
                if model.side_proc_id != 0 {
                    let side_proc_id_str = model.side_proc_id.to_string();
                    procedure_panel_side = prepare_procedures
                        .get(&side_proc_id_str)
                        .cloned();
                }
            }

            // __ Распаковываем Option и обходим непосредственно линии раскроя (CuttingTaskLine)
            if let Some(cutting_task_lines) = &order_line.cutting_task_lines {
                for mut cutting_line in cutting_task_lines {
                    // __ Определяем нужную процедуру
                    let mut work_procedure: Option<ParsedProcedure> = None;
                    if let Some(detail) = &cutting_line.detail {
                        match detail.as_str() {
                            CuttingTaskLine::PANEL_NAME |
                            CuttingTaskLine::PANEL_UP_NAME => {
                                work_procedure = procedure_panel_up.clone();
                            },
                            CuttingTaskLine::PANEL_DOWN_NAME => {
                                work_procedure = procedure_panel_down.clone();
                            },
                            CuttingTaskLine::SIDE_NAME => {
                                work_procedure = procedure_panel_side.clone();
                            },
                            _ => {},
                        }
                    }

                    // __ Тут производим расчет
                    if let Some(mut procedure) = work_procedure {
                        // __ Если не прошла парсинг, пропускаем
                        if procedure.has_parse_error {
                            continue;
                        }

                        // __ Формируем Входящий Scope Добавляем размеры элементов
                        // !!! Порядок важен
                        let in_scope: Vec<(String, f64)> = Vec::from([
                            ("Высота".to_string(), height),
                            ("Длина".to_string(), order_line.get_length()),
                            ("Ширина".to_string(), order_line.get_width()),
                        ]);

                        // __ Устанавливаем Scopes в процедуре
                        procedure.set_scopes(&in_scope);

                        // __ Сбрасываем парсер
                        parser.reset();

                        // __ Устанавливаем Scope в парсере
                        parser.set_parser_in_scope(&procedure.parameters_raw, &procedure.properties_raw);

                        // __ Запускаем Парсер
                        parser.run(&procedure.expressions_node);

                        let errors = std::mem::take(&mut parser.runtime_errors);
                        for error in errors {
                            // __ Пишем Ошибки После выполнения процедуры в Лог
                            let inform_message = LogMessage {
                                level:      LogLevel::ERROR,
                                target:     LogTarget::Expense,
                                message:    "Ошибка процедуры расчета сырья".to_string(),
                                context:    Some(Json(json!({
                                    "error": error.message,
                                    "order_id": order.order_id,
                                    "cutting_task_id": order.cutting_task_id,
                                    "procedure_name": procedure.procedure.name,
                                    "procedure_id": procedure.procedure.code_1c,
                                    "line_id": order_line.order_line_id,
                                    "size": format!(
                                        "{}x{}x{}",
                                        order_line.length.map(|v| v.to_string()).unwrap_or_else(|| "0".to_string()),
                                        order_line.width.map(|v| v.to_string()).unwrap_or_else(|| "0".to_string()),
                                        order_line.height.map(|v| v.to_string()).unwrap_or_else(|| "0".to_string()),
                                    ),
                                    "model": model_name,
                                    "model_code_1c": order_line.model_code_1c,
                                    "detail_amount": cutting_line.cut_detail_amount,
                                }))),
                                created_at: None,
                            };
                            inform_message.write(&pool).await.ok();
                        }

                        // __ Парсим результат выполнения функции
                        let (result, rest) = &procedure.set_results(&parser.scope);

                        // !!! Debug
                        // println!("result: {}, rest: {:?}", result, rest);

                        // __ Парсим выходные параметры, чтобы Записать Крой
                        procedure.set_outputs(&parser.scope);

                        let outputs = procedure.outputs_raw.clone();

                        let mut work_cutting_line =  cutting_line.clone();
                        if let Some(detail) = &cutting_line.detail {
                            match detail.as_str() {
                                CuttingTaskLine::PANEL_NAME |
                                CuttingTaskLine::PANEL_UP_NAME |
                                CuttingTaskLine::PANEL_DOWN_NAME => {
                                    if let Some(cut_width) = outputs.get("[Крышка].[Ширина]") {
                                        work_cutting_line.cut_width = *cut_width as i32;
                                    }
                                    if let Some(length) = outputs.get("[Крышка].[Длина]") {
                                        work_cutting_line.cut_length = *length as i32;
                                    }
                                    work_cutting_line.angle = angle.clone();
                                },
                                CuttingTaskLine::SIDE_NAME => {
                                    if let Some(cut_width) = outputs.get("[Боковина].[Ширина]") {
                                        work_cutting_line.cut_width = *cut_width as i32;
                                    }
                                    if let Some(length) = outputs.get("[Боковина].[Длина]") {
                                        work_cutting_line.cut_length = *length as i32;
                                    }
                                    work_cutting_line.angle = None;
                                },
                                _ => {},
                            }

                            work_cutting_line.cut_detail_amount = *result as i32;
                            work_cutting_line.save_calc_data(&mut tx).await?;
                        }

                        // !!! Debug
                        // println!("Parser outputs: {:?}", procedure.outputs_raw);

                        // // __ Превращаем в объект сохранения
                        // let scope_items: Vec<ScopeItem> = parser
                        //     .scope
                        //     .iter() // Итерируемся по ссылкам (&String, &f64)
                        //     .map(|(name, &value)| ScopeItem {
                        //         n: name.clone(), // Клонируем строку, чтобы создать новую структуру
                        //         v: value,        // f64 копируется автоматически
                        //     })
                        //     .collect();

                        //println!("scope_items: {:?}", scope_items);
                    }

                    // ТВОЯ ЛОГИКА ТУТ: У тебя есть доступ ко всем полям линии раскроя
                    // println!(
                    //     "    -> Линия раскроя ID: {}. Длина: {}, Ширина: {}, Кол-во: {}, Деталь: {:?}",
                    //     cutting_line.id, cutting_line.cut_length, cutting_line.cut_width, cutting_line.cut_detail_amount, cutting_line.detail
                    // );
                }
            } else {
                println!("    -> Для этой строки заказа нет связанных линий раскроя.");
            }
        }
    }

    // __ Закрываем Транзакцию
    tx.commit().await?;

    // __ Статистика по времени
    let duration = start_time.elapsed();
    if cfg!(debug_assertions) {
        println!("Время выполнения: {:?}", duration);
        println!("Прошло миллисекунд: {}", duration.as_millis());
    };

    let inform_message = LogMessage {
        level:      LogLevel::INFO,
        target:     LogTarget::Cut,
        message:    "Окончание расчета Раскроя".to_string(),
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

    // // 1. Обходим каждый заказ (Order)
    // for order in &orders {
    //     println!(
    //         "=== Обработка Заказа ID: {}, Сменный таск ID: {} ===",
    //         order.order_id, order.cutting_task_id
    //     );
    //
    //     // 2. Обходим каждую строку заказа (OrderLine)
    //     for order_line in &order.order_lines {
    //         println!("  Строка заказа ID: {}, Код 1С: {}", order_line.order_line_id, order_line.model_code_1c);
    //
    //         // Если у модели есть технологические процессы, их тоже можно прочитать здесь:
    //         if let Some(model) = &order_line.model {
    //             // Доступны: model.cover_up_proc_id, model.cover_down_proc_id, etc.
    //         }
    //
    //         // 3. Распаковываем Option и обходим непосредственно линии раскроя (CuttingTaskLine)
    //         if let Some(cutting_task_lines) = &order_line.cutting_task_lines {
    //             for line in cutting_task_lines {
    //                 // ТВОЯ ЛОГИКА ТУТ: У тебя есть доступ ко всем полям линии раскроя
    //                 println!(
    //                     "    -> Линия раскроя ID: {}. Длина: {}, Ширина: {}, Кол-во: {}, Деталь: {:?}",
    //                     line.id, line.cut_length, line.cut_width, line.cut_detail_amount, line.detail
    //                 );
    //             }
    //         } else {
    //             println!("    -> Для этой строки заказа нет связанных линий раскроя.");
    //         }
    //     }
    // }


}
