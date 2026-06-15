// #![allow(unused)]

pub mod helpers;
pub mod structures;

use anyhow::{Result};
use crate::structures::parsed_procedure::ParsedProcedure;
use crate::structures::parser::Parser;
use crate::structures::tokens::{Token, TokenType};
use helpers::maps::*;
use logger::structures::log_message::{LogLevel, LogMessage, LogTarget};
use procedures::structures::procedure::Procedure;
use regex::Regex;
use serde_json::json;
use sqlx::types::Json;
use std::collections::HashMap;
use sqlx::PgPool;
// use anyhow::{Context, Result};
// use crate::helpers::functions::{delete_materials_by_order_ids, delete_materials_by_order_ids_tx};
// use crate::structures::expense_material::{ExpenseMaterial, ScopeItem};
// use crate::structures::expression_nodes::{ExpressionNode, IfBranch};
// use log::log;
// use materials::structures::material::Material;
// use materials::{add_properties, get_materials, get_materials_lookup};
// use orders::structures::parsed_tree::OrderProcessRow;
// use orders::{get_order_data_tree, get_order_data_tree_pool, get_order_with_lines};
// use procedures::{get_procedures, get_procedures_by_list_code_1c};
// use std::env;
// use std::sync::OnceLock;
// use std::time::Instant;


// __ Парсим процедуры
pub async fn parse_procedures(pool: &PgPool, procedures: &Vec<Procedure>) -> Result<HashMap<String, ParsedProcedure>> {
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
        parsed_procedure.has_parse_error = false;
        parsed_procedure.procedure = procedure.clone();
        let code_source = procedure.text.as_ref().unwrap();

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

        // __ Парсим текст процедуры на токены
        let mut tokens: Vec<Token> = Vec::with_capacity(1500);
        let mut pos: usize = 0;
        // let mut has_properties = false;
        // let mut has_parameters = false;

        while pos < code_erased.len() {
            let code_text = &code_erased[pos..];

            if let Some(token) = get_token(code_text) {
                let mut next_token = token;
                next_token.pos = pos;
                pos += next_token.text.len();

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

                //
                if change_to_var_token {
                    next_token.token_type = TokenType::VARIABLE;
                }

                // __ Убираем пробелы
                if TokenType::SPACE != next_token.token_type {
                    tokens.push(next_token);
                }
            } else {
                parsed_procedure.has_parse_error = true;
                if cfg!(debug_assertions) {
                    println!("Error at position {}\n Code:\n {}", pos, &code_text[pos..]);
                }
                break;
            }
        }

        // __ Максимальное количество токенов в процедуре
        if tokens.len() > max_tokens {
            max_tokens = tokens.len();
        }

        // __ Если во время парсинга токенов ошибок не было — собираем дерево выражений
        if !parsed_procedure.has_parse_error {
            parser.reset();
            parser.set_tokens(tokens);
            parser.code_1c = procedure.code_1c.clone();
            parsed_procedure.expressions_node = parser.parse_code();
        } else {
            // Если была ошибка, пишем в лог, что AST пропускается
            if cfg!(debug_assertions) {
                println!("Procedure [{:?}] skipped due to previous tokenization error.", procedure.code_1c);
            }
            // __ Пишем лог
            let inform_message = LogMessage {
                level:      LogLevel::ERROR,
                target:     LogTarget::Expense,
                message:    "Ошибка парсинга процедуры".to_string(),
                context:    Some(Json(json!({
                    "procedure code_1c": procedure.code_1c,
                }))),
                created_at: None,
            };

            inform_message.write(&pool).await.ok();
            return Err(anyhow::anyhow!( "Ошибка парсинга процедуры".to_string()));
        }


        prepare_procedures.insert(procedure.code_1c.clone(), parsed_procedure);
    }

    // println!("Максимальное количество токенов: {}", max_tokens);

    Ok(prepare_procedures)
}


//noinspection ALL
// __ Парсим Токен
fn get_token(code: &str) -> Option<Token> {
    if let Some(map) = TOKEN_MAP.get() {
        for (token_type, regexp) in map {
            if let Some(text) = regexp.find(code) {
                // __ Regexp находит строку в кавычках, но выделяет текст без кавычек, добавляем их для сохранения длины
                let find_text = String::from(text.as_str()); // Просто берем то, что нашла регулярка

                let find_token = Token {
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
                };

                return Some(find_token);
            }
        }
    } else {
        println!("Карта токенов еще не инициализирована!");
    }
    None
}
