use crate::structures::custom_errors::CustomError;

// __ Трейт, для определения правил объекта
pub trait ExcelPattern {
    const CHECK_PATTERN: &'static [(usize, &'static str)];
    fn get_check_row() -> usize;    // __ Ряд проверяемых данных
    
    fn get_error() -> CustomError;  // __ Ассоциированная с типом ошибка
}

