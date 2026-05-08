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
    // FIX,     // Цел
    // ROUND,   // Окр
    // ALARM,   // Предупреждение
    // MISSING, // ЗначениеЗаполнено
    STRING,  // "Не задано количество слоев клея"
}

impl Default for TokenType {
    fn default() -> Self {
        Self::UNDEFINED
    }
}


#[derive(Debug, Default, Clone)]
pub struct Token {
    pub token_type: TokenType,
    pub text:       String,
    pub pos:        usize,
}
