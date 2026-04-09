--
-- PostgreSQL database dump
--

\restrict rzGq6ephmCclt2BQcSrF0jPMGdsMsLC2OUpt5emSTS2eOk8tM55UrNNbMJz7hTL

-- Dumped from database version 16.10
-- Dumped by pg_dump version 16.13 (Ubuntu 16.13-0ubuntu0.24.04.1)

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: _svap_schema; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public._svap_schema (
    id integer DEFAULT 1 NOT NULL,
    version integer DEFAULT 0 NOT NULL,
    CONSTRAINT _svap_schema_id_check CHECK ((id = 1))
);


--
-- Name: assessment_findings; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.assessment_findings (
    assessment_id text NOT NULL,
    finding_id text NOT NULL
);


--
-- Name: calibration; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.calibration (
    id integer DEFAULT 1 NOT NULL,
    run_id text,
    threshold integer NOT NULL,
    correlation_notes text,
    quality_frequency text,
    quality_combinations text,
    created_at text NOT NULL,
    CONSTRAINT calibration_new_id_check CHECK ((id = 1))
);


--
-- Name: cases; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.cases (
    case_id text NOT NULL,
    source_document text,
    case_name text NOT NULL,
    scheme_mechanics text NOT NULL,
    exploited_policy text NOT NULL,
    enabling_condition text NOT NULL,
    scale_dollars real,
    scale_defendants integer,
    scale_duration text,
    detection_method text,
    raw_extraction text,
    created_at text NOT NULL,
    source_doc_id text
);


--
-- Name: chunks; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.chunks (
    chunk_id text NOT NULL,
    doc_id text NOT NULL,
    chunk_index integer NOT NULL,
    text text NOT NULL,
    token_count integer
);


--
-- Name: convergence_scores; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.convergence_scores (
    id integer NOT NULL,
    run_id text NOT NULL,
    case_id text NOT NULL,
    quality_id text NOT NULL,
    present integer NOT NULL,
    evidence text,
    created_at text NOT NULL,
    CONSTRAINT convergence_scores_present_check CHECK ((present = ANY (ARRAY[0, 1])))
);


--
-- Name: convergence_scores_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.convergence_scores_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: convergence_scores_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.convergence_scores_id_seq OWNED BY public.convergence_scores.id;


--
-- Name: detection_patterns; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.detection_patterns (
    pattern_id text NOT NULL,
    run_id text NOT NULL,
    prediction_id text,
    data_source text NOT NULL,
    anomaly_signal text NOT NULL,
    baseline text,
    false_positive_risk text,
    detection_latency text,
    priority text,
    implementation_notes text,
    created_at text NOT NULL,
    step_id text,
    CONSTRAINT detection_patterns_priority_check CHECK ((priority = ANY (ARRAY['critical'::text, 'high'::text, 'medium'::text, 'low'::text])))
);


--
-- Name: dimension_registry; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.dimension_registry (
    dimension_id text NOT NULL,
    name text NOT NULL,
    definition text NOT NULL,
    probing_questions text,
    origin text NOT NULL,
    related_quality_ids text,
    created_at text NOT NULL,
    created_by text,
    CONSTRAINT dimension_registry_origin_check CHECK ((origin = ANY (ARRAY['case_derived'::text, 'policy_derived'::text, 'manual'::text, 'seed'::text])))
);


--
-- Name: documents; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.documents (
    doc_id text NOT NULL,
    filename text,
    doc_type text,
    full_text text NOT NULL,
    metadata text,
    created_at text NOT NULL,
    CONSTRAINT documents_doc_type_check CHECK ((doc_type = ANY (ARRAY['enforcement'::text, 'policy'::text, 'guidance'::text, 'report'::text, 'other'::text])))
);


--
-- Name: enforcement_sources; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.enforcement_sources (
    source_id text NOT NULL,
    name text NOT NULL,
    url text,
    source_type text DEFAULT 'press_release'::text NOT NULL,
    description text,
    has_document boolean DEFAULT false NOT NULL,
    s3_key text,
    doc_id text,
    summary text,
    validation_status text DEFAULT 'pending'::text,
    created_at text NOT NULL,
    updated_at text NOT NULL,
    candidate_id text,
    feed_id text,
    CONSTRAINT enforcement_sources_validation_status_check CHECK ((validation_status = ANY (ARRAY['pending'::text, 'valid'::text, 'invalid'::text, 'error'::text])))
);


--
-- Name: exploitation_steps; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.exploitation_steps (
    step_id text NOT NULL,
    tree_id text NOT NULL,
    parent_step_id text,
    step_order integer NOT NULL,
    title text NOT NULL,
    description text NOT NULL,
    actor_action text,
    is_branch_point boolean DEFAULT false,
    branch_label text,
    created_at text NOT NULL
);


--
-- Name: exploitation_trees; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.exploitation_trees (
    tree_id text NOT NULL,
    policy_id text NOT NULL,
    convergence_score integer NOT NULL,
    actor_profile text,
    lifecycle_stage text,
    detection_difficulty text,
    review_status text DEFAULT 'draft'::text,
    reviewer_notes text,
    run_id text,
    created_at text NOT NULL,
    CONSTRAINT exploitation_trees_review_status_check CHECK ((review_status = ANY (ARRAY['draft'::text, 'approved'::text, 'rejected'::text, 'revised'::text])))
);


--
-- Name: pipeline_runs; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.pipeline_runs (
    run_id text NOT NULL,
    created_at text NOT NULL,
    config_snapshot text NOT NULL,
    notes text
);


--
-- Name: policies; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.policies (
    policy_id text NOT NULL,
    name text NOT NULL,
    description text,
    source_document text,
    structural_characterization text,
    created_at text NOT NULL,
    lifecycle_status text DEFAULT 'cataloged'::text,
    lifecycle_updated_at text
);


--
-- Name: policy_scores; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.policy_scores (
    id integer NOT NULL,
    run_id text NOT NULL,
    policy_id text NOT NULL,
    quality_id text NOT NULL,
    present integer NOT NULL,
    evidence text,
    created_at text NOT NULL,
    CONSTRAINT policy_scores_present_check CHECK ((present = ANY (ARRAY[0, 1])))
);


--
-- Name: policy_scores_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.policy_scores_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: policy_scores_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.policy_scores_id_seq OWNED BY public.policy_scores.id;


--
-- Name: prediction_qualities; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.prediction_qualities (
    prediction_id text NOT NULL,
    quality_id text NOT NULL
);


--
-- Name: predictions; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.predictions (
    prediction_id text NOT NULL,
    run_id text NOT NULL,
    policy_id text NOT NULL,
    convergence_score integer NOT NULL,
    mechanics text NOT NULL,
    enabling_qualities text NOT NULL,
    actor_profile text,
    lifecycle_stage text,
    detection_difficulty text,
    review_status text DEFAULT 'draft'::text,
    reviewer_notes text,
    created_at text NOT NULL
);


--
-- Name: quality_assessments; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.quality_assessments (
    assessment_id text NOT NULL,
    run_id text NOT NULL,
    policy_id text NOT NULL,
    quality_id text NOT NULL,
    taxonomy_version text,
    present text DEFAULT 'uncertain'::text NOT NULL,
    evidence_finding_ids text,
    confidence text DEFAULT 'medium'::text NOT NULL,
    rationale text,
    created_at text NOT NULL,
    CONSTRAINT quality_assessments_confidence_check CHECK ((confidence = ANY (ARRAY['high'::text, 'medium'::text, 'low'::text]))),
    CONSTRAINT quality_assessments_present_check CHECK ((present = ANY (ARRAY['yes'::text, 'no'::text, 'uncertain'::text])))
);


--
-- Name: regulatory_sources; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.regulatory_sources (
    source_id text NOT NULL,
    source_type text NOT NULL,
    url text NOT NULL,
    title text,
    cfr_reference text,
    full_text text NOT NULL,
    fetched_at text NOT NULL,
    metadata text
);


--
-- Name: research_sessions; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.research_sessions (
    session_id text NOT NULL,
    run_id text NOT NULL,
    policy_id text NOT NULL,
    status text DEFAULT 'pending'::text NOT NULL,
    sources_queried text,
    started_at text,
    completed_at text,
    error_message text,
    trigger text DEFAULT 'initial'::text,
    CONSTRAINT research_sessions_status_check CHECK ((status = ANY (ARRAY['pending'::text, 'researching'::text, 'findings_complete'::text, 'assessment_complete'::text, 'failed'::text]))),
    CONSTRAINT research_sessions_trigger_check CHECK ((trigger = ANY (ARRAY['initial'::text, 'taxonomy_change'::text, 'regulatory_change'::text, 'manual'::text])))
);


--
-- Name: source_candidates; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.source_candidates (
    candidate_id text NOT NULL,
    feed_id text,
    title text NOT NULL,
    url text NOT NULL,
    discovered_at text NOT NULL,
    published_date text,
    status text DEFAULT 'discovered'::text NOT NULL,
    richness_score real,
    richness_rationale text,
    estimated_cases integer,
    source_id text,
    doc_id text,
    reviewed_by text DEFAULT 'auto'::text,
    created_at text NOT NULL,
    updated_at text NOT NULL,
    CONSTRAINT source_candidates_status_check CHECK ((status = ANY (ARRAY['discovered'::text, 'fetched'::text, 'scored'::text, 'accepted'::text, 'rejected'::text, 'ingested'::text, 'error'::text])))
);


--
-- Name: source_feeds; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.source_feeds (
    feed_id text NOT NULL,
    name text NOT NULL,
    listing_url text NOT NULL,
    content_type text DEFAULT 'press_release'::text NOT NULL,
    link_selector text,
    last_checked_at text,
    last_entry_url text,
    enabled boolean DEFAULT true,
    created_at text NOT NULL,
    updated_at text NOT NULL
);


--
-- Name: stage_log; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.stage_log (
    id integer NOT NULL,
    run_id text NOT NULL,
    stage integer NOT NULL,
    status text NOT NULL,
    started_at text,
    completed_at text,
    error_message text,
    metadata text,
    task_token text,
    CONSTRAINT stage_log_status_check CHECK ((status = ANY (ARRAY['running'::text, 'completed'::text, 'failed'::text, 'pending_review'::text, 'approved'::text])))
);


--
-- Name: stage_log_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.stage_log_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: stage_log_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.stage_log_id_seq OWNED BY public.stage_log.id;


--
-- Name: stage_processing_log; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.stage_processing_log (
    stage integer NOT NULL,
    entity_id text NOT NULL,
    input_hash text NOT NULL,
    run_id text,
    processed_at text NOT NULL
);


--
-- Name: step_qualities; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.step_qualities (
    step_id text NOT NULL,
    quality_id text NOT NULL
);


--
-- Name: structural_findings; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.structural_findings (
    finding_id text NOT NULL,
    run_id text NOT NULL,
    policy_id text NOT NULL,
    dimension_id text,
    observation text NOT NULL,
    source_type text DEFAULT 'llm_knowledge'::text NOT NULL,
    source_citation text,
    source_text text,
    confidence text DEFAULT 'medium'::text NOT NULL,
    status text DEFAULT 'active'::text NOT NULL,
    stale_reason text,
    created_at text NOT NULL,
    created_by text,
    CONSTRAINT structural_findings_confidence_check CHECK ((confidence = ANY (ARRAY['high'::text, 'medium'::text, 'low'::text]))),
    CONSTRAINT structural_findings_status_check CHECK ((status = ANY (ARRAY['active'::text, 'stale'::text, 'superseded'::text])))
);


--
-- Name: taxonomy; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.taxonomy (
    quality_id text NOT NULL,
    name text NOT NULL,
    definition text NOT NULL,
    recognition_test text NOT NULL,
    exploitation_logic text NOT NULL,
    canonical_examples text,
    review_status text DEFAULT 'draft'::text,
    reviewer_notes text,
    created_at text NOT NULL,
    CONSTRAINT taxonomy_review_status_check CHECK ((review_status = ANY (ARRAY['draft'::text, 'approved'::text, 'rejected'::text, 'revised'::text])))
);


--
-- Name: taxonomy_case_log; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.taxonomy_case_log (
    case_id text NOT NULL,
    processed_at text NOT NULL
);


--
-- Name: triage_results; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.triage_results (
    id integer NOT NULL,
    run_id text NOT NULL,
    policy_id text NOT NULL,
    triage_score real NOT NULL,
    rationale text NOT NULL,
    uncertainty text,
    priority_rank integer NOT NULL,
    created_at text NOT NULL
);


--
-- Name: triage_results_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.triage_results_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: triage_results_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.triage_results_id_seq OWNED BY public.triage_results.id;


--
-- Name: convergence_scores id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.convergence_scores ALTER COLUMN id SET DEFAULT nextval('public.convergence_scores_id_seq'::regclass);


--
-- Name: policy_scores id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.policy_scores ALTER COLUMN id SET DEFAULT nextval('public.policy_scores_id_seq'::regclass);


--
-- Name: stage_log id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.stage_log ALTER COLUMN id SET DEFAULT nextval('public.stage_log_id_seq'::regclass);


--
-- Name: triage_results id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.triage_results ALTER COLUMN id SET DEFAULT nextval('public.triage_results_id_seq'::regclass);


--
-- Name: _svap_schema _svap_schema_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public._svap_schema
    ADD CONSTRAINT _svap_schema_pkey PRIMARY KEY (id);


--
-- Name: assessment_findings assessment_findings_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.assessment_findings
    ADD CONSTRAINT assessment_findings_pkey PRIMARY KEY (assessment_id, finding_id);


--
-- Name: calibration calibration_new_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.calibration
    ADD CONSTRAINT calibration_new_pkey PRIMARY KEY (id);


--
-- Name: cases cases_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.cases
    ADD CONSTRAINT cases_pkey PRIMARY KEY (case_id);


--
-- Name: chunks chunks_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunks
    ADD CONSTRAINT chunks_pkey PRIMARY KEY (chunk_id);


--
-- Name: convergence_scores convergence_scores_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.convergence_scores
    ADD CONSTRAINT convergence_scores_pkey PRIMARY KEY (id);


--
-- Name: detection_patterns detection_patterns_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.detection_patterns
    ADD CONSTRAINT detection_patterns_pkey PRIMARY KEY (pattern_id);


--
-- Name: dimension_registry dimension_registry_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.dimension_registry
    ADD CONSTRAINT dimension_registry_pkey PRIMARY KEY (dimension_id);


--
-- Name: documents documents_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.documents
    ADD CONSTRAINT documents_pkey PRIMARY KEY (doc_id);


--
-- Name: enforcement_sources enforcement_sources_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.enforcement_sources
    ADD CONSTRAINT enforcement_sources_pkey PRIMARY KEY (source_id);


--
-- Name: exploitation_steps exploitation_steps_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.exploitation_steps
    ADD CONSTRAINT exploitation_steps_pkey PRIMARY KEY (step_id);


--
-- Name: exploitation_trees exploitation_trees_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.exploitation_trees
    ADD CONSTRAINT exploitation_trees_pkey PRIMARY KEY (tree_id);


--
-- Name: exploitation_trees exploitation_trees_policy_id_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.exploitation_trees
    ADD CONSTRAINT exploitation_trees_policy_id_key UNIQUE (policy_id);


--
-- Name: pipeline_runs pipeline_runs_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.pipeline_runs
    ADD CONSTRAINT pipeline_runs_pkey PRIMARY KEY (run_id);


--
-- Name: policies policies_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.policies
    ADD CONSTRAINT policies_pkey PRIMARY KEY (policy_id);


--
-- Name: policy_scores policy_scores_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.policy_scores
    ADD CONSTRAINT policy_scores_pkey PRIMARY KEY (id);


--
-- Name: prediction_qualities prediction_qualities_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.prediction_qualities
    ADD CONSTRAINT prediction_qualities_pkey PRIMARY KEY (prediction_id, quality_id);


--
-- Name: predictions predictions_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.predictions
    ADD CONSTRAINT predictions_pkey PRIMARY KEY (prediction_id);


--
-- Name: quality_assessments quality_assessments_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.quality_assessments
    ADD CONSTRAINT quality_assessments_pkey PRIMARY KEY (assessment_id);


--
-- Name: regulatory_sources regulatory_sources_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.regulatory_sources
    ADD CONSTRAINT regulatory_sources_pkey PRIMARY KEY (source_id);


--
-- Name: research_sessions research_sessions_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.research_sessions
    ADD CONSTRAINT research_sessions_pkey PRIMARY KEY (session_id);


--
-- Name: source_candidates source_candidates_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.source_candidates
    ADD CONSTRAINT source_candidates_pkey PRIMARY KEY (candidate_id);


--
-- Name: source_candidates source_candidates_url_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.source_candidates
    ADD CONSTRAINT source_candidates_url_key UNIQUE (url);


--
-- Name: source_feeds source_feeds_listing_url_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.source_feeds
    ADD CONSTRAINT source_feeds_listing_url_key UNIQUE (listing_url);


--
-- Name: source_feeds source_feeds_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.source_feeds
    ADD CONSTRAINT source_feeds_pkey PRIMARY KEY (feed_id);


--
-- Name: stage_log stage_log_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.stage_log
    ADD CONSTRAINT stage_log_pkey PRIMARY KEY (id);


--
-- Name: stage_processing_log stage_processing_log_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.stage_processing_log
    ADD CONSTRAINT stage_processing_log_pkey PRIMARY KEY (stage, entity_id);


--
-- Name: step_qualities step_qualities_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.step_qualities
    ADD CONSTRAINT step_qualities_pkey PRIMARY KEY (step_id, quality_id);


--
-- Name: structural_findings structural_findings_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.structural_findings
    ADD CONSTRAINT structural_findings_pkey PRIMARY KEY (finding_id);


--
-- Name: taxonomy_case_log taxonomy_case_log_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.taxonomy_case_log
    ADD CONSTRAINT taxonomy_case_log_pkey PRIMARY KEY (case_id);


--
-- Name: taxonomy taxonomy_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.taxonomy
    ADD CONSTRAINT taxonomy_pkey PRIMARY KEY (quality_id);


--
-- Name: triage_results triage_results_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.triage_results
    ADD CONSTRAINT triage_results_pkey PRIMARY KEY (id);


--
-- Name: idx_patterns_step; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_patterns_step ON public.detection_patterns USING btree (step_id);


--
-- Name: idx_steps_parent; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_steps_parent ON public.exploitation_steps USING btree (parent_step_id);


--
-- Name: idx_steps_tree; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_steps_tree ON public.exploitation_steps USING btree (tree_id);


--
-- Name: uq_convergence; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX uq_convergence ON public.convergence_scores USING btree (case_id, quality_id);


--
-- Name: uq_enforcement_source_url; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX uq_enforcement_source_url ON public.enforcement_sources USING btree (url) WHERE (url IS NOT NULL);


--
-- Name: uq_policy_score; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX uq_policy_score ON public.policy_scores USING btree (policy_id, quality_id);


--
-- Name: uq_quality_assessment; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX uq_quality_assessment ON public.quality_assessments USING btree (policy_id, quality_id);


--
-- Name: uq_triage; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX uq_triage ON public.triage_results USING btree (policy_id);


--
-- Name: assessment_findings assessment_findings_assessment_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.assessment_findings
    ADD CONSTRAINT assessment_findings_assessment_id_fkey FOREIGN KEY (assessment_id) REFERENCES public.quality_assessments(assessment_id) ON DELETE CASCADE;


--
-- Name: assessment_findings assessment_findings_finding_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.assessment_findings
    ADD CONSTRAINT assessment_findings_finding_id_fkey FOREIGN KEY (finding_id) REFERENCES public.structural_findings(finding_id);


--
-- Name: cases cases_source_doc_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.cases
    ADD CONSTRAINT cases_source_doc_fkey FOREIGN KEY (source_doc_id) REFERENCES public.documents(doc_id);


--
-- Name: chunks chunks_doc_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunks
    ADD CONSTRAINT chunks_doc_id_fkey FOREIGN KEY (doc_id) REFERENCES public.documents(doc_id);


--
-- Name: convergence_scores convergence_scores_case_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.convergence_scores
    ADD CONSTRAINT convergence_scores_case_id_fkey FOREIGN KEY (case_id) REFERENCES public.cases(case_id);


--
-- Name: convergence_scores convergence_scores_quality_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.convergence_scores
    ADD CONSTRAINT convergence_scores_quality_id_fkey FOREIGN KEY (quality_id) REFERENCES public.taxonomy(quality_id);


--
-- Name: detection_patterns detection_patterns_step_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.detection_patterns
    ADD CONSTRAINT detection_patterns_step_fkey FOREIGN KEY (step_id) REFERENCES public.exploitation_steps(step_id) ON DELETE CASCADE;


--
-- Name: enforcement_sources enforcement_sources_doc_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.enforcement_sources
    ADD CONSTRAINT enforcement_sources_doc_fkey FOREIGN KEY (doc_id) REFERENCES public.documents(doc_id);


--
-- Name: exploitation_steps exploitation_steps_parent_step_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.exploitation_steps
    ADD CONSTRAINT exploitation_steps_parent_step_id_fkey FOREIGN KEY (parent_step_id) REFERENCES public.exploitation_steps(step_id) ON DELETE CASCADE;


--
-- Name: exploitation_steps exploitation_steps_tree_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.exploitation_steps
    ADD CONSTRAINT exploitation_steps_tree_id_fkey FOREIGN KEY (tree_id) REFERENCES public.exploitation_trees(tree_id) ON DELETE CASCADE;


--
-- Name: exploitation_trees exploitation_trees_policy_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.exploitation_trees
    ADD CONSTRAINT exploitation_trees_policy_id_fkey FOREIGN KEY (policy_id) REFERENCES public.policies(policy_id);


--
-- Name: policy_scores policy_scores_policy_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.policy_scores
    ADD CONSTRAINT policy_scores_policy_id_fkey FOREIGN KEY (policy_id) REFERENCES public.policies(policy_id);


--
-- Name: policy_scores policy_scores_quality_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.policy_scores
    ADD CONSTRAINT policy_scores_quality_id_fkey FOREIGN KEY (quality_id) REFERENCES public.taxonomy(quality_id);


--
-- Name: prediction_qualities prediction_qualities_prediction_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.prediction_qualities
    ADD CONSTRAINT prediction_qualities_prediction_id_fkey FOREIGN KEY (prediction_id) REFERENCES public.predictions(prediction_id) ON DELETE CASCADE;


--
-- Name: prediction_qualities prediction_qualities_quality_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.prediction_qualities
    ADD CONSTRAINT prediction_qualities_quality_id_fkey FOREIGN KEY (quality_id) REFERENCES public.taxonomy(quality_id);


--
-- Name: predictions predictions_policy_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.predictions
    ADD CONSTRAINT predictions_policy_id_fkey FOREIGN KEY (policy_id) REFERENCES public.policies(policy_id);


--
-- Name: quality_assessments quality_assessments_policy_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.quality_assessments
    ADD CONSTRAINT quality_assessments_policy_id_fkey FOREIGN KEY (policy_id) REFERENCES public.policies(policy_id);


--
-- Name: quality_assessments quality_assessments_quality_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.quality_assessments
    ADD CONSTRAINT quality_assessments_quality_id_fkey FOREIGN KEY (quality_id) REFERENCES public.taxonomy(quality_id);


--
-- Name: research_sessions research_sessions_policy_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.research_sessions
    ADD CONSTRAINT research_sessions_policy_id_fkey FOREIGN KEY (policy_id) REFERENCES public.policies(policy_id);


--
-- Name: source_candidates source_candidates_feed_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.source_candidates
    ADD CONSTRAINT source_candidates_feed_id_fkey FOREIGN KEY (feed_id) REFERENCES public.source_feeds(feed_id);


--
-- Name: stage_log stage_log_run_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.stage_log
    ADD CONSTRAINT stage_log_run_id_fkey FOREIGN KEY (run_id) REFERENCES public.pipeline_runs(run_id);


--
-- Name: step_qualities step_qualities_quality_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.step_qualities
    ADD CONSTRAINT step_qualities_quality_id_fkey FOREIGN KEY (quality_id) REFERENCES public.taxonomy(quality_id);


--
-- Name: step_qualities step_qualities_step_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.step_qualities
    ADD CONSTRAINT step_qualities_step_id_fkey FOREIGN KEY (step_id) REFERENCES public.exploitation_steps(step_id) ON DELETE CASCADE;


--
-- Name: structural_findings structural_findings_dimension_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.structural_findings
    ADD CONSTRAINT structural_findings_dimension_id_fkey FOREIGN KEY (dimension_id) REFERENCES public.dimension_registry(dimension_id);


--
-- Name: structural_findings structural_findings_policy_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.structural_findings
    ADD CONSTRAINT structural_findings_policy_id_fkey FOREIGN KEY (policy_id) REFERENCES public.policies(policy_id);


--
-- Name: taxonomy_case_log taxonomy_case_log_case_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.taxonomy_case_log
    ADD CONSTRAINT taxonomy_case_log_case_id_fkey FOREIGN KEY (case_id) REFERENCES public.cases(case_id);


--
-- Name: triage_results triage_results_policy_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.triage_results
    ADD CONSTRAINT triage_results_policy_id_fkey FOREIGN KEY (policy_id) REFERENCES public.policies(policy_id);


--
-- PostgreSQL database dump complete
--

\unrestrict rzGq6ephmCclt2BQcSrF0jPMGdsMsLC2OUpt5emSTS2eOk8tM55UrNNbMJz7hTL

