use axum::body::Body;
use http_body_util::BodyExt;
use hyper::{body::Incoming, header, Request, Response, StatusCode};
use mapping_manager::create_app;
use sqlx::postgres::PgConnectOptions;
use std::net::SocketAddr;
use testcontainers::{core::WaitFor, runners::AsyncRunner, ContainerAsync, GenericImage, ImageExt};

use mapping_manager::omop_types;

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

    sqlx::migrate!("./tests/data")
        .run(&pool)
        .await
        .expect("Setting up test database DDL/DML");

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

    // Testing GET request
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

    // Create an example concept for the POST request
    let example_concept = omop_types::MappedConcept {
        concept_name: "FBC_Haemoglobin".to_string(),
        domain_id: "LIMS.BloodResults".to_string(),
        vocabulary_id: "GSTT".to_string(),
        concept_class_id: "Observable Entity".to_string(),
        concept_code: "FBC_Hb_Mass".to_string(),
        maps_to_concept_id: 37171451,
    };

    // Serialize the concept to JSON
    let concept_json = serde_json::to_string(&example_concept).unwrap();

    // Send the POST request with JSON body
    let response = client
        .request(
            Request::builder()
                .method("POST")
                .uri(format!("http://{addr}/concept"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(concept_json))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let new_concept_string = convert_body_to_string(response).await;

    let re: omop_types::NewConceptId = serde_json::from_str(&new_concept_string).unwrap();

    dbg!(re);
}

async fn convert_body_to_string(body: Response<Incoming>) -> String {
    String::from_utf8(
        body.into_body()
            .collect()
            .await
            .expect("Collect bytes from incoming")
            .to_bytes()
            .into(),
    )
    .expect("Convert bytes to string")
}
