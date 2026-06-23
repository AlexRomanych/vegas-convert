use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use crate::structures::cutting_task_line::CuttingTaskLine;

#[derive(Debug, Serialize, FromRow, Deserialize, Clone)]
pub struct CuttingTask {
    pub id: i64,
    pub cutting_task_lines: Vec<CuttingTaskLine>,
}
