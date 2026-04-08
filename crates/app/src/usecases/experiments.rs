use anyhow::Result;
use pump_agent_core::{
    EvaluationRow, ExperimentDetail, ExperimentRow, HypothesisRow, PgEventStore,
};
use serde::{Deserialize, Serialize};

use crate::strategy::StrategyConfig;

use super::inspect::DatabaseRequest;

const SCHEMA_SQL: &str = include_str!("../../../../schema/postgres.sql");

#[derive(Debug, Clone)]
pub struct CreateExperimentRequest {
    pub database: DatabaseRequest,
    pub experiment_id: String,
    pub title: String,
    pub target_wallet: String,
    pub thesis: Option<String>,
    pub notes: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentContext {
    pub experiment_id: String,
    pub hypothesis_id: Option<String>,
    #[serde(default = "default_json_object")]
    pub notes: serde_json::Value,
    #[serde(default = "default_json_array")]
    pub artifact_paths: serde_json::Value,
    #[serde(default)]
    pub failure_tags: Vec<String>,
    pub conclusion: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ListExperimentsRequest {
    pub database: DatabaseRequest,
    pub limit: i64,
}

#[derive(Debug, Clone)]
pub struct CreateHypothesisRequest {
    pub database: DatabaseRequest,
    pub hypothesis_id: String,
    pub experiment_id: String,
    pub family: String,
    pub description: String,
    pub strategy_config: Option<StrategyConfig>,
    pub sample_window: serde_json::Value,
    pub notes: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct CreateEvaluationRequest {
    pub database: DatabaseRequest,
    pub evaluation_id: String,
    pub experiment_id: String,
    pub hypothesis_id: Option<String>,
    pub strategy_run_id: Option<i64>,
    pub task_id: Option<String>,
    pub target_wallet: String,
    pub family: Option<String>,
    pub strategy_name: Option<String>,
    pub source_type: String,
    pub source_ref: String,
    pub score_overall: Option<f64>,
    pub score_breakdown: serde_json::Value,
    pub metrics: serde_json::Value,
    pub failure_tags: Vec<String>,
    pub artifact_paths: serde_json::Value,
    pub notes: serde_json::Value,
    pub conclusion: Option<String>,
}

#[derive(Debug, Clone)]
pub struct InspectExperimentRequest {
    pub database: DatabaseRequest,
    pub experiment_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationSummary {
    pub evaluation_id: String,
    pub experiment_id: String,
    pub hypothesis_id: Option<String>,
    pub strategy_run_id: Option<i64>,
    pub task_id: Option<String>,
    pub target_wallet: String,
    pub family: Option<String>,
    pub strategy_name: Option<String>,
    pub source_type: String,
    pub source_ref: String,
    pub score_overall: Option<f64>,
    pub score_breakdown: serde_json::Value,
    pub metrics: serde_json::Value,
    pub failure_tags: Vec<String>,
    pub artifact_paths: serde_json::Value,
    pub notes: serde_json::Value,
    pub conclusion: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentDetailResult {
    pub experiment: ExperimentRow,
    pub hypotheses: Vec<HypothesisRow>,
    pub evaluations: Vec<EvaluationSummary>,
}

pub async fn create_experiment(request: CreateExperimentRequest) -> Result<ExperimentRow> {
    let store = connect_store(&request.database).await?;
    store
        .create_experiment(
            &request.experiment_id,
            &request.title,
            &request.target_wallet,
            request.thesis.as_deref(),
            request.notes,
        )
        .await
}

pub async fn list_experiments(request: ListExperimentsRequest) -> Result<Vec<ExperimentRow>> {
    let store = connect_store(&request.database).await?;
    store.list_experiments(request.limit).await
}

pub async fn create_hypothesis(request: CreateHypothesisRequest) -> Result<HypothesisRow> {
    let store = connect_store(&request.database).await?;
    let strategy_config = match request.strategy_config {
        Some(config) => serde_json::to_value(config)?,
        None => serde_json::json!({}),
    };

    store
        .create_hypothesis(
            &request.hypothesis_id,
            &request.experiment_id,
            &request.family,
            &request.description,
            strategy_config,
            request.sample_window,
            request.notes,
        )
        .await
}

pub async fn create_evaluation(request: CreateEvaluationRequest) -> Result<EvaluationSummary> {
    let store = connect_store(&request.database).await?;
    let row = store
        .create_evaluation(
            &request.evaluation_id,
            &request.experiment_id,
            request.hypothesis_id.as_deref(),
            request.strategy_run_id,
            request.task_id.as_deref(),
            &request.target_wallet,
            request.family.as_deref(),
            request.strategy_name.as_deref(),
            &request.source_type,
            &request.source_ref,
            request.score_overall,
            request.score_breakdown,
            request.metrics,
            &request.failure_tags,
            request.artifact_paths,
            request.notes,
            request.conclusion.as_deref(),
        )
        .await?;

    Ok(EvaluationSummary::from(row))
}

pub async fn inspect_experiment(
    request: InspectExperimentRequest,
) -> Result<Option<ExperimentDetailResult>> {
    let store = connect_store(&request.database).await?;
    let detail = store.inspect_experiment(&request.experiment_id).await?;
    Ok(detail.map(ExperimentDetailResult::from))
}

async fn connect_store(request: &DatabaseRequest) -> Result<PgEventStore> {
    let store = PgEventStore::connect(&request.database_url, request.max_db_connections).await?;
    if request.apply_schema {
        store.apply_schema(SCHEMA_SQL).await?;
    }
    Ok(store)
}

impl From<EvaluationRow> for EvaluationSummary {
    fn from(value: EvaluationRow) -> Self {
        Self {
            evaluation_id: value.evaluation_id,
            experiment_id: value.experiment_id,
            hypothesis_id: value.hypothesis_id,
            strategy_run_id: value.strategy_run_id,
            task_id: value.task_id,
            target_wallet: value.target_wallet,
            family: value.family,
            strategy_name: value.strategy_name,
            source_type: value.source_type,
            source_ref: value.source_ref,
            score_overall: value.score_overall,
            score_breakdown: value.score_breakdown,
            metrics: value.metrics,
            failure_tags: value.failure_tags,
            artifact_paths: value.artifact_paths,
            notes: value.notes,
            conclusion: value.conclusion,
            created_at: value.created_at,
        }
    }
}

impl From<ExperimentDetail> for ExperimentDetailResult {
    fn from(value: ExperimentDetail) -> Self {
        Self {
            experiment: value.experiment,
            hypotheses: value.hypotheses,
            evaluations: value
                .evaluations
                .into_iter()
                .map(EvaluationSummary::from)
                .collect(),
        }
    }
}

pub fn generate_record_id(prefix: &str) -> String {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    format!("{}-{}", prefix, millis)
}

fn default_json_object() -> serde_json::Value {
    serde_json::json!({})
}

fn default_json_array() -> serde_json::Value {
    serde_json::json!([])
}
