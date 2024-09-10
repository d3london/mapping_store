-- Add migration script here
CREATE SCHEMA omop_vocab;

CREATE TABLE omop_vocab.concept
(
    concept_id       integer      NOT NULL PRIMARY KEY,
    concept_name     varchar(255) NOT NULL,
    domain_id        varchar(20)  NOT NULL,
    vocabulary_id    varchar(20)  NOT NULL,
    concept_class_id varchar(20)  NOT NULL,
    standard_concept varchar(1)   NULL,
    concept_code     varchar(50)  NOT NULL,
    valid_start_date date         NOT NULL,
    valid_end_date   date         NOT NULL,
    invalid_reason   varchar(1)   NULL
);

CREATE SCHEMA mappings;

CREATE TABLE mappings.concept
(
    concept_id       integer GENERATED ALWAYS AS IDENTITY (START WITH 2000000000) NOT NULL
        CONSTRAINT concept_id_pk
            PRIMARY KEY
        CONSTRAINT concept_pk
            UNIQUE,
    concept_name     varchar                                                      NOT NULL,
    domain_id        varchar                                                      NOT NULL,
    vocabulary_id    varchar                                                      NOT NULL,
    concept_class_id varchar                                                      NOT NULL,
    standard_concept varchar DEFAULT NULL,
    concept_code     varchar                                                      NOT NULL,
    valid_start_date date    DEFAULT NOW()                                        NOT NULL,
    valid_end_date   date    DEFAULT date('2099-12-31')                           NOT NULL,
    invalid_reason   varchar
);

CREATE UNIQUE INDEX concept_concept_id_index
    ON mappings.concept (domain_id, vocabulary_id, concept_code)
    WHERE invalid_reason IS NULL;

CREATE TABLE mappings.concept_relationship
(
    concept_id_1     integer                         NOT NULL REFERENCES mappings.concept (concept_id),
    concept_id_2     integer                         NOT NULL REFERENCES omop_vocab.concept (concept_id),
    relationship_id  varchar(20)                     NOT NULL,
    valid_start_date date DEFAULT NOW()              NOT NULL,
    valid_end_date   date DEFAULT date('2099-12-31') NOT NULL,
    invalid_reason   varchar(1),
    CONSTRAINT unique_live_mapping_constraint
        UNIQUE (concept_id_1, concept_id_2, valid_end_date)
);
