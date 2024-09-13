use axum::body::Body;
use http_body_util::BodyExt;
use hyper::{body::Incoming, header, Request, Response, StatusCode};
use hyper_util::client::legacy::connect::HttpConnector;
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
    println!("        \x1b[93mSetup:\x1b[0m Spinning up test Postgres database.");

    let container = GenericImage::new("postgres", "16.3")
        .with_wait_for(WaitFor::message_on_stdout(
            "database system is ready to accept connections",
        ))
        .with_wait_for(WaitFor::seconds(5))
        .with_env_var("POSTGRES_DB".to_string(), DB_NAME)
        .with_env_var("POSTGRES_USER".to_string(), DB_USER)
        .with_env_var("POSTGRES_PASSWORD".to_string(), DB_PASSWORD)
        .start()
        .await
        .expect("Failed to start Postgres");

    println!("        \x1b[93mSetup:\x1b[0m Postgres container created and ready.");

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

    println!("        \x1b[93mSetup:\x1b[0m Migrating database.");
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
async fn integration_tests() {
    let (_pg_container, addr) = create_test_instance().await;

    let client =
        hyper_util::client::legacy::Builder::new(hyper_util::rt::TokioExecutor::new()).build_http();

    print!("\n  \x1b[93mIntegration:\x1b[0m Check heartbeat ... ");

    // Testing GET request
    let response = client
        .request(
            Request::builder()
                .uri(format!("http://{addr}/heartbeat"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    println!("ok");

    print!("  \x1b[93mIntegration:\x1b[0m Querying empty concepts ... ");

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
    let resp = convert_body_to_string(response).await;
    assert_eq!(resp, "[]");

    println!("ok");

    create_incorrect_mapping(&client, &addr).await;
    correct_incorrect_mapping(&client, &addr).await;

    // _pg_container goes out of scope here, therefore invoking Drop()
    println!("\n        \x1b[93mSetup:\x1b[0m Destroying Postgres container.\n");
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

async fn create_incorrect_mapping(
    client: &hyper_util::client::legacy::Client<HttpConnector, Body>,
    address: &SocketAddr,
) {
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

    print!("  \x1b[93mIntegration:\x1b[0m Creating incorrect mapped concept ... ");
    // Send the POST request with JSON body
    let response = client
        .request(
            Request::builder()
                .method("POST")
                .uri(format!("http://{address}/concept"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(concept_json))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let new_concept_string = convert_body_to_string(response).await;

    let new_concept_id: omop_types::NewConceptId =
        serde_json::from_str(&new_concept_string).unwrap();

    assert_eq!(new_concept_id.concept_id.unwrap(), 2_000_000_000);

    println!("ok");
}

async fn correct_incorrect_mapping(
    client: &hyper_util::client::legacy::Client<HttpConnector, Body>,
    address: &SocketAddr,
) {
    // Create an example concept for the POST request
    let new_target_concept_id = omop_types::NewConceptId {
        concept_id: Some(37208644),
    };

    // Serialize the concept to JSON
    let concept_json = serde_json::to_string(&new_target_concept_id).unwrap();

    print!("  \x1b[93mIntegration:\x1b[0m Correct incorrect mapping ... ");
    // Send the POST request with JSON body
    let response = client
        .request(
            Request::builder()
                .method("PATCH")
                .uri(format!("http://{address}/concept/2000000000"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(concept_json))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    println!("ok");
}
