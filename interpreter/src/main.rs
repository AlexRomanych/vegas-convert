#![allow(unused)]

mod helpers;

use anyhow::{Context, Result};
use helpers::maps::*;
use materials::get_materials;
use orders::structures::parsed_tree::OrderProcessRow;
use orders::{get_order_data_tree, get_order_data_tree_pool, get_order_with_lines};
use procedures::structures::procedure::Procedure;
use procedures::{get_procedures, get_procedures_by_list_code_1c};
use regex::Regex;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::env;
use std::sync::OnceLock;
use std::time::Instant;

#[derive(Debug, Default, Clone)]
pub struct Token {
    token_type: TokenType,
    text:       String,
    pos:        usize,
}

impl Default for TokenType {
    fn default() -> Self {
        Self::UNDEFINED
    }
}


#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)] // Эти 4 макроса обязательны
pub enum TokenType {
    UNDEFINED, // пока не используем
    NUMBER,
    VARIABLE,
    SEMICOLON,
    COMMA,
    SPACE,
    ASSIGN,
    PLUS,
    MINUS,
    AND,
    OR,
    NOT,
    GE,
    LE,
    GT,
    LT,
    NE,
    STAR,
    SLASH,
    LPAR,
    RPAR,
    PARAMETER, // Входные параметры, например, [Матрас].[Ширина] или [Матрас].[Длина]. Будем засовывать в Scope
    PROPERTY,  // Входные свойства, например, [НастилМатериалы].{Плотность} или [ПолотнаСтеганные].{РабочаяШирина}. Будем засовывать в Scope
    RETURN,    // Итоговое возвращаемое значение процедуры: [БлокПружинный] и [БлокПружинныйОтход]
    OUTPUT,    // Выходные параметры, например, [БлокПружинный].[Ширина], [БлокПружинный].[Длина], [БлокПружинный].[Высота]
    OPERATOR,  // Оператор, типа Окр, Цел и тд
    KEYWORD,   // Ключевое слово, пока не используем
    IF,
    ELSE,
    ELSEIF,
    ENDIF,
    THEN,
    FIX,     // Цел
    ROUND,   // Окр
    ALARM,   // Предупреждение
    MISSING, // ЗначениеЗаполнено
    STRING,  // "Не задано количество слоев клея"
}


// struct StatementsNode {
//
// }

#[derive(Debug)]
pub struct IfBranch {
    pub condition: ExpressionNode,
    pub body:      Vec<ExpressionNode>,
}


// #[derive(Debug)]
// pub enum ExpressionNode {
//     Number(Token),
//     Variable(Token),
//     // BinOperation и Assign хранят узлы внутри Box, так как размер структуры
//     // в Rust должен быть известен заранее (рекурсия требует кучи)
//     BinOperation {
//         operator: Token,
//         left: Box<ExpressionNode>,
//         right: Box<ExpressionNode>,
//     },
//     UnaryOperation {
//         operator: Token,
//         operand: Box<ExpressionNode>,
//     },
//     Assign {
//         operator: Token,
//         left: Box<ExpressionNode>,
//         right: Box<ExpressionNode>,
//     },
//     Statements(Vec<ExpressionNode>),
//     If {
//         branches: Vec<IfBranch>,               // Основной "Если" и все "ИначеЕсли"
//         else_body: Option<Vec<ExpressionNode>>, // Блок "Иначе"
//     },
//     // Вызов функции: Имя(Аргумент1, Аргумент2)
//     FunctionCall {
//         name: Token,
//         args: Vec<ExpressionNode>,
//     },
//
// }

#[derive(Debug)]
pub enum ExpressionNode {
    Number(Token),
    Variable(Token),
    String(Token), // <-- Добавляем этот вариант
    // Бинарные операции (A + B, X > Y)
    BinOperation {
        operator: Token,
        left:     Box<ExpressionNode>,
        right:    Box<ExpressionNode>,
    },
    // Унарные операции (НЕ A, -X)
    UnaryOperation {
        operator: Token,
        operand:  Box<ExpressionNode>,
    },
    // Присваивание (Переменная = Значение)
    Assign {
        operator: Token,
        left:     Box<ExpressionNode>,
        right:    Box<ExpressionNode>,
    },
    // Вызов функции: Имя(Аргумент1, Аргумент2)
    FunctionCall {
        name: Token,
        args: Vec<ExpressionNode>,
    },
    // Список выражений (тело функции или блока)
    Statements(Vec<ExpressionNode>),
    // Условный оператор Если...ИначеЕсли...КонецЕсли
    If {
        branches:  Vec<IfBranch>,
        else_body: Option<Vec<ExpressionNode>>,
    },
}


#[derive(Debug, Clone, Default)]
pub struct ParsedProcedure {
    procedure:      Procedure,
    tokens:         Vec<Token>,
    returns_raw:    HashMap<String, f64>,
    returns:        HashMap<String, f64>,
    properties_raw: HashMap<String, f64>, // raw - это сырые значения: [Матрас].[Длина]
    properties:     HashMap<String, f64>, // Без raw - это значения без [] и родителя: Длина
    parameters_raw: HashMap<String, f64>,
    parameters:     HashMap<String, f64>,
    outputs_raw:    BTreeMap<String, f64>, // В отсортированном порядке
    outputs:        BTreeMap<String, f64>, // Выходные параметры в отсортированном порядке
    in_scope:       HashMap<String, f64>,  // Входные параметры, которые не меняются в процессе расчетов
    out_scope:      HashMap<String, f64>,  // Все переменные, которые получились в результате расчетов в процедуре

    has_properties: bool, // __ Есть ли свойства: [НастилМатериалы].{Плотность}
    has_parameters: bool, // __ Есть ли параметры: Ширина = [Матрас].[Ширина]
}

impl ParsedProcedure {
    // __ Очищаем от скобочек то, что нашли
    pub fn un_raw(&mut self) {
        self.returns = Self::process_list(&self.returns_raw);
        self.properties = Self::process_list(&self.properties_raw);
        self.parameters = Self::process_list(&self.parameters_raw);
        self.outputs = Self::process_list(&self.outputs_raw);
    }

    fn remove_pair(text: &str) -> String {
        // Сначала берем часть после точки, если она есть
        let target = text
            .split_once('.')
            .map(|(_, p2)| p2)
            .unwrap_or(text);
        target
            .replace('[', "")
            .replace(']', "")
            .replace('{', "")
            .replace('}', "")
    }

    fn process_list<T, V>(list: &T) -> T
    where
        // 1. Ссылка на T должна давать итератор по (String, V)
        for<'a> &'a T: IntoIterator<Item = (&'a String, &'a V)>,
        // 2. T должен уметь собираться из (String, V)
        T: FromIterator<(String, V)>,
        // 3. Тип значения V должен поддерживать клонирование
        V: Clone,
    {
        list.into_iter()
            .map(|(k, v)| (Self::remove_pair(k), v.clone()))
            .collect()
    }

    // __ Устанавливаем входные Scopes
    pub fn set_scopes(&mut self, scopes: &Vec<(String, f64)>) {
        for (var, val) in scopes {
            // __ Вставляем входные параметры
            if let Some(v) = self.parameters.get(var) {
                self.parameters
                    .insert(var.clone(), *val);
            }

            // __ Вставляем входные параметры в оригинальные названия парметров после паринга токенов [Матрас].[Длина]
            // __ Приходят только в виде вектора кортежей ("Длина", 2.0)
            for (k, v) in self.parameters_raw.iter_mut() {
                for (parameter, value) in scopes {
                    if k.contains(parameter) {
                        *v = *value;
                        break;
                    }
                }
            }

            // __ Вставляем входные свойства
            if let Some(v) = self.properties.get(var) {
                self.properties
                    .insert(var.clone(), *val);
            }

            // __ Вставляем входные свойства в оригинальные названия парметров после паринга токенов [Матрас].[Длина]
            // __ Приходят только в виде вектора кортежей ("Длина", 2.0)
            for (k, v) in self.properties_raw.iter_mut() {
                for (parameter, value) in scopes {
                    if k.contains(parameter) {
                        *v = *value;
                        break;
                    }
                }
            }
        }
    }
}


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
    let orders_ids = HashSet::from([820_i64, 821_i64]);
    let orders = get_data(orders_ids).await?;

    // __ Собираем уникальные процедуры
    let mut procedures_unique: HashSet<String> = HashSet::new();
    // let mut procedures_unique: HashMap<String, String> = HashMap::new();

    for (order_id, order_lines) in orders {
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
        }
    }

    // __ Получаем материалы
    let materials = get_materials().await?;

    // __ Получаем процедуры с сервера
    // let procedures = get_procedures_by_list_code_1c(&procedures_unique).await?;     // из списка

    // let procedures = get_procedures().await?; // все
    let procedures = get_procedures_local(); // для разработки


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
                        parsed_procedure
                            .parameters_raw
                            .insert(next_token.text.clone(), 0.0);
                        // has_parameters = true;
                        unique_parameters.insert(next_token.text.clone(), next_token.clone()); // Собираем уникальные аргументы
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

        parsed_procedure.procedure = procedure.clone();
        parsed_procedure.tokens = tokens;
        parsed_procedure.has_properties = has_properties;
        parsed_procedure.has_parameters = has_parameters;


        // parsed_procedure
        //     .tokens
        //     .iter()
        //     .for_each(|token| println!("{token:?}"));

        // println!("parsed_procedure: {:#?}", parsed_procedure);

        prepare_procedures.insert(procedure.code_1c, parsed_procedure); // !!!
    } // !!!

    // println!("parsed_procedure: {:#?}", prepare_procedures);


    let target_material = materials.get("000000691").unwrap();
    let mut target_procedure = prepare_procedures
        .get("_")
        .unwrap()
        .clone();

    // target_procedure
    //     .tokens
    //     .iter()
    //     .enumerate() // Добавляет счетчик (0, 1, 2...)
    //     .for_each(|(i, token)| {
    //         println!("{i}: {token:?}");
    //     });

    target_procedure.un_raw();

    let in_scope: Vec<(String, f64)> = vec![
        ("Длина".to_string(), 2.0),
        ("Ширина".to_string(), 1.6),
        ("Высота".to_string(), 0.2),
        ("Плотность".to_string(), 25.0),
        ("ВысотаИзСпецификации".to_string(), 0.145),
    ];
    target_procedure.set_scopes(&in_scope); // __ Устанавливаем Scopes в процедуре

    println!("Параметры: {:#?}", target_procedure.parameters_raw);
    println!("Свойства: {:#?}", target_procedure.properties_raw);
    // println!("Выходные значения: {:#?}", target_procedure.returns);
    // println!("Выходные параметры: {:#?}", target_procedure.outputs);

    println!("{:#?}", target_material);


    let mut parser = Parser::new(target_procedure.tokens.clone());
    parser.set_parser_in_scope(&target_procedure.parameters_raw, &target_procedure.properties_raw);
    let expressions_node = parser.parse_code();
    parser.run(&expressions_node);
    println!("scope: {:#?}", parser.scope);

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

    println!("max_tokens: {}", max_tokens);

    // __ Статистика по времени
    let duration = start_time.elapsed();
    println!("Время выполнения: {:?}", duration);
    println!("Прошло миллисекунд: {}", duration.as_millis());

    Ok(())
}


#[derive(Debug)]
pub struct Parser {
    // procedure: ParsedProcedure,
    tokens:   Vec<Token>,
    pos:      usize,
    scope:    HashMap<String, f64>, // __ Scope хранит значения переменных
    in_scope: HashMap<String, f64>, // __ Scope входящие параметры + свойства
}

impl Parser {
    // __ Конструктор
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            pos: 0,
            scope: HashMap::new(),
            in_scope: HashMap::new(),
        }
    }

    // __ Установка входного scope
    pub fn set_parser_in_scope(&mut self, scope_parameters: &HashMap<String, f64>, scope_properties: &HashMap<String, f64>) {
        for (k, v) in scope_parameters {
            self.in_scope
                .insert(k.clone(), v.clone());
        }
        for (k, v) in scope_properties {
            self.in_scope
                .insert(k.clone(), v.clone());
        }

        println!("scope: {:#?}", self.in_scope);
    }


    // __ Вспомогательные методы
    fn match_token(&mut self, types: &[TokenType]) -> Option<Token> {
        if self.pos < self.tokens.len() {
            let current_token = &self.tokens[self.pos];
            if types.contains(&current_token.token_type) {
                self.pos += 1;
                return Some((*current_token).clone());
            }
        }
        None
    }

    fn require(&mut self, token_types: &[TokenType]) -> Token {
        match self.match_token(token_types) {
            Some(token) => token,
            None => {
                println!("Токен: {:?}", self.tokens[self.pos]);
                panic!("На позиции {} ожидается {:?}", self.pos, token_types)
            },
        }
    }

    // __ --- Методы парсинга ---
    pub fn parse_code(&mut self) -> ExpressionNode {
        let mut statements = Vec::new();
        while self.pos < self.tokens.len() {
            statements.push(self.parse_expression());
            self.require(&[TokenType::SEMICOLON]);
        }
        ExpressionNode::Statements(statements)
    }

    fn parse_expression(&mut self) -> ExpressionNode {
        // Проверяем на "Если"
        if self
            .match_token(&[TokenType::IF])
            .is_some()
        {
            return self.parse_if();
        }

        // Проверяем на "Лог" (твоя UnaryOperationNode из примера)
        // if let Some(op) = self.match_token(&[TokenType::LOG]) {
        //     return ExpressionNode::UnaryOperation {
        //         operator: op,
        //         operand: Box::new(self.parse_formula()),
        //     };
        // }

        // Проверяем, начинается ли выражение с переменной (для присваивания)
        if let Some(variable_token) = self.match_token(&[TokenType::VARIABLE]) {
            if let Some(assign_token) = self.match_token(&[TokenType::ASSIGN]) {
                let right_node = self.parse_formula();
                return ExpressionNode::Assign {
                    operator: assign_token,
                    left:     Box::new(ExpressionNode::Variable(variable_token)),
                    right:    Box::new(right_node),
                };
            }
            // Если после переменной нет "=", значит это просто формула, откатываемся
            self.pos -= 1;
        }
        self.parse_formula()
    }

    #[rustfmt::skip]
    // 1. Самый низкий приоритет: Логические И / ИЛИ
    fn parse_formula(&mut self) -> ExpressionNode {
        let mut left_node = self.parse_comparison();

        while let Some(operator) = self.match_token(&[TokenType::AND, TokenType::OR]) {
            let right_node = self.parse_comparison();
            left_node = ExpressionNode::BinOperation {
                operator,
                left: Box::new(left_node),
                right: Box::new(right_node),
            };
        }
        left_node
    }

    // 2. Сравнения: =, <>, >, <, >=, <=
    fn parse_comparison(&mut self) -> ExpressionNode {
        let mut left_node = self.parse_additive();

        while let Some(operator) = self.match_token(&[
            TokenType::ASSIGN, // В 1С внутри условия '=' это сравнение
            TokenType::NE,
            TokenType::GT,
            TokenType::LT,
            TokenType::GE,
            TokenType::LE,
        ]) {
            let right_node = self.parse_additive();
            left_node = ExpressionNode::BinOperation {
                operator,
                left: Box::new(left_node),
                right: Box::new(right_node),
            };
        }
        left_node
    }

    // 3. Сложение и вычитание: +, -
    fn parse_additive(&mut self) -> ExpressionNode {
        let mut left_node = self.parse_multiplicative();

        while let Some(operator) = self.match_token(&[TokenType::PLUS, TokenType::MINUS]) {
            let right_node = self.parse_multiplicative();
            left_node = ExpressionNode::BinOperation {
                operator,
                left: Box::new(left_node),
                right: Box::new(right_node),
            };
        }
        left_node
    }

    // 4. Умножение и деление: *, /
    fn parse_multiplicative(&mut self) -> ExpressionNode {
        let mut left_node = self.parse_unary();

        while let Some(operator) = self.match_token(&[TokenType::STAR, TokenType::SLASH]) {
            let right_node = self.parse_unary();
            left_node = ExpressionNode::BinOperation {
                operator,
                left: Box::new(left_node),
                right: Box::new(right_node),
            };
        }
        left_node
    }

    // 5. Унарные операции: не, - (отрицание)
    fn parse_unary(&mut self) -> ExpressionNode {
        if let Some(operator) = self.match_token(&[TokenType::NOT, TokenType::MINUS]) {
            let operand = self.parse_parentheses();
            return ExpressionNode::UnaryOperation {
                operator,
                operand: Box::new(operand),
            };
        }
        self.parse_parentheses()
    }

    // 6. Самый высокий приоритет: Скобки, Функции, Литералы
    fn parse_parentheses(&mut self) -> ExpressionNode {
        if self
            .match_token(&[TokenType::LPAR])
            .is_some()
        {
            let node = self.parse_formula(); // Начинаем цикл приоритетов заново внутри скобок
            self.require(&[TokenType::RPAR]);
            node
        } else {
            self.parse_variable_or_number()
        }
    }

    fn parse_variable_or_number(&mut self) -> ExpressionNode {
        if let Some(token) = self.match_token(&[TokenType::NUMBER]) {
            return ExpressionNode::Number(token);
        }

        if let Some(token) = self.match_token(&[TokenType::STRING]) {
            return ExpressionNode::String(token);
        }

        if let Some(token) = self.match_token(&[TokenType::VARIABLE]) {
            // Проверка на вызов функции: Имя(Аргументы)
            if self
                .match_token(&[TokenType::LPAR])
                .is_some()
            {
                let mut args = Vec::new();
                if self.tokens[self.pos].token_type != TokenType::RPAR {
                    loop {
                        args.push(self.parse_formula());
                        if self
                            .match_token(&[TokenType::COMMA])
                            .is_none()
                        {
                            break;
                        }
                    }
                }
                self.require(&[TokenType::RPAR]);
                return ExpressionNode::FunctionCall { name: token, args };
            }
            return ExpressionNode::Variable(token);
        }

        let current = &self.tokens[self.pos];
        panic!("Неожиданный токен {:?} на позиции {}", current.token_type, self.pos);
    }


    fn parse_formula_old(&mut self) -> ExpressionNode {
        // Сначала проверяем на унарные операторы (НЕ, МИНУС и т.д.)
        if let Some(operator) = self.match_token(&[TokenType::NOT, TokenType::MINUS]) {
            let operand = self.parse_parentheses(); // Или parse_unary для рекурсии
            return ExpressionNode::UnaryOperation {
                operator,
                operand: Box::new(operand),
            };
        }

        let mut left_node = self.parse_parentheses();

        while let Some(operator) = self.match_token(&[
            TokenType::PLUS,
            TokenType::MINUS,
            TokenType::STAR,
            TokenType::SLASH,
            TokenType::AND,
            TokenType::OR,
            TokenType::GE,
            TokenType::LE,
            TokenType::GT,
            TokenType::LT,
            TokenType::NE,
            TokenType::NOT,
            TokenType::ASSIGN, // <-- Добавь это здесь!
        ]) {
            let right_node = self.parse_parentheses();
            left_node = ExpressionNode::BinOperation {
                operator,
                left: Box::new(left_node),
                right: Box::new(right_node),
            };
        }
        left_node
    }

    fn parse_parentheses_old(&mut self) -> ExpressionNode {
        if self
            .match_token(&[TokenType::LPAR])
            .is_some()
        {
            let node = self.parse_formula();
            self.require(&[TokenType::RPAR]);
            node
        } else {
            self.parse_variable_or_number()
        }
    }

    fn parse_variable_or_number_old(&mut self) -> ExpressionNode {
        // 1. Числа
        if let Some(token) = self.match_token(&[TokenType::NUMBER]) {
            return ExpressionNode::Number(token);
        }

        // 2. Строки
        if let Some(token) = self.match_token(&[TokenType::STRING]) {
            return ExpressionNode::String(token);
        }

        // 3. Переменные или Функции
        if let Some(token) = self.match_token(&[TokenType::VARIABLE]) {
            // Проверяем: если дальше идет '(', значит это вызов функции
            if self
                .match_token(&[TokenType::LPAR])
                .is_some()
            {
                let mut args = Vec::new();
                if self.tokens[self.pos].token_type != TokenType::RPAR {
                    loop {
                        args.push(self.parse_formula());
                        if self
                            .match_token(&[TokenType::COMMA])
                            .is_none()
                        {
                            break;
                        }
                    }
                }
                self.require(&[TokenType::RPAR]);
                return ExpressionNode::FunctionCall { name: token, args };
            }
            return ExpressionNode::Variable(token);
        }


        // if let Some(token) = self.match_token(&[TokenType::VARIABLE]) {
        //     return ExpressionNode::Variable(token);
        // }
        println!("Токен: {:?}", self.tokens[self.pos]);
        panic!("Ожидается переменная или число на позиции {}", self.pos);
    }

    fn parse_if(&mut self) -> ExpressionNode {
        let mut branches = Vec::new();
        let mut else_body = None;

        // 1. Обрабатываем основной блок "Если"
        // (Слово "Если" уже съедено в parse_expression, здесь парсим условие)
        let condition = self.parse_formula();
        self.require(&[TokenType::THEN]); // Ожидаем "Тогда"

        let body = self.parse_block_until(&[TokenType::ELSEIF, TokenType::ELSE, TokenType::ENDIF]);

        branches.push(IfBranch { condition, body });

        // 2. Цикл по всем "ИначеЕсли"
        while self
            .match_token(&[TokenType::ELSEIF])
            .is_some()
        {
            let ei_condition = self.parse_formula();
            self.require(&[TokenType::THEN]);
            let ei_body = self.parse_block_until(&[TokenType::ELSEIF, TokenType::ELSE, TokenType::ENDIF]);
            branches.push(IfBranch {
                condition: ei_condition,
                body:      ei_body,
            });
        }

        // 3. Обрабатываем "Иначе", если оно есть
        if self
            .match_token(&[TokenType::ELSE])
            .is_some()
        {
            else_body = Some(self.parse_block_until(&[TokenType::ENDIF]));
        }

        // 4. Завершаем "КонецЕсли"
        self.require(&[TokenType::ENDIF]);

        ExpressionNode::If { branches, else_body }
    }

    /// Вспомогательный метод: собирает выражения в Vec, пока не встретит один из стоп-токенов
    fn parse_block_until(&mut self, stop_tokens: &[TokenType]) -> Vec<ExpressionNode> {
        let mut statements = Vec::new();

        while self.pos < self.tokens.len() {
            // Проверяем, не встретили ли мы стоп-слово (не поглощая его!)
            let current_type = self.tokens[self.pos].token_type;
            if stop_tokens.contains(&current_type) {
                break;
            }

            statements.push(self.parse_expression());
            self.require(&[TokenType::SEMICOLON]);
        }

        statements
    }


    // __ Интерпретатор (Метод run)
    #[rustfmt::skip] // Запрещаем форматеру трогать этот массив
    pub fn run(&mut self, node: &ExpressionNode) -> f64 {
        match node {
            ExpressionNode::Number(token) => token
                .text
                .replace(",", ".")
                .parse::<f64>()
                .unwrap_or(0.0),
            ExpressionNode::Variable(token) => {
                if let Some(value) = self.in_scope.get(&token.text) {
                    return *value;
                }
                *self.scope.get(&token.text).expect(&format!("Переменная {} не найдена", token.text))
            },
            ExpressionNode::BinOperation { operator, left, right } => {
                let l_val = self.run(left);
                let r_val = self.run(right);
                let raw_result = match operator.token_type {
                    TokenType::ASSIGN => if (l_val - r_val).abs() < 1e-10 { 1.0 } else { 0.0 }, // __ Сравнение в форме вычитания из-за точности
                    TokenType::GT     => if l_val > r_val { 1.0 } else { 0.0 },
                    TokenType::LT     => if l_val < r_val { 1.0 } else { 0.0 },
                    TokenType::GE     => if l_val >= r_val { 1.0 } else { 0.0 },
                    TokenType::LE     => if l_val <= r_val { 1.0 } else { 0.0 },
                    TokenType::AND    => if l_val > 0.0 && r_val > 0.0 { 1.0 } else { 0.0 },
                    TokenType::OR     => if l_val > 0.0 || r_val > 0.0 { 1.0 } else { 0.0 },
                    TokenType::PLUS   => l_val + r_val,
                    TokenType::MINUS  => l_val - r_val,
                    TokenType::STAR   => l_val * r_val,
                    TokenType::SLASH  => {
                        if r_val == 0.0 {
                            println!("⚠️ Деление на ноль!");
                            0.0
                        } else {
                            l_val / r_val
                        }
                    },
                    _ => 0.0,
                };

                // Округляем только математические операции (не логические 1.0/0.0)
                if matches!(operator.token_type, TokenType::PLUS | TokenType::MINUS | TokenType::STAR | TokenType::SLASH) {
                    Self::round_to_precision(raw_result)
                } else {
                    raw_result
                }

            },
            ExpressionNode::Assign { left, right, .. } => {
                let result = self.run(right);
                // Присваиваем уже округленное значение
                let clean_result = Self::round_to_precision(result);

                if let ExpressionNode::Variable(v) = &**left {
                    self.scope.insert(v.text.clone(), clean_result);
                }
                clean_result

                // let result = self.run(right);
                // if let ExpressionNode::Variable(v) = &**left {
                //     self.scope
                //         .insert(v.text.clone(), result);
                // }
                // result
            },
            ExpressionNode::If { branches, else_body } => {
                let mut executed = false;

                for branch in branches {
                    // Если результат условия > 0 (трактуем как true)
                    if self.run(&branch.condition) > 0.0 {
                        for stmt in &branch.body {
                            self.run(stmt);
                        }
                        executed = true;
                        break; // Выходим из If, так как ветка найдена
                    }
                }

                // Если ни одна ветка не сработала и есть блок "Иначе"
                if !executed {
                    if let Some(body) = else_body {
                        for stmt in body {
                            self.run(stmt);
                        }
                    }
                }
                0.0 // Условие само по себе обычно возвращает 0
            },
            ExpressionNode::Statements(list) => {
                let mut last_val = 0.0;
                for stmt in list {
                    last_val = self.run(stmt);
                }
                last_val
            },
            ExpressionNode::FunctionCall{ name, args } => {
                match name.text.as_str() {
                    "Окр" => {
                        let value = self.run(&args[0]);
                        let digits = if args.len() > 1 { self.run(&args[1]) as i32 } else { 0 };
                        let factor = 10.0_f64.powi(digits);
                        (value * factor).round() / factor
                    },
                    "Цел" => {
                        // Получаем значение первого аргумента
                        let value = if let Some(first_arg) = args.get(0) {
                            self.run(first_arg)
                        } else {
                            0.0
                        };
                        value.trunc() // .trunc() отсекает дробную часть, оставляя целое число (3.9 -> 3.0, -3.9 -> -3.0)
                    },
                    "ЗначениеЗаполнено" => {
                        // Получаем значение аргумента. Если его нет — по умолчанию 0.0
                        let value = args.get(0).map_or(0.0, |arg| self.run(arg));

                        // В 1С для чисел ЗначениеЗаполнено возвращает Истина, если число не 0.
                        // Учитываем "хвосты" f64, используя эпсилон-сравнение
                        if value.abs() > 1e-10 {
                            1.0
                        } else {
                            0.0
                        }
                    },
                    "Предупреждение" => {
                        // 1. Пытаемся получить аргумент.
                        // Поскольку Предупреждение в 1С чаще всего принимает строку, нам нужно решить, как её отобразить.
                        if let Some(arg) = args.get(0) {
                            // Если это строковый литерал (ExpressionNode::String), берем текст без кавычек
                            // Если это число или переменная, запускаем run()
                            match arg {
                                ExpressionNode::String(token) => {
                                    // Убираем кавычки для чистого вывода
                                    let clean_text = token.text.trim_matches('"');
                                    println!("⚠️  1С Предупреждение: {}", clean_text);
                                },
                                _ => {
                                    let val = self.run(arg);
                                    println!("⚠️  1С Предупреждение: {}", val);
                                }
                            }
                        }
                        0.0 // Функция Предупреждение ничего не возвращает в 1С
                    },
                    _ => {
                        println!("Пропущенная функция: {}", name.text);
                        0.0
                    },
                }

            },
            _ => 0.0,
        }
    }

    // __ Округление
    fn round_to_precision(val: f64) -> f64 {
        let precision = 1_000_000_000_0.0; // 10 знаков после запятой
        (val * precision).round() / precision
    }

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
                ВысотаБорт = [ВысотаИзСпецификации];
                Если  ЗначениеЗаполнено(ВысотаБорт) Тогда
                    Предупреждение("Не задана высота отбортовки ППУ");
                КонецЕсли;
                Длина = [Матрас].[Длина];
                Ширина = [Матрас].[Ширина];
                Если Длина > 2 Тогда
                    Длина = 2;
                КонецЕсли;
                Если Ширина > 2 Тогда
                    Ширина = 2;
                КонецЕсли;
                // % отхода
                Если (Ширина=0.5 или Ширина=0.6 или Ширина=0.7 или Ширина=0.8 или Ширина=0.9 или Ширина=1 или Ширина=1.2 или Ширина=1.4 или Ширина=1.6 или Ширина=1.8) и Длина = 2 Тогда
                К1 = 1.095;   // 9,5% отхода для матрасов НЕстандартных размеров. Внесено 22.01.2025
                //К1 = 1.105;  //  10,5% отхода для матрасов стандартных размеров. Внесено 26.07.2024
                //К1 = 1.115;  //  11,5%отхода для матрасов стандартных размеров. Основание: Приказ от 15.03.2023
                //К1 = 1.09;  //  9% отхода для матрасов стандартных размеров   до 23.03.23
                //К1 = 1.11; // 11% отхода для матрасов стандартных размеров. Основание: Приказ от 16.06.2021
                Иначе
                К1 = 1.115;   // 11,5% отхода для матрасов НЕстандартных размеров. Внесено 22.01.2025
                //К1 = 1.125;   // 12,5% отхода для матрасов НЕстандартных размеров. Внесено 26.07.2024
                //К1 = 1.135;   // 13,5% отхода для матрасов НЕстандартных размеров. Основание: Приказ от 15.03.2023
                //К1 = 1.11;  // 11% отхода для матрасов НЕстандартных размеров  до 23.03.23
                //К1 = 1.13; // 13% отхода для матрасов НЕстандартных размеров. Основание: Приказ от 16.06.2021
                КонецЕсли;
                // Стандартный борт
                Б4 = 0.04;
                Б6 = 0.06;
                Б8 = 0.08;
                Б10 = 0.10;
                Б12 = 0.12;
                Б14 = 0.14;
                // Стандартный торец
                Т4 = 0.04;
                Т6 = 0.06;
                Т8 = 0.08;
                // ДЛИНА матраса
                Если Длина=0.37 Тогда	// Форматка 37 см
                    ШиринаБок = Б6;
                ИначеЕсли Длина=0.44 Тогда	// Форматка 44 см
                    ШиринаБок = Б4;
                ИначеЕсли Длина<=1.97 Тогда
                    ШиринаТорец = Т6;
                Иначе
                    ШиринаТорец = Т8;
                КонецЕсли;
                // ШИРИНА матраса
                Если Ширина = 0.26 Тогда	// Форматка 26 см
                    ШиринаТорец = Т4;
                ИначеЕсли Ширина = 0.28 Тогда	// Форматка 28 см
                    ШиринаТорец = Т6;
                ИначеЕсли Ширина<=0.73 Тогда
                    ШиринаБок = Б6;
                ИначеЕсли Ширина<=0.78 Тогда
                    ШиринаБок = Б8;
                ИначеЕсли Ширина<=0.81 Тогда
                    ШиринаБок = Б10;
                ИначеЕсли Ширина<=0.83 Тогда
                    ШиринаБок = Б6;
                ИначеЕсли Ширина<=0.87 Тогда
                    ШиринаБок = Б8;
                ИначеЕсли Ширина<=0.91 Тогда
                    ШиринаБок = Б10;
                ИначеЕсли Ширина<=0.93 Тогда
                    ШиринаБок = Б6;
                ИначеЕсли Ширина<=0.97 Тогда
                    ШиринаБок = Б8;
                ИначеЕсли Ширина<=1.02 Тогда
                    ШиринаБок = Б10;
                ИначеЕсли Ширина<=1.06 Тогда
                    ШиринаБок = Б12;
                ИначеЕсли Ширина<=1.09 Тогда
                    ШиринаБок = Б14;
                ИначеЕсли Ширина<=1.13 Тогда
                    ШиринаБок = Б6;
                ИначеЕсли Ширина<=1.17 Тогда
                    ШиринаБок = Б8;
                ИначеЕсли Ширина<=1.22 Тогда
                    ШиринаБок = Б10;
                ИначеЕсли Ширина<=1.25 Тогда
                    ШиринаБок = Б12;
                ИначеЕсли Ширина<=1.28 Тогда
                    ШиринаБок = Б14;
                ИначеЕсли Ширина<=1.32 Тогда
                    ШиринаБок = Б6;
                ИначеЕсли Ширина<=1.37 Тогда
                    ШиринаБок = Б8;
                ИначеЕсли Ширина<=1.42 Тогда
                    ШиринаБок = Б10;
                ИначеЕсли Ширина<=1.45 Тогда
                    ШиринаБок = Б12;
                ИначеЕсли Ширина<=1.48 Тогда
                    ШиринаБок = Б14;
                ИначеЕсли Ширина<=1.53 Тогда
                    ШиринаБок = Б6;
                ИначеЕсли Ширина<=1.57 Тогда
                    ШиринаБок = Б8;
                ИначеЕсли Ширина<=1.62 Тогда
                    ШиринаБок = Б10;
                ИначеЕсли Ширина<=1.65 Тогда
                    ШиринаБок = Б12;
                ИначеЕсли Ширина<=1.68 Тогда
                    ШиринаБок = Б14;
                ИначеЕсли Ширина<=1.73 Тогда
                    ШиринаБок = Б6;
                ИначеЕсли Ширина<=1.77 Тогда
                    ШиринаБок = Б8;
                ИначеЕсли Ширина<=1.82 Тогда
                    ШиринаБок = Б10;
                ИначеЕсли Ширина<=1.85 Тогда
                    ШиринаБок = Б12;
                ИначеЕсли Ширина<=1.88 Тогда
                    ШиринаБок = Б14;
                ИначеЕсли Ширина<=1.93 Тогда
                    ШиринаБок = Б6;
                ИначеЕсли Ширина<=1.97 Тогда
                    ШиринаБок = Б8;
                Иначе
                    ШиринаБок = Б10;
                КонецЕсли;
                // РАСЧЕТ отбортовки
                Бок = (Длина * ШиринаБок * ВысотаБорт)*2;
                Торец =((Ширина - 2*ШиринаБок) * ШиринаТорец * ВысотаБорт)*2;
                БокФорматка = Длина * ШиринаБок * 0.145;
                ТорецФорматка =((Ширина - ШиринаБок) * ШиринаТорец * 0.145)*2;
                // МАТЕРИАЛ
                [НастилМатериалы].[Ширина] = 1.6;
                [НастилМатериалы].[Длина] = 2;
                [НастилМатериалы].[Высота] = 1.25;
                // РАСХОД материала
                Если (Ширина=0.26 или Ширина=0.28 или Длина=0.37 или Длина=0.44) Тогда	// ФОРМАТКА
                    ПолезныйРасход = (БокФорматка + ТорецФорматка) * [НастилМатериалы].{Плотность};
                Иначе
                    ПолезныйРасход = (Бок + Торец) * [НастилМатериалы].{Плотность};
                КонецЕсли;
                    ОбщийРасход = ПолезныйРасход * К1;
                    Отход = ОбщийРасход - ПолезныйРасход;

                [НастилМатериалы] =  ПолезныйРасход;
                [НастилМатериалыОтход] = Отход;
                // УВЕЛИЧЕНИЕ % отхода
                //КоэффУвеличен = 1.35; // 35 % (05.04.2021 - письмо Макей Е.)
                //КоэффУвеличен = 1.45; // до 01.08.22
                //КоэффУвеличен = 1.4; // 40 % ( изменено с 01.08.2022 - письмо Нахайчук Т. от 20.07.22)
                //КоэффУвеличен = 1.3; // 30 % ( изменено с 01.09.2022 - письмо Нахайчук Т. от 30.08.22)
                //КоэффУвеличен = 1.2; // 20 % ( изменено 23.10.2023
                //КоэффУвеличен = 1.00;  //0%  // изменено 02.11.2023
                //КоэффУвеличен = 1.10;  //10%  // изменено 23.09.2024
                //КоэффУвеличен = 1.03;  //3%  // изменено 22.01.2025
                //КоэффУвеличен = 1.1; // изменено 11.03.2025
                //КоэффУвеличен = 1.2; // изменено 18.03.2025

                КоэффУвеличен = 1.35; // изменено 24.03.2025
                Отход = Отход * КоэффУвеличен;
                ПолезныйРасход = ОбщийРасход - Отход;

                [НастилМатериалы] =  ПолезныйРасход;
                [НастилМатериалыОтход] = Отход;
            "#,
    );

    let procedure = Procedure {
        code_1c:        "_".to_string(),
        name:           "".to_string(),
        text_vba:       None,
        object_code_1c: None,
        object_name:    None,
        text:           Some(text),
    };

    vec![procedure]
}


fn get_code_string() -> String {
    String::from(
        r#"
            Длина = [Матрас].[Длина];
            Ширина = [Матрас].[Ширина];

            КоличествоСлоев = [ВысотаИзСпецификации];

            Если не ЗначениеЗаполнено(КоличествоСлоев) Тогда
                Предупреждение("Не задано количество слоев клея");
            КонецЕсли;

            T1 = 0.045 ;
            К1 = 1;

            Если Длина > 2 Тогда
                Длина = 2;
            КонецЕсли;
            Если Ширина > 2 Тогда
                Ширина = 2;
            КонецЕсли;
            Если Длина <= 0.44 Тогда
                [Клей] = 0;
            Иначе
                [Клей] = Длина * Ширина * КоличествоСлоев * T1 * К1;
            КонецЕсли;
            [Клей].[Длина] = 1;
            [Клей].[Ширина] = 2;
            [Клей].[Высота] = 4;
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
