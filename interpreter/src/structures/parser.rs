use crate::structures::expression_nodes::{ExpressionNode, IfBranch};
use crate::structures::tokens::{Token, TokenType};
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct Parser {
    // procedure: ParsedProcedure,
    tokens:      Vec<Token>,
    pos:         usize,
    pub scope:   HashMap<String, f64>, // __ Scope хранит значения переменных
    in_scope:    HashMap<String, f64>, // __ Scope входящие параметры + свойства
    pub code_1c: String,
}


impl Parser {
    // __ Конструктор
    pub fn new() -> Self {
        Self {
            tokens:   Vec::new(),
            pos:      0,
            scope:    HashMap::new(),
            in_scope: HashMap::new(),
            code_1c: String::new(),
        }
    }

    // __ Устанавливаем список Токенов
    pub fn set_tokens(&mut self, tokens: Vec<Token>) {
        self.tokens = tokens;
    }

    // __ Сбрасываем Парсер
    pub fn reset(&mut self) {
        self.tokens.clear();
        self.pos = 0;
        self.scope.clear();
        self.in_scope.clear();
        self.code_1c.clear();
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

        // println!("scope: {:#?}", self.in_scope);
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

            // __ Условие для ;; Если встретили токен точки с запятой без выражения — просто съедаем его
            // Если встретили токен точки с запятой без выражения — просто съедаем его
            // if self.match_token(&[TokenType::SEMICOLON]).is_some() {
            //     continue;
            // }

            statements.push(self.parse_expression());
            self.require(&[TokenType::SEMICOLON]);
        }
        ExpressionNode::Statements(statements)
    }

    fn parse_expression(&mut self) -> ExpressionNode {
        // 1. Проверяем на пустую точку с запятой (пустое выражение)
        // Если текущий токен — точка с запятой, возвращаем "пустоту"
        if self.tokens[self.pos].token_type == TokenType::SEMICOLON {
            return ExpressionNode::None;
        }

        // 2. Проверяем на "Если"
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

        // 3. Проверяем на присваивание
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

        // 4. Иначе это просто формула (математика/сравнение)
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


        println!("Procedure code: {}", self.code_1c);
        self.tokens
            .iter()
            .enumerate() // Добавляет счетчик (0, 1, 2...)
            .for_each(|(i, token)| {
                println!("{i}: {token:?}");
            });

        let current = &self.tokens[self.pos];
        panic!("Неожиданный токен {:?} на позиции {}", current.token_type, self.pos);
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

            // __ Условие для ;; Если встретили токен точки с запятой без выражения — просто съедаем его
            // if self.match_token(&[TokenType::SEMICOLON]).is_some() {
            //     continue;
            // }

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
            ExpressionNode::None => 0.0, // Просто ничего не делаем
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
