use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Concept {
    pub concept_id: i32,
    pub concept_name: String,
    pub domain_id: String,
    pub vocabulary_id: String,
    pub concept_class_id: String,
    pub standard_concept: Option<String>,
    pub concept_code: String,
    pub valid_start_date: chrono::NaiveDate,
    pub valid_end_date: chrono::NaiveDate,
    pub invalid_reason: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ConceptRelationship {
    pub concept_id_1: i32,
    pub concept_id_2: i32,
    pub relationship_id: String,
    pub valid_start_date: chrono::NaiveDate,
    pub valid_end_date: chrono::NaiveDate,
    pub invalid_reason: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MappedConcept {
    pub concept_name: String,
    pub domain_id: String,
    pub vocabulary_id: String,
    pub concept_class_id: String,
    pub concept_code: String,
    pub maps_to_concept_id: i32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NewConceptId {
    pub concept_id: Option<i32>,
}
