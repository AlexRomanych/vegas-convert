use sqlx::PgPool;
use tracing::{Event, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

pub struct PostgresLayer {
    pool: PgPool,
}

impl PostgresLayer {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl<S> Layer<S> for PostgresLayer
where
    S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let pool = self.pool.clone();

        // Извлекаем данные из события
        let metadata = event.metadata();
        let level = metadata.level().to_string();
        let target = metadata.target().to_string();

        // Важно: запись в БД должна быть асинхронной.
        // Мы используем tokio::spawn, чтобы не блокировать основной поток парсинга.
        tokio::spawn(async move {
            let _ = sqlx::query("INSERT INTO logs (level, target, message, created_at) VALUES ($1, $2, $3, NOW())")
                .bind(level)
                .bind(target)
                .bind("Тут логика извлечения текста сообщения")
                .execute(&pool)
                .await;
        });
    }
}



pub async fn init_logger(pool: PgPool) {
    let pg_layer = PostgresLayer::new(pool);

    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with(fmt::layer()) // Дублируем логи в консоль для удобства
        .with(pg_layer) // Наш слой для БД
        .init();
}
