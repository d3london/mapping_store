use axum::body::Body;
use core::time;
use hyper::{Request, StatusCode};
use mapping_manager::create_app;
use sqlx::migrate::Migrator;
use sqlx::postgres::PgConnectOptions;
use std::{net::SocketAddr, thread};
use testcontainers::{core::WaitFor, runners::AsyncRunner, ContainerAsync, GenericImage, ImageExt};

const DB_NAME: &str = "postgres";
const DB_USER: &str = "postgres";
const DB_PASSWORD: &str = "postgres";

/// The container needs to stay alive for the duration of the test.
/// We will return the container along with the connection details.
async fn setup_postgres_test_container() -> (ContainerAsync<GenericImage>, String, u16) {
    let container = GenericImage::new("postgres", "16.3")
        .with_wait_for(WaitFor::message_on_stdout(
            "database system is ready to accept connections",
        ))
        .with_env_var("POSTGRES_DB".to_string(), DB_NAME)
        .with_env_var("POSTGRES_USER".to_string(), DB_USER)
        .with_env_var("POSTGRES_PASSWORD".to_string(), DB_PASSWORD)
        .start()
        .await
        .expect("Failed to start Postgres");

    let host = container.get_host().await.expect("Get postgres host");
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("Get mapped postgres port");

    (container, host.to_string(), port)
}

async fn create_test_instance() -> (ContainerAsync<GenericImage>, SocketAddr) {
    let (pg_instance, host, port) = setup_postgres_test_container().await;

    let connect_settings = PgConnectOptions::new()
        .database("postgres")
        .host(&host)
        .port(port)
        .username(DB_USER)
        .password(DB_PASSWORD)
        .database(DB_NAME);

    let pool = sqlx::PgPool::connect_with(connect_settings)
        .await
        .expect("Get postgres pool");

    sqlx::migrate!().run(&pool).await.expect("Migrate database"); // defaults to "./migrations"

    let app = create_app(pool).await;
    let listener = tokio::net::TcpListener::bind(&"localhost:0").await.unwrap();
    let addr = listener.local_addr().expect("Get test app address");
    tokio::spawn(async move { axum::serve(listener, app).await.expect("start axum server") });

    (pg_instance, addr) // Return the container and address
}

#[tokio::test]
async fn test_route() {
    let (_pg_container, addr) = create_test_instance().await;

    let client =
        hyper_util::client::legacy::Builder::new(hyper_util::rt::TokioExecutor::new()).build_http();

    let response = client
        .request(
            Request::builder()
                .uri(format!("http://{addr}/concepts"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // The container will be dropped here, which will stop it
}
