//! # Hello World
//! this is some important stuff
mod omop_types;
use mapping_manager::create_app;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .init();

    let conn_str = std::env::var("DATABASE_URL").expect("Env var DATABASE_URL is required.");
    let pool = sqlx::PgPool::connect(&conn_str)
        .await
        .expect("Unable to create pool.");

    let app = create_app(pool).await;

    let listener = tokio::net::TcpListener::bind(&"0.0.0.0:7777")
        .await
        .unwrap();

    tracing::debug!("Listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();

    Ok(())
}
