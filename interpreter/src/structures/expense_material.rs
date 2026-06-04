use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
// __ Импортируем FromRow для SELECT запросов в эту структуру
use sqlx::postgres::PgArguments;
use sqlx::query::Query;
use sqlx::{FromRow, Postgres, Transaction};
use crate::helpers::functions::round_to_precision;

// Выносим константную строку запроса наружу имплементации
static INSERT_QUERY: LazyLock<String> = LazyLock::new(|| {
    format!(
        r#"
            INSERT INTO {} (
                order_line_id, material_code_1c, material_code_1c_copy,
                expense_per_pic, expense, rest_per_pic, rest,
                unit, detail, procedure, object_name, position,
                scopes, outputs,
                updated_at, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, NOW(), NOW())
        "#,
        ExpenseMaterial::EXPENSE_MATERIALS_TABLE_NAME
    )
});

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ScopeItem {
    pub n: String, // __ name
    pub v: f64,    // __ value
}

#[derive(sqlx::FromRow, Serialize, Deserialize, Debug, Clone, Default)]
pub struct ExpenseMaterial {
    pub order_line_id:               i64,            // __ Привязка к линии Заявки
    pub material_code_1c:            Option<String>, // __ Материал
    pub material_code_1c_copy:       Option<String>, // __ Копия ссылки на материал
    pub expense_per_pic:             f64,            // __ Расход на единицу
    pub expense:                     f64,            // __ Общий расход
    pub rest_per_pic:                f64,            // __ Остаток на единицу
    pub rest:                        f64,            // __ Общий остаток
    pub unit:                        Option<String>, // __ Единица измерения
    pub detail:                      Option<String>, // __ Название детали из спецификации
    pub procedure:                   Option<String>, // __ Название процедуры, если есть
    pub object_name:                 Option<String>, // __ Название объекта процедуры, к которому она была применена
    pub position:                    Option<i16>,    // __ Позиция в списке записей спецификации
    pub material_name_specification: Option<String>, // __ Название материала в спецификациях (Вход процедуры)
    pub material_name_expense:       Option<String>, // __ Название материала в расходе (Выход процедуры)

    #[sqlx(json)]
    pub scopes:  Vec<ScopeItem>, // __ Переменные, которые получили в процедуре расчета
    #[sqlx(json)]
    pub outputs: Vec<ScopeItem>, // __ Выходные свойства материала
}

impl ExpenseMaterial {
    pub const EXPENSE_MATERIALS_TABLE_NAME: &'static str = "order_line_material_pivot";

    pub fn new() -> Self {
        Self::default()
    }

    // __ Сохраняем в базе
    pub async fn save_record(self, tx: &mut Transaction<'_, Postgres>) -> Result<(), sqlx::Error> {
        // 1. Формируем SQL-строку динамически
        let query_string = r#"
                INSERT INTO order_line_material_pivot (
                    order_line_id, material_code_1c, material_code_1c_copy,
                    expense_per_pic, expense, rest_per_pic, rest,
                    unit, detail, procedure, object_name, position,
                    scopes, outputs,
                    material_name_specification, material_name_expense,
                    updated_at, created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, NOW(), NOW())
            "#;
        let precision = 4u32;
        sqlx::query(query_string)
            .bind(self.order_line_id)
            .bind(self.material_code_1c)
            .bind(self.material_code_1c_copy)
            .bind(round_to_precision(self.expense_per_pic, precision))
            .bind(round_to_precision(self.expense, precision))
            .bind(round_to_precision(self.rest_per_pic, precision))
            .bind(round_to_precision(self.rest, precision))
            .bind(self.unit)
            .bind(self.detail)
            .bind(self.procedure)
            .bind(self.object_name)
            .bind(self.position)
            // Для JSON полей передаем десериализованный Value напрямую
            .bind(serde_json::to_value(&self.scopes).unwrap())
            .bind(serde_json::to_value(&self.outputs).unwrap())

            .bind(self.material_name_specification)
            .bind(self.material_name_expense)
            // Используем двойную разыменовку, чтобы достучаться до Executor у транзакции
            .execute(&mut **tx)
            .await?;

        Ok(())
    }
}
