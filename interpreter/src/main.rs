#![allow(unused)]

mod helpers;

use anyhow::{Context, Result};
use helpers::maps::*;
use orders::{get_order_data_tree, get_order_with_lines};
use orders::structures::parsed_tree::OrderProcessRow;
use regex::Regex;
use std::collections::HashMap;
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
    PARAMETER,
    PROPERTY,
    RETURN,
    OPERATOR, // Оператор, типа Окр, Цел и тд
    KEYWORD,  // Ключевое слово, пока не используем
    IF,
    ELSE,
    ELSEIF,
    ENDIF,
    THEN,
}


// struct StatementsNode {
//
// }

#[derive(Debug)]
pub struct IfBranch {
    pub condition: ExpressionNode,
    pub body:      Vec<ExpressionNode>,
}


#[derive(Debug)]
pub enum ExpressionNode {
    Number(Token),
    Variable(Token),
    // BinOperation и Assign хранят узлы внутри Box, так как размер структуры
    // в Rust должен быть известен заранее (рекурсия требует кучи)
    BinOperation {
        operator: Token,
        left:     Box<ExpressionNode>,
        right:    Box<ExpressionNode>,
    },
    UnaryOperation {
        operator: Token,
        operand:  Box<ExpressionNode>,
    },
    Assign {
        operator: Token,
        left:     Box<ExpressionNode>,
        right:    Box<ExpressionNode>,
    },
    Statements(Vec<ExpressionNode>),
    If {
        branches:  Vec<IfBranch>,               // Основной "Если" и все "ИначеЕсли"
        else_body: Option<Vec<ExpressionNode>>, // Блок "Иначе"
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // __ Статистические измерения
    let start_time = Instant::now();

    get_data(820i64).await?;

    println!("Time elapsed: {:?}", start_time.elapsed());

    return Ok(());

    // for i in (0..=4500) {


    let code_source = get_code_string();

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

    println!("Code: {}", code_erased);

    // let code: Vec<char> = code_erased.chars().collect();

    // __ Подготавливаем карты
    get_token_map();
    get_keywords();
    get_operators();

    let mut tokens: Vec<Token> = Vec::new();
    let mut pos: usize = 0;

    while pos < code_erased.len() {
        let code_text = &code_erased[pos..];

        if let Some(token) = get_token(code_text) {
            let mut next_token = token;
            next_token.pos = pos;
            pos += next_token.text.len();
            // println!("Token: {:?}", next_token);
            tokens.push(next_token);
        } else {
            // TODO: Сделать обработку ошибок
            panic!("Error at position {}\n Code:\n {}", pos, &code_text[pos..]);
        }
    }

    // __ Убираем пробелы
    tokens.retain(|token| token.token_type != TokenType::SPACE);
    // tokens.retain(|token| token.token_type == TokenType::UNDEFINED);

    println!("Tokens: {tokens:#?}");

    let mut parser = Parser::new(tokens);
    let expressions_node = parser.parse_code();

    println!("{:#?}", expressions_node);

    parser.run(&expressions_node);

    println!("scope: {:#?}", parser.scope);

    // println!("parser: {:#?}", parser);

    // }
    // __ Статистика по времени
    let duration = start_time.elapsed();
    println!("Время выполнения: {:?}", duration);
    println!("Прошло миллисекунд: {}", duration.as_millis());
}


#[derive(Debug)]
pub struct Parser {
    tokens: Vec<Token>,
    pos:    usize,
    scope:  HashMap<String, f64>, // __ Scope хранит значения переменных
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            pos: 0,
            scope: HashMap::new(),
        }
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
            None => panic!("На позиции {} ожидается {:?}", self.pos, token_types),
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
    fn parse_formula(&mut self) -> ExpressionNode {
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
            // TokenType::NOT,

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

    fn parse_parentheses(&mut self) -> ExpressionNode {
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

    fn parse_variable_or_number(&mut self) -> ExpressionNode {
        if let Some(token) = self.match_token(&[TokenType::NUMBER]) {
            return ExpressionNode::Number(token);
        }
        if let Some(token) = self.match_token(&[TokenType::VARIABLE]) {
            return ExpressionNode::Variable(token);
        }
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


    // --- Интерпретатор (Метод run) ---
    #[rustfmt::skip] // Запрещаем форматеру трогать этот массив
    pub fn run(&mut self, node: &ExpressionNode) -> f64 {
        match node {
            ExpressionNode::Number(token) => token
                .text
                .replace(",", ".")
                .parse::<f64>()
                .unwrap_or(0.0),
            ExpressionNode::Variable(token) => *self
                .scope
                .get(&token.text)
                .expect(&format!("Переменная {} не найдена", token.text)),
            ExpressionNode::BinOperation { operator, left, right } => {
                let l_val = self.run(left);
                let r_val = self.run(right);
                match operator.token_type {
                    TokenType::GT => if l_val > r_val { 1.0 } else { 0.0 },
                    TokenType::LT => if l_val < r_val { 1.0 } else { 0.0 },
                    TokenType::GE => if l_val >= r_val { 1.0 } else { 0.0 },
                    TokenType::LE => if l_val <= r_val { 1.0 } else { 0.0 },
                    TokenType::AND => if l_val > 0.0 && r_val > 0.0 { 1.0 } else { 0.0 },
                    TokenType::OR  => if l_val > 0.0 || r_val > 0.0 { 1.0 } else { 0.0 },
                    TokenType::PLUS => l_val + r_val,
                    TokenType::MINUS => l_val - r_val,
                    TokenType::STAR => l_val * r_val,
                    TokenType::SLASH => l_val / r_val,
                    _ => 0.0,
                }
            },
            ExpressionNode::Assign { left, right, .. } => {
                let result = self.run(right);
                if let ExpressionNode::Variable(v) = &**left {
                    self.scope
                        .insert(v.text.clone(), result);
                }
                result
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
            _ => 0.0,
        }
    }
}


fn get_token(code: &str) -> Option<Token> {
    if let Some(map) = TOKEN_MAP.get() {
        for (token_type, regexp) in map {
            //
            if let Some(text) = regexp.find(code) {
                let mut find_token = Token {
                    token_type: *token_type,
                    pos:        0, // Записываем в вызывающей функции
                    text:       String::from(text.as_str()),
                };


                if let Some(keywords) = KEYWORDS.get() {
                    // __ Проверяем, на ключевое слово
                    if keywords
                        .get(find_token.text.to_lowercase().as_str())
                        .is_some()
                    {
                        find_token.token_type = TokenType::KEYWORD;
                    } else if let Some(operators) = OPERATORS.get() {
                        // __ Проверяем, на оператор
                        if operators
                            .get(find_token.text.to_lowercase().as_str())
                            .is_some()
                        {
                            find_token.token_type = TokenType::OPERATOR;
                        }
                    }
                }

                return Some(find_token);
            }
        }
    } else {
        println!("Карта токенов еще не инициализирована!");
    }

    None
}


fn get_code_string() -> String {
    String::from(
        r"
            ДиаметрРулона = 0.35;
            КоличествоОборотов = 30;  // по КР № 33_23  3 оборота (потом вернуть на 2)
            Припуск = 0.3;
            К1 = 0.1;  //  % отхода = 0

            Результат = Припуск * КоличествоОборотов;

            Если Результат > 10 Тогда
                Переменная = 5;
            ИначеЕсли Результат<8 Тогда
                Переменная = 10;
            иначе
            Переменная = 15;
            КонецЕсли;

        ",
    )

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


async fn get_data(order_id: i64) -> Result<Vec<OrderProcessRow>> {
    let pool = database::connect()
        .await
        .context("Ошибка соединения с БД")?;

    let order_tree = get_order_data_tree(&pool, order_id)
        .await
        .context("Ошибка получения Заявки")?;

    // let order = get_order_with_lines(&pool, 820i64).await.context("Ошибка получения Заявки")?;
    // println!("pool: {:#?}", pool);
    // println!("Order: {:#?}", order);
    // println!("Order: {:#?}", order_tree);

    Ok(order_tree)
}
