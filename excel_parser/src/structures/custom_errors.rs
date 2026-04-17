use serde_json::json;
use logger::structures::log_message::{LogLevel, LogMessage, LogTarget};

pub enum CustomError {
    ErrorStructureMaterialsFile,
    ErrorStructureProceduresFile,
    ErrorStructureModelsFile,
    ErrorStructureSpecificationsFile,
}


impl CustomError {
    pub fn get_error_code(&self) -> usize {
        match self {
            CustomError::ErrorStructureMaterialsFile => 0b0000_0000_0000_0001,
            CustomError::ErrorStructureProceduresFile => 0b0000_0000_0000_0010,
            CustomError::ErrorStructureModelsFile => 0b0000_0000_0000_0100,
            CustomError::ErrorStructureSpecificationsFile => 0b0000_0000_0000_1000,
        }
    }

    pub fn get_log_message(&self) -> LogMessage {
        match self {
            CustomError::ErrorStructureMaterialsFile => LogMessage {
                level: LogLevel::ERROR,
                target: LogTarget::ModelsUpdate,
                message: "Неверная структура Excel файла материалов".to_string(),
                context: None,
                created_at: None,
            },
            CustomError::ErrorStructureProceduresFile => LogMessage {
                level: LogLevel::ERROR,
                target: LogTarget::ModelsUpdate,
                message: "Неверная структура Excel файла процедур расчета".to_string(),
                context: None,
                created_at: None,
            },
            CustomError::ErrorStructureModelsFile => LogMessage {
                level: LogLevel::ERROR,
                target: LogTarget::ModelsUpdate,
                message: "Неверная структура Excel файла моделей".to_string(),
                context: None,
                created_at: None,
            },
            CustomError::ErrorStructureSpecificationsFile => LogMessage {
                level: LogLevel::ERROR,
                target: LogTarget::ModelsUpdate,
                message: "Неверная структура Excel файла спецификаций".to_string(),
                context: None,
                created_at: None,
            },
        }
    }
}
