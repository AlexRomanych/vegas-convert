use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Postgres, Transaction};

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct CuttingTaskLine {
    pub id:                i64,
    pub cut_length:        i32,
    pub cut_width:         i32,
    pub cut_detail_amount: i32,
    pub detail:            Option<String>,
    pub angle:             Option<String>,
}


impl CuttingTaskLine {
    pub const PANEL_NAME: &'static str = "panel"; // __ Верхняя и нижняя крышки одинаковы
    pub const PANEL_UP_NAME: &'static str = "panel_up";
    pub const PANEL_DOWN_NAME: &'static str = "panel_down";
    pub const SIDE_NAME: &'static str = "side";

    pub async fn save_calc_data(&self, tx: &mut Transaction<'_, Postgres>) -> Result<()> {
        sqlx::query!(
            r#"
                UPDATE cutting_task_lines 
                SET 
                    cut_length = $1, 
                    cut_width = $2, 
                    cut_detail_amount = $3, 
                    detail = $4, 
                    angle = $5
                WHERE id = $6
            "#,
            self.cut_length,
            self.cut_width,
            self.cut_detail_amount,
            self.detail,
            self.angle,
            self.id // Передаем id шестым параметром для WHERE
        )
        .execute(&mut **tx)
        .await?; // Дожидаемся ответа от базы данных

        Ok(())
    }
}
