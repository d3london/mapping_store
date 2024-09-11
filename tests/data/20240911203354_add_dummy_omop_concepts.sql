-- Add migration script here
INSERT INTO omop_vocab.concept (concept_id, concept_name, domain_id, vocabulary_id, concept_class_id, standard_concept, concept_code, valid_start_date, valid_end_date, invalid_reason) 
VALUES (37171451, 'HbA1c (glycated haemoglobin A1c)/HbA1 (haemoglobin A1) percent in blood', 'Measurement', 'SNOMED', 'Observable Entity', 'S', '3531000237106', '2023-06-07', '2099-12-31', null);

INSERT INTO omop_vocab.concept (concept_id, concept_name, domain_id, vocabulary_id, concept_class_id, standard_concept, concept_code, valid_start_date, valid_end_date, invalid_reason) 
VALUES (37208644, 'Haemoglobin mass concentration in blood', 'Measurement', 'SNOMED', 'Observable Entity', 'S', '1107511000000100', '2019-06-01', '2099-12-31', null);
