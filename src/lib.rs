//! # Hello World
//! this is some important stuff
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use omop_types::NewConceptId;
use sqlx::{Acquire, PgPool, Pool, Postgres};
pub mod omop_types;

/// Used to create axum Router types that can be used elsewhere
///
pub async fn create_app(pool: Pool<Postgres>) -> Router {
    

    Router::new()
        .route("/concepts", get(get_concepts))
        .route("/concept", post(new_concept))
        .route(
            "/concept/:concept_id",
            get(get_concept)
                .delete(delete_concept)
                .patch(update_target_concept),
        )
        .route(
            "/concept/:concept_id/target",
            get(get_source_concept_target),
        )
        .route("/concept_relationships", get(get_concept_relationships))
        .route("/heartbeat", get(heartbeat))
        .with_state(pool)
}

pub async fn get_concepts(State(pool): State<PgPool>) -> impl IntoResponse {
    let concepts = sqlx::query_as!(
        omop_types::Concept,
        r#"
        SELECT
            concept_id,
            concept_name,
            domain_id,
            vocabulary_id,
            concept_class_id,
            standard_concept,
            concept_code,
            valid_start_date,
            valid_end_date,
            invalid_reason
        FROM mappings.concept;
        "#
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    (StatusCode::OK, Json(concepts))
}

async fn get_concept(State(pool): State<PgPool>, Path(concept_id): Path<i32>) -> Response {
    match sqlx::query_as!(
        omop_types::Concept,
        r#"
        SELECT
            concept_id,
            concept_name,
            domain_id,
            vocabulary_id,
            concept_class_id,
            standard_concept,
            concept_code,
            valid_start_date,
            valid_end_date,
            invalid_reason
        FROM mappings.concept 
        WHERE concept_id = $1;
        "#,
        concept_id
    )
    .fetch_one(&pool)
    .await
    {
        Ok(concept) => (StatusCode::OK, Json(concept)).into_response(),
        Err(sqlx::Error::RowNotFound) => {
            (StatusCode::NOT_FOUND, "Concept not found").into_response()
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response(),
    }
}

async fn get_source_concept_target(
    State(pool): State<PgPool>,
    Path(concept_id): Path<i32>,
) -> Response {
    let q = sqlx::query_as!(
        omop_types::Concept,
        r#"
        SELECT ct.*
        FROM mappings.concept_relationship cr
        INNER JOIN omop_vocab.concept ct
            ON cr.concept_id_2 = ct.concept_id
        WHERE cr.concept_id_1 = $1
          AND cr.invalid_reason IS NULL
        "#,
        concept_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    (StatusCode::OK, Json(q)).into_response()
}

async fn new_concept(
    State(pool): State<PgPool>,
    Json(payload): Json<omop_types::MappedConcept>,
) -> Response {
    let check_concept_exists = sqlx::query_as!(
        omop_types::Concept,
        r#"
        SELECT * FROM mappings.concept
        WHERE 
            domain_id = $1
        AND vocabulary_id = $2
        AND concept_code = $3
        AND concept_class_id = $4
        AND invalid_reason IS NULL
        "#,
        payload.domain_id,
        payload.vocabulary_id,
        payload.concept_code,
        payload.concept_class_id,
    )
    .fetch_all(&pool)
    .await;

    match check_concept_exists {
        Ok(res) => {
            if !res.is_empty() {
                return (StatusCode::CONFLICT, "Duplicate entry").into_response();
            }
        }
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
        }
    }

    let check_target_concept_query = sqlx::query_as!(
        omop_types::Concept,
        r#"
        SELECT * FROM omop_vocab.concept
        WHERE 
            concept_id = $1
        "#,
        payload.maps_to_concept_id
    )
    .fetch_all(&pool)
    .await;

    match check_target_concept_query {
        Ok(res) => {
            if res.is_empty() {
                return (StatusCode::NOT_FOUND, "OMOP Concept does not exist").into_response();
            }
        }
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
        }
    }

    let query_response = sqlx::query_as!(
        omop_types::NewConceptId,
        r#"
        WITH concept_insert AS (
            INSERT INTO mappings.concept (concept_name, domain_id, vocabulary_id, concept_class_id,
                                        concept_code)
                VALUES ($1, $2, $3, $4, $5)
                RETURNING concept_id
            )

        INSERT INTO mappings.concept_relationship (concept_id_1, concept_id_2, relationship_id)
            VALUES ((SELECT concept_id FROM concept_insert), $6, 'Maps to')
        RETURNING (SELECT concept_id FROM concept_insert);
    "#,
        payload.concept_name,
        payload.domain_id,
        payload.vocabulary_id,
        payload.concept_class_id,
        payload.concept_code,
        payload.maps_to_concept_id
    )
    .fetch_one(&pool)
    .await;

    match query_response {
        Ok(res) => {
            (StatusCode::OK, Json(res)).into_response()
        }
        Err(sqlx::Error::Database(e)) if e.code().as_deref() == Some("23505") => {
            (StatusCode::CONFLICT, "Duplicate entry").into_response()
        }
        Err(_) => {
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
        }
    }
}

async fn delete_concept(State(pool): State<PgPool>, Path(concept_id): Path<i32>) -> Response {
    let mut conn = pool.acquire().await.unwrap();
    let mut tx = conn.begin().await.unwrap();

    let update_concept_result = sqlx::query_as!(
        omop_types::NewConceptId,
        r#"
        UPDATE mappings.concept
            SET valid_end_date = now(),
                invalid_reason = 'D'
        WHERE concept_id = $1
          AND invalid_reason IS NULL
        RETURNING concept_id;
        "#,
        concept_id
    )
    .fetch_one(&mut *tx)
    .await;

    match update_concept_result {
        Ok(_) => {}
        Err(sqlx::Error::RowNotFound) => {
            return (StatusCode::NOT_FOUND, "Concept not found").into_response()
        }
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed during deletion process.",
            )
                .into_response()
        }
    }

    sqlx::query!(
        r#"
        UPDATE mappings.concept_relationship
            SET valid_end_date = now(),
                invalid_reason = 'D'
        WHERE concept_id_1 = $1;
        "#,
        concept_id
    )
    .execute(&mut *tx)
    .await
    .unwrap();

    tx.commit().await.expect("Unable to delete concept.");

    (StatusCode::OK).into_response()
}

async fn update_target_concept(
    State(pool): State<PgPool>,
    Path(concept_id): Path<i32>,
    Json(payload): Json<NewConceptId>,
) -> Response {
    let mut conn = pool.acquire().await.unwrap();
    let mut tx = conn.begin().await.unwrap();

    let update_guard = sqlx::query_as!(
        omop_types::ConceptRelationship,
        r#"
        WITH updated AS (
        UPDATE mappings.concept_relationship
            SET valid_end_date = NOW(),
                invalid_reason = 'U'
            WHERE concept_id_1 = $1
              AND invalid_reason IS NULL
        RETURNING *
                )
        SELECT * FROM updated;
        "#,
        concept_id
    )
    .fetch_all(&mut *tx)
    .await;

    match update_guard {
        Ok(res) if res.is_empty() => {
            return (StatusCode::NOT_FOUND).into_response();
        }
        Ok(_) => {}
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response()
        }
    }

    sqlx::query!(
        r#"
        INSERT INTO mappings.concept_relationship (concept_id_1, concept_id_2, relationship_id, valid_start_date, valid_end_date)
        VALUES ($1, $2, 'Maps to', now(), DATE('2099-12-31'));
        "#,
        concept_id,
        payload.concept_id
    )
    .execute(&mut *tx)
    .await
    .expect("Unable to insert new mapping relationship");

    tx.commit()
        .await
        .expect("Unable to commit updated targets.");

    (StatusCode::OK).into_response()
}

async fn get_concept_relationships(State(pool): State<PgPool>) -> Response {
    let response = sqlx::query_as!(
        omop_types::ConceptRelationship,
        r#"
        SELECT concept_id_1, concept_id_2, relationship_id, valid_start_date, valid_end_date, invalid_reason
        FROM mappings.concept_relationship
        "#
    ).fetch_all(&pool).await.unwrap();

    (StatusCode::OK, Json(response)).into_response()
}

async fn heartbeat() -> Response {
    (StatusCode::OK).into_response()
}
