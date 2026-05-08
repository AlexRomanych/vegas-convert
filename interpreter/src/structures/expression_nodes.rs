use crate::structures::tokens::Token;

#[derive(Debug, Clone, Default)]
pub struct IfBranch {
    pub condition: ExpressionNode,
    pub body:      Vec<ExpressionNode>,
}


#[derive(Debug, Clone, Default)]
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

    #[default]
    None,
}
