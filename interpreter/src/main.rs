#![allow(unused)]

mod helpers;
mod structures;

use crate::helpers::functions::{delete_materials_by_order_ids, delete_materials_by_order_ids_tx};
use crate::structures::expense_material::{ExpenseMaterial, ScopeItem};
use crate::structures::expression_nodes::{ExpressionNode, IfBranch};
use crate::structures::parsed_procedure::ParsedProcedure;
use crate::structures::parser::Parser;
use crate::structures::tokens::{Token, TokenType};
use anyhow::{Context, Result};
use helpers::maps::*;
use log::log;
use materials::structures::material::Material;
use materials::{add_properties, get_materials, get_materials_lookup};
use orders::structures::parsed_tree::OrderProcessRow;
use orders::{get_order_data_tree, get_order_data_tree_pool, get_order_with_lines};
use procedures::structures::procedure::Procedure;
use procedures::{get_procedures, get_procedures_by_list_code_1c};
use regex::Regex;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::env;
use std::sync::OnceLock;
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<()> {
    // __ Статистические измерения
    let start_time = Instant::now();

    // __ Получаем входной параметр
    // args() возвращает итератор. Первый элемент (индекс 0) — это всегда путь к самой программе.
    // let args: Vec<String> = env::args().collect();
    //
    // if args.len() > 1 {
    //     let first_param = &args[1];
    //     println!("Получен параметр: {}", first_param);
    // } else {
    //     println!("Параметры не переданы");
    // }


    // __ Получаем структуру для парсинга
    let orders_ids = HashSet::from([295_i64]);
    // let orders_ids = HashSet::from([820_i64, 821_i64]);
    let orders = get_data(orders_ids).await?;

    // __ Получаем материалы
    let mut materials = get_materials().await?; // Все материалы с ключем по коду 1с
    add_properties(&mut materials); // Добавляем свойства
    let materials_lookup = get_materials_lookup().await?; // Структура для поиска, где ключ - код 1с Категории

    // __ Собираем уникальные процедуры
    let mut procedures_unique: HashSet<String> = HashSet::new();
    // let mut procedures_unique: HashMap<String, String> = HashMap::new();

    for (order_id, order_lines) in &orders {
        for order_line in order_lines {
            if let Some(base_vec) = &order_line.base {
                for base in base_vec.iter() {
                    if let Some(items) = &base.items {
                        for item in items.iter() {
                            if let Some(procedure_code) = &item.pc {
                                procedures_unique.insert(procedure_code.clone());
                                // procedures_unique.insert(procedure_code.clone(), (*item).clone().pn.unwrap());
                            }
                        }
                    }
                }
            }
            if let Some(cover_vec) = &order_line.cover {
                for cover in cover_vec.iter() {
                    if let Some(items) = &cover.items {
                        for item in items.iter() {
                            if let Some(procedure_code) = &item.pc {
                                procedures_unique.insert(procedure_code.clone());
                                // procedures_unique.insert(procedure_code.clone(), (*item).clone().pn.unwrap());
                            }
                        }
                    }
                }
            }
        }
    }


    // __ Получаем процедуры с сервера
    let procedures = get_procedures_by_list_code_1c(&procedures_unique).await?; // из списка

    let procedures = get_procedures().await?; // все
    // let procedures = get_procedures_local(); // для разработки


    // println!("{:#?}", procedures);
    println!("{}", procedures.len());


    // println!("{:#?}", procedures_unique);
    // println!("{:#?}", procedures_unique.len());
    // println!("{:#?}", procedures);


    // println!("Time elapsed: {:?}", start_time.elapsed());
    //
    // return Ok(());

    // for i in (0..=4500) {

    // __ Подготавливаем карты
    get_token_map();
    get_keywords();
    get_operators();

    // __ Создаем объект Парсера
    let mut parser = Parser::new();

    let mut prepare_procedures = HashMap::<String, ParsedProcedure>::new();
    let mut max_tokens = 0;

    let mut unique_parameters: HashMap<String, Token> = HashMap::new();
    let mut unique_properties: HashMap<String, Token> = HashMap::new();
    let mut unique_returns: HashMap<String, Token> = HashMap::new();

    for procedure in procedures {
        if procedure.text.is_none() {
            continue;
        }

        let mut parsed_procedure = ParsedProcedure::default();
        parsed_procedure.procedure = procedure.clone();
        let code_source = procedure.text.as_ref().unwrap();

        // let procedure = Procedure::default();
        // let code_source = get_code_string();

        // __ Подготавливаем код
        let code_erased = code_source
            .clone()
            .lines()
            .map(|line| {
                line.split_once("//")
                    .map(|(before, _after)| before)
                    .unwrap_or(line)
                    .trim()
            })
            .filter(|line| !line.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n");

        // println!("Code: {}", code_erased);
        // let code: Vec<char> = code_erased.chars().collect();

        let mut tokens: Vec<Token> = Vec::with_capacity(1500);
        let mut pos: usize = 0;
        let mut has_properties = false;
        let mut has_parameters = false;

        while pos < code_erased.len() {
            let code_text = &code_erased[pos..];

            if let Some(token) = get_token(code_text) {
                let mut next_token = token;
                next_token.pos = pos;
                pos += next_token.text.len();

                // println!("Token: {:?}", next_token);

                let mut change_to_var_token = false;

                // __ Действия перед тем, как положить в вектор
                match next_token.token_type {
                    // __ Проверяем на свойства
                    TokenType::PROPERTY => {
                        change_to_var_token = true;
                        parsed_procedure
                            .properties_raw
                            .insert(next_token.text.clone(), 0.0);
                        // has_properties = true;
                        unique_properties.insert(next_token.text.clone(), next_token.clone()); // Собираем уникальные свойства
                    },
                    // __ Проверяем на параметры
                    TokenType::PARAMETER => {
                        change_to_var_token = true;
                        // __ Убираем из параметров случайно попавшие свойства ([БлокПружинный].[Длина]), которые попадают сюда
                        // __ из-за использования в выражениях:
                        // __ [БлокПружинный] = (РабочаяДлина * РабочаяШирина)/([БлокПружинный].[Длина] * [БлокПружинный].[Ширина])
                        match parsed_procedure
                            .procedure
                            .object_name
                            .clone()
                        {
                            Some(obj_name) => {
                                if !next_token.text.contains(&obj_name) {
                                    parsed_procedure
                                        .parameters_raw
                                        .insert(next_token.text.clone(), 0.0);
                                    unique_parameters.insert(next_token.text.clone(), next_token.clone()); // Собираем уникальные аргументы
                                }
                            },
                            None => {
                                parsed_procedure
                                    .parameters_raw
                                    .insert(next_token.text.clone(), 0.0);
                                unique_parameters.insert(next_token.text.clone(), next_token.clone()); // Собираем уникальные аргументы
                            },
                        }
                    },
                    // __ Проверяем на выходные свойства
                    TokenType::OUTPUT => {
                        change_to_var_token = true;
                        parsed_procedure
                            .outputs_raw
                            .insert(next_token.text.clone(), 0.0);
                        unique_returns.insert(next_token.text.clone(), next_token.clone()); // Собираем уникальные аргументы
                    },

                    // __ Проверяем на выходные значения
                    TokenType::RETURN => {
                        change_to_var_token = true;
                        parsed_procedure
                            .returns_raw
                            .insert(next_token.text.clone(), 0.0);
                        unique_returns.insert(next_token.text.clone(), next_token.clone()); // Собираем уникальные аргументы
                    },
                    _ => {},
                }

                if change_to_var_token {
                    next_token.token_type = TokenType::VARIABLE;
                }

                // // __ Проверяем на свойства
                // if TokenType::PROPERTY == next_token.token_type {
                //     parsed_procedure.properties.insert(next_token.text.clone(), 0.0);
                //     has_properties = true;
                //     unique_properties.insert(next_token.text.clone(), next_token.clone()); // Собираем уникальные свойства
                // }
                //
                // // __ Проверяем на параметры
                // if TokenType::PARAMETER == next_token.token_type {
                //     parsed_procedure.parameters.insert(next_token.text.clone(), 0.0);
                //     has_parameters = true;
                //     unique_parameters.insert(next_token.text.clone(), next_token.clone()); // Собираем уникальные аргументы
                // }
                //
                // // __ Проверяем на выходные значения
                // if TokenType::RETURN == next_token.token_type {
                //     parsed_procedure.returns.insert(next_token.text.clone(), 0.0);
                //     unique_returns.insert(next_token.text.clone(), next_token.clone()); // Собираем уникальные аргументы
                // }

                // __ Убираем пробелы
                if TokenType::SPACE != next_token.token_type {
                    tokens.push(next_token);
                }
            } else {
                // TODO: Сделать обработку ошибок
                panic!("Error at position {}\n Code:\n {}", pos, &code_text[pos..]);
            }
        }

        // __ Убираем пробелы
        // tokens.retain(|token| token.token_type != TokenType::SPACE);
        // tokens.retain(|token| token.token_type == TokenType::UNDEFINED);

        // println!("Tokens: {tokens:#?}");

        if tokens.len() > max_tokens {
            max_tokens = tokens.len();
            // println!("{}", procedure.code_1c);
        }


        // println!("{}", procedure.code_1c);


        // parsed_procedure.tokens = tokens.clone();
        // parsed_procedure.print_tokens();

        // parsed_procedure.has_properties = has_properties;
        // parsed_procedure.has_parameters = has_parameters;

        parser.reset();
        parser.set_tokens(tokens);
        parser.code_1c = procedure.code_1c.clone();
        parsed_procedure.expressions_node = parser.parse_code();

        prepare_procedures.insert(procedure.code_1c, parsed_procedure); // !!!
    } // !!!

    // !!! Debug
    // // println!("parsed_procedure: {:#?}", prepare_procedures);
    //
    // // let target_material = materials.get("000000691").unwrap();
    // let mut target_procedure = prepare_procedures
    //     .get("_")
    //     .unwrap()
    //     .clone();
    //
    // // target_procedure.un_raw();
    // // println!("Процедура: {:#?}", target_procedure);
    // //
    // // return Ok(());
    // let in_scope: Vec<(String, f64)> = vec![
    //     ("Высота".to_string(), 0.2),
    //     ("ВысотаИзСпецификации".to_string(), 4.0),
    //     ("Длина".to_string(), 2.0),
    //     ("Ширина".to_string(), 1.6),
    //     // ("Плотность".to_string(), 25.0),
    // ];
    // target_procedure.set_scopes(&in_scope); // __ Устанавливаем Scopes в процедуре
    // // //
    // // println!("Процедура: {:#?}", target_procedure);
    // // return Ok(());
    //
    // let mut parser = Parser::new();
    // parser.reset();
    //
    // // __ Устанавливаем Scope в парсере
    // parser.set_parser_in_scope(&target_procedure.parameters_raw, &target_procedure.properties_raw);
    //
    // // __ Запускаем Парсер
    // parser.run(&target_procedure.expressions_node);
    //
    // // __ Парсим результат выполнения функции
    // let (result, rest) = &target_procedure.set_results(&parser.scope);
    //
    // println!("Result: {:?}", result);
    //
    // // __ Парсим выходные параметры. Здесь не надо, потому что не ищем материал
    // &target_procedure.set_outputs(&parser.scope);
    //
    // println!("Процедура: {:#?}", target_procedure);
    // println!("Парсер: {:#?}", parser);
    //
    // return Ok(());
    // !!! ================================

    // println!("Параметры: {:#?}", target_procedure.parameters_raw);
    // println!("Свойства: {:#?}", target_procedure.properties_raw);
    // println!("Выходные значения: {:#?}", target_procedure.returns);
    // println!("Выходные параметры: {:#?}", target_procedure.outputs);

    // println!("{:#?}", target_material);

    // parser.set_parser_in_scope(&target_procedure.parameters_raw, &target_procedure.properties_raw);
    // let expressions_node = parser.parse_code();
    // parser.run(&expressions_node);
    // println!("scope: {:#?}", parser.scope);

    // println!("{:#?}", expressions_node);

    // parser.run(&expressions_node);
    //
    // println!("scope: {:#?}", parser.scope);


    // let mut parser = Parser::new(tokens);
    // let expressions_node = parser.parse_code();
    //
    // println!("{:#?}", expressions_node);
    //
    // parser.run(&expressions_node);
    //
    // println!("scope: {:#?}", parser.scope);


    // println!("parsed_procedure: {:#?}", target_procedure);
    // println!("parser: {:#?}", parser);

    // }

    // println!("{:#?}", unique_parameters.iter().for_each(|(text, token)| println!("{:?}", text) ));
    // println!("{:#?}", unique_properties.iter().for_each(|(text, token)| println!("{:?}", text) ));
    // println!("{:#?}", unique_returns.iter().for_each(|(text, token)| println!("{:?}", text) ));

    // __ Соединяемся с базой и создаем транзакцию
    let pool = database::connect().await?;

    // __ Создаем транзакцию
    let mut tx = pool.begin().await?;

    // __ Очищаем таблицу с материалами для переданных заказов
    let orders_id_vec: Vec<i64> = orders.keys().copied().collect();
    delete_materials_by_order_ids_tx(&mut tx, &orders_id_vec).await?;

    // __ Расчитываем материалы
    // rustfmt:skip
    for (order_id, order_lines) in orders {
        for order_line in order_lines {
            // !!! Debug
            println!(
                "{}x{}x{} : {} - {}",
                order_line.get_width() * 100f64,
                order_line.get_length() * 100f64,
                order_line.get_height() * 100f64,
                order_line.model_name,
                order_line.amount
            );
            println!("===============================================");

            // if !order_line.model_name.contains("F5") {
            //     continue;
            // }

            // __ Два прохода: База и Чехол
            for i in 0..=1 {
                let source_data = if i == 0 { &order_line.base } else { &order_line.cover };

                if let Some(construct_vec) = &source_data {
                    // !!! Debug
                    // println!("{} {}", order_line.model_name, base_vec.len());
                    // // println!("{:#?}", base_vec);
                    // println!("===============================================");


                    // __ МЭ
                    for construct in construct_vec.iter() {
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

                                            // __ Формируем Входящий Scope Добавляем размеры элементов
                                            // !!! Порядок важен
                                            let mut in_scope: Vec<(String, f64)> = Vec::from([
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

                                                // println!("Материал ---------->");
                                                // println!(
                                                //     "{}: {} + {} = {} {}",
                                                //     material.name,
                                                //     result,
                                                //     rest.unwrap_or_default(),
                                                //     result + rest.unwrap_or_default(),
                                                //     &item
                                                //         .u
                                                //         .as_ref()
                                                //         .unwrap_or(&String::new())
                                                // );
                                                // println!("Scope: {:?}", parser.scope);
                                                // println!("Results: {:?}", procedure.returns_raw);
                                                // println!("Outputs: {:?}", procedure.outputs_raw);
                                                // println!("<----------");
                                            } else {
                                                // __ Если это категория - тогда ищем материал в базе
                                                // println!("{:?}", procedure.properties_raw);


                                                // if material.name.contains("ППУ 2540") {
                                                //     println!("Мат: {:?}", material);
                                                // }

                                                // __ Если это категория, то у нее должны быть выходные параметры
                                                if procedure.outputs_raw.len() != 0 {
                                                    // __ Парсим выходные параметры, чтобы найти материал
                                                    &procedure.set_outputs(&parser.scope);

                                                    // __
                                                    // &procedure.un_raw_outputs();

                                                    // __ Ищем сам материал
                                                    let mut target_material: Option<Material> = None;
                                                    let mut err_message = "".to_string();
                                                    // __ Получаем категорию
                                                    if let Some(target_material_category) = materials_lookup.get(&material.code_1c) {
                                                        // __ Перебираем все материалы в этой категории
                                                        for (mat_code, mat) in target_material_category {
                                                            // __ Задаем два сравнивающих массива
                                                            if let Some(output_mat) = &mat.properties_map_numeric {
                                                                // Свойства, которые есть в материале

                                                                // let output_mat_debug = output_mat.clone();


                                                                // println!("==========");
                                                                // println!("{:?}", output_mat);
                                                                // println!("==========");

                                                                let output_proc = &procedure.outputs; // Свойства, которые вернула процедура

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
                                                                        // if material.code_1c.eq("000017363") {
                                                                        //     println!("?????");
                                                                        // }
                                                                        //
                                                                        // println!("Keys equal: {}", *proc_prop_key.to_lowercase() == *mat_prop_key.to_lowercase());
                                                                        // println!("Values equal: {}", (*proc_prop_value - *mat_prop_value).abs() < 1e-10);

                                                                        if *proc_prop_key.to_lowercase() == *mat_prop_key.to_lowercase()
                                                                            && (*proc_prop_value - *mat_prop_value).abs() < 1e-10
                                                                        {
                                                                            find_assign = true;
                                                                            break;
                                                                        }
                                                                        // is_find = false;
                                                                        // break;
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
                                                        err_message.push_str("Не найдена категория: ");
                                                        err_message.push_str(&material.code_1c);
                                                    }

                                                    let mut material_code_1c: Option<String>;
                                                    let mut material_code_1c_copy: Option<String>;
                                                    let mut material_name_expense: Option<String>;
                                                    let mut material_name_specification: Option<String>;
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
                                                        material_name_specification = Some(category_name);
                                                    } else {
                                                        has_error = true;
                                                        material_code_1c = None;
                                                        material_code_1c_copy = None;
                                                        material_name_expense = Some("Не найден материал".to_string());
                                                        if !err_message.is_empty() {
                                                            material_name_specification = Some(err_message);
                                                        } else {
                                                            material_name_specification = Some(category_name);
                                                        }
                                                    }

                                                    // println!("Категория +++++++++++>");
                                                    // if let Some(res_material) = target_material {
                                                    //     println!(
                                                    //         "{}: {} + {} = {} {}",
                                                    //         res_material.name,
                                                    //         result,
                                                    //         rest.unwrap_or_default(),
                                                    //         result + rest.unwrap_or_default(),
                                                    //         &item
                                                    //             .u
                                                    //             .as_ref()
                                                    //             .unwrap_or(&String::new())
                                                    //     );
                                                    //     println!("Процедура: {}", procedure.procedure.name);
                                                    //     println!("Scope: {:?}", parser.scope);
                                                    //     println!("Results: {:?}", procedure.returns_raw);
                                                    //     println!("Outputs: {:?}", procedure.outputs);
                                                    // } else {
                                                    //     println!("Материал не найден...");
                                                    //     println!("Категория {:?}", material.name);
                                                    //     println!("Scope: {:?}", parser.scope);
                                                    //     println!("Outputs: {:?}", procedure.outputs);
                                                    // }
                                                    //
                                                    //
                                                    // println!("<+++++++++++");


                                                    // __ Пропускаем нули
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
                                                        unit: material.unit.clone(),
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

                                                // println!("{:?}", material.clone().get_properties());
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
                    }
                }
            }
            // if let Some(cover_vec) = &order_line.cover {
            //     for cover in cover_vec.iter() {
            //         if let Some(items) = &cover.items {
            //             for item in items.iter() {
            //                 if let Some(procedure_code) = &item.pc {
            //                     procedures_unique.insert(procedure_code.clone());
            //                     // procedures_unique.insert(procedure_code.clone(), (*item).clone().pn.unwrap());
            //                 }
            //             }
            //         }
            //     }
            // }
        }
    }

    // __ Закрываем Транзакцию
    tx.commit().await?;

    println!("max_tokens: {}", max_tokens);

    // __ Статистика по времени
    let duration = start_time.elapsed();
    println!("Время выполнения: {:?}", duration);
    println!("Прошло миллисекунд: {}", duration.as_millis());

    Ok(())
}


//noinspection ALL
fn get_token(code: &str) -> Option<Token> {
    if let Some(map) = TOKEN_MAP.get() {
        for (token_type, regexp) in map {
            if let Some(text) = regexp.find(code) {
                // __ Regexp находит строку в кавычках, но выделяет текст без кавычек, добавляем их для сохранения длины
                let find_text = String::from(text.as_str()); // Просто берем то, что нашла регулярка

                // let find_text = match token_type {
                //     TokenType::STRING => {
                //         let mut temp_str = r#"""#.to_string();
                //         temp_str.push_str(text.as_str());
                //         temp_str.push_str(r#"""#);
                //         temp_str
                //     }
                //     _ => String::from(text.as_str()),
                // };

                let mut find_token = Token {
                    token_type: {
                        match token_type {
                            // __ Проверяем, это входной парметр типа длины или ширины или выходной для выбора материала (с '=' после )
                            TokenType::PARAMETER => {
                                let re = Regex::new(r"(?P<item>\[[^\]]+\]\.\[[^\]]+\])\s*(?P<op>=)?").unwrap();
                                // Находим только первое совпадение во всем тексте
                                if let Some(caps) = re.captures(code) {
                                    // let name = &caps["item"];
                                    if caps.name("op").is_some() { TokenType::OUTPUT } else { TokenType::PARAMETER }
                                } else {
                                    TokenType::PARAMETER
                                }
                            },
                            // __ Та же ситуация с Return: КоличествоСлоев = [ВысотаИзСпецификации]; - это параметр
                            // __ [Клей] = 5; - это итоговое значение (Return)
                            TokenType::RETURN => {
                                let re = Regex::new(r"(?P<item>\[[^\]]+\])\s*(?P<op>=)?").unwrap();
                                // Находим только первое совпадение во всем тексте
                                if let Some(caps) = re.captures(code) {
                                    // let name = &caps["item"];
                                    if caps.name("op").is_some() { TokenType::RETURN } else { TokenType::PARAMETER }
                                } else {
                                    TokenType::RETURN
                                }
                            },
                            _ => *token_type,
                        }
                    },
                    pos:        0, // Записываем в вызывающей функции
                    text:       find_text,
                    // text:       String::from(text.as_str()),
                };


                // let mut find_token = Token {
                //     token_type: *token_type,
                //     pos:        0, // Записываем в вызывающей функции
                //     text:       find_text,
                //     // text:       String::from(text.as_str()),
                // };


                // if let Some(keywords) = KEYWORDS.get() {
                //     // __ Проверяем, на ключевое слово
                //     if keywords
                //         .get(find_token.text.to_lowercase().as_str())
                //         .is_some()
                //     {
                //         find_token.token_type = TokenType::KEYWORD;
                //     } else if let Some(operators) = OPERATORS.get() {
                //         // __ Проверяем, на оператор
                //         if operators
                //             .get(find_token.text.to_lowercase().as_str())
                //             .is_some()
                //         {
                //             find_token.token_type = TokenType::OPERATOR;
                //         }
                //     }
                // }

                return Some(find_token);
            }
        }
    } else {
        println!("Карта токенов еще не инициализирована!");
    }

    None
}


fn get_procedures_local() -> Vec<Procedure> {
    // let text = String::from(
    //     r#"
    //         Бок = (Длина * ШиринаБок * ВысотаБорт)*2;
    //         Торец =((Ширина - 2*ШиринаБок) * ШиринаТорец * ВысотаБорт)*2;
    //         БокФорматка = Длина * ШиринаБок * 0.145;
    //         ТорецФорматка =((Ширина - ШиринаБок) * ШиринаТорец * 0.145)*2;
    //     "#,
    // );

    let text = String::from(
        r#"
                Длина = [Матрас].[Длина];
                Ширина = [Матрас].[Ширина];
                // ДЛИНА пружинного блока
                [БлокПружинный].[Длина] = 1.81;
                Если Длина < 1.78 Тогда
                    РабочаяДлина = Длина - 2*0.06;   // Борт 6 см
                ИначеЕсли Длина < 1.82 Тогда
                    РабочаяДлина = 1.68;   // 41 пружин
                ИначеЕсли Длина < 1.86 Тогда
                    РабочаяДлина = 1.72;  // 42 пружин
                ИначеЕсли Длина < 1.90 Тогда
                    РабочаяДлина = 1.76;  // 43 пружин
                ИначеЕсли Длина < 1.94 Тогда
                    РабочаяДлина = 1.8;  // 44 пружин
                Иначе
                    РабочаяДлина = 1.81;  // Стандартная длина
                КонецЕсли;
                // ШИРИНА пружинного блока
                Если Ширина <= 0.59 Тогда
                    [БлокПружинный].[Ширина] = 0.61;
                    РабочаяШирина = Ширина - 2*0.06;
                ИначеЕсли Ширина <= 0.65 Тогда
                    [БлокПружинный].[Ширина] = 0.61;
                    РабочаяШирина = 0.52;	// 12 пружин
                ИначеЕсли Ширина <= 0.69 Тогда
                    [БлокПружинный].[Ширина] = 0.61;
                    РабочаяШирина = 0.57;	// 13 пружин
                ИначеЕсли Ширина <= 0.81 Тогда
                    [БлокПружинный].[Ширина] = 0.61;
                    РабочаяШирина = 0.61;	// 14 пружин
                ИначеЕсли Ширина <= 0.91 Тогда
                    [БлокПружинный].[Ширина] = 0.71;
                    РабочаяШирина = 0.71;	// 16 пружин
                ИначеЕсли Ширина <= 1.09 Тогда
                    [БлокПружинный].[Ширина] = 0.81;
                    РабочаяШирина = 0.81;	// 19 пружин
                ИначеЕсли Ширина <= 1.28 Тогда
                    [БлокПружинный].[Ширина] = 1.01;
                    РабочаяШирина = 1.01;	// 23 пружин
                ИначеЕсли Ширина <= 1.48 Тогда
                    [БлокПружинный].[Ширина] = 1.21;
                    РабочаяШирина = 1.21;	// 27 пружин
                ИначеЕсли Ширина <= 1.68 Тогда
                    [БлокПружинный].[Ширина] = 1.41;
                    РабочаяШирина = 1.41;	// 32 пружин
                ИначеЕсли Ширина <= 1.88 Тогда
                    [БлокПружинный].[Ширина] = 1.61;
                    РабочаяШирина = 1.61;	// 36 пружин
                Иначе
                    [БлокПружинный].[Ширина] = 1.81;
                    РабочаяШирина = 1.81;	// 41 пружин
                КонецЕсли;
                // РАСЧЕТ
                [БлокПружинный] = (РабочаяДлина * РабочаяШирина)/([БлокПружинный].[Длина] * [БлокПружинный].[Ширина]);
                [БлокПружинныйОтход] = 1 - [БлокПружинный];
                // ФОРМАТКА
                Если Ширина = 0.28 Тогда
                    [БлокПружинный] = 0;
                    [БлокПружинныйОтход] = 0;
                КонецЕсли;
            "#,
    );

    let procedure = Procedure {
        code_1c:        "_".to_string(),
        name:           "".to_string(),
        text_vba:       None,
        object_code_1c: None,
        object_name:    Some("БлокПружинный".to_string()),
        text:           Some(text),
    };

    vec![procedure]
}


fn get_code_string_() -> String {
    String::from(
        r#"
            Длина = [Матрас].[Длина];
            Ширина = [Матрас].[Ширина];
            // ДЛИНА пружинного блока
            [БлокПружинный].[Длина] = 1.87;
            Если Длина < 1.7 Тогда
                РабочаяДлина = Длина - 2*0.06;   // Борт 6 см
            ИначеЕсли Длина < 1.76 Тогда
                РабочаяДлина = 1.64;   // 28 пружин
            ИначеЕсли Длина < 1.82 Тогда
                РабочаяДлина = 1.69;  // 29 пружин
            ИначеЕсли Длина < 1.88 Тогда
                РабочаяДлина = 1.69;  // 29 пружин
            ИначеЕсли Длина < 1.94 Тогда
                РабочаяДлина = 1.75;  // 30 пружин
            Иначе
                РабочаяДлина = 1.87;  // Стандартная длина
            КонецЕсли;
            // ШИРИНА пружинного блока
            Если Ширина <= 0.59 Тогда
                [БлокПружинный].[Ширина] = 0.67;
                РабочаяШирина = Ширина - 2*0.06;
            ИначеЕсли Ширина <= 0.63 Тогда
                [БлокПружинный].[Ширина] = 0.67;
                РабочаяШирина = 0.49;	// 8 пружин
            ИначеЕсли Ширина <= 0.69 Тогда
                [БлокПружинный].[Ширина] = 0.67;
                РабочаяШирина = 0.55;	// 9 пружин
            ИначеЕсли Ширина <= 0.81 Тогда
                [БлокПружинный].[Ширина] = 0.67;
                РабочаяШирина = 0.67;	// 11 пружин
            ИначеЕсли Ширина <= 0.91 Тогда
                [БлокПружинный].[Ширина] = 0.77;
                РабочаяШирина = 0.77;	// 12 пружин
            ИначеЕсли Ширина <= 1.08 Тогда
                [БлокПружинный].[Ширина] = 0.87;
                РабочаяШирина = 0.87;	// 14 пружин
            ИначеЕсли Ширина <= 1.28 Тогда
                [БлокПружинный].[Ширина] = 1.07;
                РабочаяШирина = 1.07;	// 17 пружин
            ИначеЕсли Ширина <= 1.48 Тогда
                [БлокПружинный].[Ширина] = 1.27;
                РабочаяШирина = 1.27;	// 20 пружин
            ИначеЕсли Ширина <= 1.68 Тогда
                [БлокПружинный].[Ширина] = 1.47;
                РабочаяШирина = 1.47;	// 23 пружин
            ИначеЕсли Ширина <= 1.87 Тогда
                [БлокПружинный].[Ширина] = 1.67;
                РабочаяШирина = 1.67;	// 26 пружин
            Иначе
                [БлокПружинный].[Ширина] = 1.87;
                РабочаяШирина = 1.87;	// 29 пружин
            КонецЕсли;
            // РАСЧЕТ
            [БлокПружинный] = (РабочаяДлина * РабочаяШирина)/([БлокПружинный].[Длина] * [БлокПружинный].[Ширина]);
            [БлокПружинныйОтход] = 1 - [БлокПружинный];
            // ФОРМАТКА
            Если Ширина <= 0.28 Тогда
                [БлокПружинный] = 0;
                [БлокПружинныйОтход] = 0;
            КонецЕсли;
        "#,
    )
    // String::from(
    //     r#"
    //         ДиаметрРулона = 0.35;
    //         КоличествоОборотов = 30;  // по КР № 33_23  3 оборота (потом вернуть на 2)
    //         Припуск = 0.3;
    //         К1 = 0.1;  //  % отхода = 0
    //
    //         Результат = Припуск * КоличествоОборотов;
    //
    //         Если Результат > 10 Тогда
    //             Переменная = 5;
    //         ИначеЕсли Результат<8 Тогда
    //             Переменная = 10;
    //         иначе
    //         Переменная = 15;
    //         КонецЕсли;
    //
    //         Если не ЗначениеЗаполнено(КоличествоСлоев) Тогда
    //         Предупреждение("Не задано количество слоев клея");
    //         КонецЕсли;
    //
    //
    //     "#,
    // )

    // String::from(
    //     r"
    //         ДиаметрРулона = 0.35;
    //         КоличествоОборотов = 3;  // по КР № 33_23  3 оборота (потом вернуть на 2)
    //         Припуск = 0.3;
    //         К1 = 0.1;  //  % отхода = 0
    //
    //         [УпаковМатериалы] = Окр((ДиаметрРулона * 3.14 * КоличествоОборотов + Припуск), 2);
    //         [УпаковМатериалыОтход] = [УпаковМатериалы] * К1;
    //
    //         Если [Матрас].[Ширина] > 0.5 и [Матрас].[Ширина] <= 1.1 Тогда
    //             [УпаковМатериалы].[Ширина] = 1.3;
    //         ИначеЕсли [Матрас].[Ширина] > 1.1 и [Матрас].[Ширина] <= 1.6 Тогда
    //             [УпаковМатериалы].[Ширина] = 1.8;
    //         ИначеЕсли [Матрас].[Ширина] > 1.6 и [Матрас].[Ширина] <= 1.8 Тогда
    //             [УпаковМатериалы].[Ширина] = 2;
    //         ИначеЕсли [Матрас].[Ширина] > 1.8 и [Матрас].[Ширина] <= 2 Тогда
    //             [УпаковМатериалы].[Ширина] = 2.2;
    //         Иначе
    //             [УпаковМатериалы] = 0;
    //             [УпаковМатериалыОтход] = 0;
    //         КонецЕсли;
    //     ",
    // )

    // String::from(
    //     r"
    //         ШиринаПолотнаРабочая = [ШвейныеМатериалы].{РабочаяШирина};
    //         [ШвейныеМатериалы].[Ширина] = [ШвейныеМатериалы].{Ширина};
    //         ШиринаПолотна = [ШвейныеМатериалы].{РабочаяШирина};
    //
    //         Захват = 0.05; // Тут комментарий 1
    //         КоэффициентФорматки = 1.4;
    //         КоэффициентНестандарт = 1.1;
    //         КоэффициентРастяжения = 1.05;
    //
    //         // Тут комментарий 2
    //
    //         Длина = [ЧехолДляМатраса].[Длина];
    //         Ширина = [ЧехолДляМатраса].[Ширина];
    //         Высота = [ЧехолДляМатраса].[Высота];
    //
    //         ПолезныйРасход = (((Длина+Высота)/КоэффициентРастяжения)*((Ширина+1.333*Высота)/КоэффициентРастяжения)/ШиринаПолотна) - (((Высота/2+0.015)*(Высота/2+0.015)/ ШиринаПолотна)*4);
    //         ОбщийРасход = ПолезныйРасход*КоэффициентНестандарт;
    //         Отход = ОбщийРасход - ПолезныйРасход;
    //     ",
    // )
}


async fn get_data(order_ids: HashSet<i64>) -> Result<BTreeMap<i64, Vec<OrderProcessRow>>> {
    let pool = database::connect()
        .await
        .context("Ошибка соединения с БД")?;

    let order_tree = get_order_data_tree_pool(&pool, order_ids)
        .await
        .context("Ошибка получения Заявки")?;

    // let order = get_order_with_lines(&pool, 820i64).await.context("Ошибка получения Заявки")?;
    // println!("pool: {:#?}", pool);
    // println!("Order: {:#?}", order);
    // println!("Order: {:#?}", order_tree);

    Ok(order_tree)
}


//
// "[МаркировкаСборочный].[Длина]"
// "[Матрас].[Длина]"
// "[Наматрасник].[Ширина]"
// "[Кровать].[ЦветТкани]"
// "[Подматрасник].[Ширина]"
// "[БлокПружинный].[Ширина]"
// "[ЧехолДляНаматрасника].[Длина]"
// "[ДетальПолотнаСтеганные].[Ширина]"
// "[ЧехолДляПодушки].[Ширина]"
// "[Подушка].[Высота]"
// "[ПотельноеБелье].[Ширина]"
// "[КаталогТканей].[Длина]"
// "[БлокПружинный].[Длина]"
// "[НаматрасникЗащитный].[Ширина]"
// "[ЧехолДляМатраса].[Высота]"
// "[ДетальКровать].[Ширина]"
// "[ШвейныеМатериалы].[Длина]"
// "[НастилМатериалы].[Ширина]"
// "[Кровать].[Ширина]"
// "[ЧехолДляНаматрасника].[Ширина]"
// "[ДетальПолотна].[Длина]"
// "[ЧехолДляНаматрасника].[Высота]"
// "[Крой деталь].[Длина]"
// "[Нашивка].[Ширина]"
// "[ПотельноеБелье].[Длина]"
// "[ОдеялаСтеганные].[Высота]"
// "[НетканыйМатериал].[Длина]"
// "[Кровать].[Длина]"
// "[НетканыйМатериал].[Ширина]"
// "[ДетальКровать].[Длина]"
// "[УпаковМатериалы].[Ширина]"
// "[ШвейныеМатериалы].[Ширина]"
// "[ОдеялаСтеганные].[Длина]"
// "[УпаковМатериалы].[РабочаяШирина]"
// "[УпаковМатериалы].[Длина]"
// "[КаталогНаматрасников].[Длина]"
// "[УпаковМатериалы].[Высота]"
// "[ДетальПолотнаСтеганные].[Длина]"
// "[ЧехолДляМатраса].[Ширина]"
// "[Повязка защитная].[Ширина]"
// "[НастилМатериалы].[Длина]"
// "[Подматрасник].[Длина]"
// "[Нашивка].[Длина]"
// "[ДетальПолотна].[Ширина]"
// "[Ручка].[Ширина]"
// "[Повязка защитная].[Длина]"
// "[ЧехолДляКаталогаНаматрасников].[Длина]"
// "[Матрас].[Высота]"
// "[ШвейныеМатериалы].[ЦветТкани]"
// "[ЧехолДляКаталогаНаматрасников].[Ширина]"
// "[Крой деталь].[Высота]"
// "[КаталогНаматрасников].[Ширина]"
// "[МаркировкаСборочный].[Ширина]"
// "[НетканыйМатериал].[Высота]"
// "[Наматрасник].[Высота]"
// "[ОдеялаСтеганные].[Ширина]"
// "[НаматрасникЗащитный].[Длина]"
// "[ЧехолДляПодушки].[Длина]"
// "[Подушка].[Длина]"
// "[ЧехолДляМатраса].[Длина]"
// "[Подушка].[Ширина]"
// "[ПолотнаСтеганные].[Ширина]"
// "[КаталогТканей].[Ширина]"
// "[Матрас].[Ширина]"
// "[НетканыйМатериал].[Плотность]"
// "[КаталогТканей].[Высота]"
// "[Ручка].[Длина]"
// "[НастилМатериалы].[Плотность]"
// "[Крой деталь].[Ширина]"
// "[Наматрасник].[Длина]"
// "[БлокПружинный].[Высота]"
// "[НастилМатериалы].[Высота]"
//
