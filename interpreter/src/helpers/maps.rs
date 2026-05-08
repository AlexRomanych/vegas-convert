use crate::TokenType;
use regex::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;

// __ Карта токенов
pub static TOKEN_MAP: OnceLock<Vec<(TokenType, Regex)>> = OnceLock::new();
pub fn get_token_map() -> &'static Vec<(TokenType, Regex)> {
    TOKEN_MAP.get_or_init(|| {
        vec![
            // !!! Тут важен порядок
            // 1. Property: [Объект].{Параметр}
            // Самый длинный и специфичный — первым
            (TokenType::PROPERTY, Regex::new(r"^\[[^]]+\]\.\{[^}]+\}").unwrap()),
            // 2. Parameter: [Объект].[Свойство]
            // Средний по сложности
            (TokenType::PARAMETER, Regex::new(r"^\[[^]]+\]\.\[[^]]+\]").unwrap()),
            // (TokenType::PARAMETER, Regex::new(r"^(\[[^\]]+\]\.\[[^\]]+\])\s*([^\s;]?)").unwrap()),
            // 3. Return: [Объект]
            // Самый короткий — последним из этой группы
            (TokenType::RETURN, Regex::new(r"^\[[^]]+\]").unwrap()),
            // !!! End block
            (TokenType::SEMICOLON, Regex::new("^;").unwrap()),
            (TokenType::COMMA, Regex::new(r"^,").unwrap()),
            (TokenType::SPACE, Regex::new(r"^\s+").unwrap()),
            (TokenType::ASSIGN, Regex::new("^=").unwrap()),
            (TokenType::PLUS, Regex::new(r"^\+").unwrap()),
            (TokenType::MINUS, Regex::new(r"^-").unwrap()),
            (TokenType::AND, Regex::new(r"(?i)^(и|and)\b").unwrap()),
            (TokenType::OR, Regex::new(r"(?i)^(или|or)\b").unwrap()),
            (TokenType::NOT, Regex::new(r"(?i)^(не|not)\b").unwrap()),
            // !!! Тут важен порядок
            (TokenType::GE, Regex::new(r"^>=").unwrap()), // Больше или равно
            (TokenType::LE, Regex::new(r"^<=").unwrap()), // Меньше или равно
            (TokenType::GT, Regex::new(r"^>").unwrap()),  // Больше
            (TokenType::LT, Regex::new(r"^<").unwrap()),  // Меньше
            (TokenType::NE, Regex::new(r"^<>").unwrap()), // Не равно (в стиле 1С/SQL)
            // !!! End block
            (TokenType::STAR, Regex::new(r"^\*").unwrap()), // Для умножения "*"
            (TokenType::SLASH, Regex::new(r"^/").unwrap()), // Для деления "/"
            (TokenType::LPAR, Regex::new(r"^\(").unwrap()), // Левая скобка "("
            (TokenType::RPAR, Regex::new(r"^\)").unwrap()), // Правая скобка ")"
            // (TokenType::FIX, Regex::new(r"(?i)^(цел)\b").unwrap()),
            // (TokenType::ROUND, Regex::new(r"(?i)^(окр)\b").unwrap()),
            // (TokenType::ALARM, Regex::new(r"(?i)^(предупреждение)\b").unwrap()),
            // (TokenType::MISSING, Regex::new(r"(?i)^(ЗначениеЗаполнено)\b").unwrap()),
            // Строка: "Не задано количество слоев клея"
            (TokenType::STRING, Regex::new(r#"^"[^"]*""#).unwrap()),
            // (TokenType::STRING, Regex::new(r#"^"([^"]*)""#).unwrap()),
            // Если / If
            (TokenType::IF, Regex::new(r"(?i)^(если|if)\b").unwrap()),
            // ИначеЕсли / ElseIf
            (TokenType::ELSEIF, Regex::new(r"(?i)^(иначеесли|elseif)\b").unwrap()),
            // Иначе / Else
            (TokenType::ELSE, Regex::new(r"(?i)^(иначе|else)\b").unwrap()),
            // КонецЕсли / EndIf
            (TokenType::ENDIF, Regex::new(r"(?i)^(конецесли|endif)\b").unwrap()),
            // Тогда / Then
            (TokenType::THEN, Regex::new(r"(?i)^(тогда|then)\b").unwrap()),
            // !!! Оставляем переменную в самом конце
            (TokenType::NUMBER, Regex::new(r"^[0-9]+([.,][0-9]+)?").unwrap()),
            (TokenType::VARIABLE, Regex::new(r"(?i)^[а-яёa-z_][а-яёa-z0-9_]*").unwrap()),
            // (TokenType::VARIABLE, Regex::new(r"^[а-яА-Яa-zA-Z_][а-яА-Яa-zA-Z0-9_]*").unwrap()),
            // (TokenType::VARIABLE, Regex::new(r"^[а-яА-Я_]+").unwrap()),
        ]
    })
}


// __ Ключевые слова (Если, Тогда, Иначе)
pub static KEYWORDS: OnceLock<HashMap<&str, &str>> = OnceLock::new();
pub fn get_keywords() -> &'static HashMap<&'static str, &'static str> {
    KEYWORDS.get_or_init(|| {
        HashMap::from([
            // ("если", "if"),
            // ("иначеесли", "Else If"),
            // ("иначе", "Else"),
            // ("конецесли", "End If"),
            // ("тогда", "Then"),
            // ("не", "Not"),
            // ("или", "Or"),
            // ("и", "And"),
        ])
    })
}

// __ Операторы (Цел, Окр)
pub static OPERATORS: OnceLock<HashMap<&str, &str>> = OnceLock::new();
pub fn get_operators() -> &'static HashMap<&'static str, &'static str> {
    OPERATORS.get_or_init(|| HashMap::from([("цел", "Fix"), ("окр", "Round")]))
}
