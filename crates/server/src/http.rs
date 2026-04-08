use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    Json, Router,
    extract::{Path, Query, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use pump_agent_app::{
    api::{
        build_clone_report, clone_eval_output, clone_explain_why_output, clone_rank_output,
        compare_runs_output, evaluation_output, experiment_detail_output, experiment_output,
        fit_params_output, hypothesis_output, infer_strategy_output, mint_shard_summary_output,
        suggest_next_experiment_output, sweep_db_output, task_run_output, wallet_dossier_output,
    },
    strategy::{StrategyConfig, SweepConfig},
    usecases::{
        AddressInspectRequest, CloneAnalysisRequest, CloneEvalRequest, CloneRankRequest,
        CompareRunsRequest, CreateEvaluationRequest, CreateExperimentRequest,
        CreateHypothesisRequest, DatabaseRequest, ExperimentContext, FitParamsRequest,
        InferStrategyRequest, IngestTaskRequest, InspectExperimentRequest, ListExperimentsRequest,
        LivePaperTaskRequest, MintShardSummaryRequest, ReplayDbRequest, RunInspectRequest,
        RunsRequest, StatsRequest, SweepBatchInspectRequest, SweepDbRequest, address_inspect,
        analyze_clone_candidates, clone_eval, clone_rank, compare_runs, create_evaluation,
        create_experiment, create_hypothesis, fetch_stats, fit_params, infer_strategy,
        inspect_experiment, inspect_run, inspect_sweep_batch, list_experiments, list_runs,
        replay_db, run_ingest_task, run_live_paper_task, summarize_mint_shards, sweep_db,
    },
};
use pump_agent_core::PgEventStore;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::watch;

const SCHEMA_VERSION: &str = "v1";
const SCHEMA_SQL: &str = include_str!("../../../schema/postgres.sql");

#[derive(Clone)]
pub struct ApiState {
    database_url: String,
    max_db_connections: u32,
    next_request_id: Arc<AtomicU64>,
    task_manager: TaskManager,
}

impl ApiState {
    pub fn new(database_url: String, max_db_connections: u32) -> Self {
        Self {
            database_url,
            max_db_connections,
            next_request_id: Arc::new(AtomicU64::new(1)),
            task_manager: TaskManager::default(),
        }
    }

    fn database(&self, apply_schema: bool) -> DatabaseRequest {
        DatabaseRequest {
            database_url: self.database_url.clone(),
            max_db_connections: self.max_db_connections,
            apply_schema,
        }
    }

    fn request_id(&self) -> String {
        let counter = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default();
        format!("req-{}-{}", millis, counter)
    }

    fn task_id(&self, kind: &str) -> String {
        let counter = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default();
        format!("task-{}-{}-{}", kind, millis, counter)
    }

    fn resolve_task_database_url(&self, database_url: Option<String>) -> String {
        database_url.unwrap_or_else(|| self.database_url.clone())
    }

    async fn task_store(&self) -> anyhow::Result<PgEventStore> {
        let store = PgEventStore::connect(&self.database_url, self.max_db_connections).await?;
        store.apply_schema(SCHEMA_SQL).await?;
        Ok(store)
    }

    fn internal_error(&self, error: anyhow::Error) -> ApiError {
        let message = error.to_string();
        if is_external_dependency_error(&message) {
            ApiError::new(
                StatusCode::BAD_GATEWAY,
                self.request_id(),
                "EXTERNAL_DEPENDENCY_ERROR",
                message,
                None,
                true,
            )
        } else {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                self.request_id(),
                "INTERNAL_ERROR",
                message,
                None,
                false,
            )
        }
    }

    fn validation_error(&self, message: impl Into<String>, details: Option<Value>) -> ApiError {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            self.request_id(),
            "VALIDATION_ERROR",
            message.into(),
            details,
            false,
        )
    }

    fn not_found_error(&self, message: impl Into<String>, details: Option<Value>) -> ApiError {
        ApiError::new(
            StatusCode::NOT_FOUND,
            self.request_id(),
            "NOT_FOUND",
            message.into(),
            details,
            false,
        )
    }
}

pub fn router(state: ApiState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/stats", get(stats))
        .route("/v1/runs", get(runs))
        .route("/v1/runs/compare", post(compare_runs_handler))
        .route("/v1/runs/{id}", get(run_inspect))
        .route("/v1/batches/{batch_id}", get(batch_inspect))
        .route("/v1/address/inspect", post(address_inspect_handler))
        .route("/v1/address/mint-shards", post(address_mint_shards_handler))
        .route("/v1/address/dossier", post(address_dossier_handler))
        .route("/v1/clone/report", post(clone_report_handler))
        .route("/v1/clone/explain-why", post(clone_explain_why_handler))
        .route(
            "/v1/clone/suggest-next-experiment",
            post(clone_suggest_next_experiment_handler),
        )
        .route("/v1/clone/eval", post(clone_eval_handler))
        .route("/v1/clone/infer", post(clone_infer_handler))
        .route("/v1/clone/fit", post(clone_fit_handler))
        .route("/v1/clone/rank", post(clone_rank_handler))
        .route("/v1/replay/db", post(replay_db_handler))
        .route("/v1/sweep/db", post(sweep_db_handler))
        .route(
            "/v1/experiments",
            get(experiments_handler).post(create_experiment_handler),
        )
        .route(
            "/v1/experiments/{experiment_id}",
            get(experiment_detail_handler),
        )
        .route("/v1/hypotheses", post(create_hypothesis_handler))
        .route("/v1/evaluations", post(create_evaluation_handler))
        .route("/v1/tasks/ingest", post(ingest_task_handler))
        .route("/v1/tasks/live-paper", post(live_paper_task_handler))
        .route("/v1/tasks/{task_id}", get(task_status_handler))
        .route("/v1/tasks/{task_id}/cancel", post(task_cancel_handler))
        .with_state(state)
}

async fn healthz(State(state): State<ApiState>) -> impl IntoResponse {
    success_response(&state, HealthzResponse { status: "ok" })
}

async fn stats(State(state): State<ApiState>) -> Result<impl IntoResponse, ApiError> {
    let data = fetch_stats(StatsRequest {
        database: state.database(true),
    })
    .await
    .map_err(|error| state.internal_error(error))?;
    Ok(success_response(&state, data))
}

async fn runs(
    State(state): State<ApiState>,
    Query(query): Query<RunsQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let limit = query.limit.unwrap_or(50).clamp(1, 500);
    let data = list_runs(RunsRequest {
        database: state.database(false),
        limit,
    })
    .await
    .map_err(|error| state.internal_error(error))?;
    Ok(success_response(&state, data))
}

async fn compare_runs_handler(
    State(state): State<ApiState>,
    payload: Result<Json<CompareRunsBody>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let payload = parse_json(payload, &state)?;
    let result = compare_runs(CompareRunsRequest {
        database: state.database(true),
        left_run_id: payload.left_id,
        right_run_id: payload.right_id,
        fill_limit: payload.fill_limit.unwrap_or(20).clamp(0, 500),
    })
    .await
    .map_err(|error| state.internal_error(error))?;
    let Some(result) = result else {
        return Err(state.not_found_error(
            "one or both strategy runs were not found",
            Some(json!({
                "resource": "strategy_run",
                "left_id": payload.left_id,
                "right_id": payload.right_id,
            })),
        ));
    };
    Ok(success_response(&state, compare_runs_output(result)))
}

async fn run_inspect(
    State(state): State<ApiState>,
    Path(id): Path<i64>,
    Query(query): Query<RunInspectQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let data = inspect_run(RunInspectRequest {
        database: state.database(false),
        run_id: id,
        fill_limit: query.fill_limit.unwrap_or(50).clamp(0, 500),
    })
    .await
    .map_err(|error| state.internal_error(error))?;
    Ok(success_response(&state, data))
}

async fn batch_inspect(
    State(state): State<ApiState>,
    Path(batch_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let data = inspect_sweep_batch(SweepBatchInspectRequest {
        database: state.database(false),
        batch_id,
    })
    .await
    .map_err(|error| state.internal_error(error))?;
    Ok(success_response(&state, data))
}

async fn address_inspect_handler(
    State(state): State<ApiState>,
    payload: Result<Json<AddressInspectBody>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let payload = parse_json(payload, &state)?;
    let data = address_inspect(AddressInspectRequest {
        database: state.database(true),
        address: payload.address,
        top_mints_limit: payload.top_mints_limit.unwrap_or(10).clamp(1, 100),
        roundtrip_limit: payload.roundtrip_limit.unwrap_or(10).clamp(1, 100),
    })
    .await
    .map_err(|error| state.internal_error(error))?;
    Ok(success_response(&state, data))
}

async fn address_dossier_handler(
    State(state): State<ApiState>,
    payload: Result<Json<AddressDossierBody>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let payload = parse_json(payload, &state)?;
    let database = state.database(true);
    let analysis = analyze_clone_candidates(CloneAnalysisRequest {
        database: database.clone(),
        address: payload.address.clone(),
    })
    .await
    .map_err(|error| state.internal_error(error))?;
    let inspect = address_inspect(AddressInspectRequest {
        database,
        address: payload.address,
        top_mints_limit: payload.top_mints_limit.unwrap_or(10).clamp(1, 100),
        roundtrip_limit: payload.roundtrip_limit.unwrap_or(10).clamp(1, 100),
    })
    .await
    .map_err(|error| state.internal_error(error))?;
    let experiment = if let Some(experiment_id) = payload.experiment_id.as_deref() {
        let detail = inspect_experiment(InspectExperimentRequest {
            database: state.database(true),
            experiment_id: experiment_id.to_string(),
        })
        .await
        .map_err(|error| state.internal_error(error))?;
        let Some(detail) = detail else {
            return Err(state.not_found_error(
                format!("experiment not found: {}", experiment_id),
                Some(json!({
                    "resource": "experiment",
                    "id": experiment_id,
                })),
            ));
        };
        Some(detail)
    } else {
        None
    };
    let output = wallet_dossier_output(
        inspect,
        &analysis.wallet,
        &analysis.best_family,
        &analysis.runner_up,
        experiment.as_ref(),
        payload.sample_limit.unwrap_or(5).clamp(1, 25),
    );
    Ok(success_response(&state, output))
}

async fn address_mint_shards_handler(
    State(state): State<ApiState>,
    payload: Result<Json<AddressMintShardsBody>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let payload = parse_json(payload, &state)?;
    let data = summarize_mint_shards(MintShardSummaryRequest {
        database: state.database(true),
        address: payload.address,
        limit: payload.limit.unwrap_or(20).clamp(1, 200),
    })
    .await
    .map_err(|error| state.internal_error(error))?;
    Ok(success_response(&state, mint_shard_summary_output(data)))
}

async fn clone_report_handler(
    State(state): State<ApiState>,
    payload: Result<Json<CloneReportBody>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let payload = parse_json(payload, &state)?;
    let analysis = analyze_clone_candidates(CloneAnalysisRequest {
        database: state.database(true),
        address: payload.address,
    })
    .await
    .map_err(|error| state.internal_error(error))?;
    let report = build_clone_report(
        &analysis.wallet,
        &analysis.best_family,
        &analysis.runner_up,
        None,
    );
    Ok(success_response(&state, report))
}

async fn clone_explain_why_handler(
    State(state): State<ApiState>,
    payload: Result<Json<CloneReportBody>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let payload = parse_json(payload, &state)?;
    let analysis = analyze_clone_candidates(CloneAnalysisRequest {
        database: state.database(true),
        address: payload.address,
    })
    .await
    .map_err(|error| state.internal_error(error))?;
    let output =
        clone_explain_why_output(&analysis.wallet, &analysis.best_family, &analysis.runner_up);
    Ok(success_response(&state, output))
}

async fn clone_suggest_next_experiment_handler(
    State(state): State<ApiState>,
    payload: Result<Json<SuggestNextExperimentBody>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let payload = parse_json(payload, &state)?;
    let analysis = analyze_clone_candidates(CloneAnalysisRequest {
        database: state.database(true),
        address: payload.address,
    })
    .await
    .map_err(|error| state.internal_error(error))?;
    let experiment = if let Some(experiment_id) = payload.experiment_id.as_deref() {
        let detail = inspect_experiment(InspectExperimentRequest {
            database: state.database(true),
            experiment_id: experiment_id.to_string(),
        })
        .await
        .map_err(|error| state.internal_error(error))?;
        let Some(detail) = detail else {
            return Err(state.not_found_error(
                format!("experiment not found: {}", experiment_id),
                Some(json!({
                    "resource": "experiment",
                    "id": experiment_id,
                })),
            ));
        };
        Some(detail)
    } else {
        None
    };
    let output = suggest_next_experiment_output(
        &analysis.wallet,
        &analysis.best_family,
        &analysis.runner_up,
        experiment.as_ref(),
    );
    Ok(success_response(&state, output))
}

async fn clone_eval_handler(
    State(state): State<ApiState>,
    payload: Result<Json<CloneEvalBody>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let payload = parse_json(payload, &state)?;
    let result = clone_eval(CloneEvalRequest {
        database: state.database(true),
        address: payload.address.clone(),
        strategy: payload.strategy,
        run_id: payload.run_id,
        experiment: payload.experiment,
    })
    .await
    .map_err(|error| state.internal_error(error))?;
    let Some(data) = result else {
        return Err(state.not_found_error(
            format!(
                "strategy run not found: {}",
                payload.run_id.unwrap_or_default()
            ),
            Some(json!({
                "resource": "strategy_run",
                "id": payload.run_id,
            })),
        ));
    };
    Ok(success_response(&state, clone_eval_output(data)))
}

async fn clone_infer_handler(
    State(state): State<ApiState>,
    payload: Result<Json<InferStrategyBody>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let payload = parse_json(payload, &state)?;
    let data = infer_strategy(InferStrategyRequest {
        database: state.database(true),
        address: payload.address,
        family: payload.family,
    })
    .await
    .map_err(|error| state.internal_error(error))?;
    Ok(success_response(&state, infer_strategy_output(data)))
}

async fn clone_fit_handler(
    State(state): State<ApiState>,
    payload: Result<Json<FitParamsBody>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let payload = parse_json(payload, &state)?;
    let top = payload.top.unwrap_or(10).clamp(1, 100);
    let data = fit_params(FitParamsRequest {
        database: state.database(true),
        address: payload.address,
        family: payload.family,
        base_overrides: payload.strategy,
        sweep: payload.sweep.unwrap_or_default(),
    })
    .await
    .map_err(|error| state.internal_error(error))?;
    Ok(success_response(&state, fit_params_output(data, top)))
}

async fn clone_rank_handler(
    State(state): State<ApiState>,
    payload: Result<Json<CloneRankBody>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let payload = parse_json(payload, &state)?;
    let top = payload.top.unwrap_or(10).clamp(1, 100);
    let data = clone_rank(CloneRankRequest {
        database: state.database(true),
        address: payload.address,
        scan_limit: payload.scan_limit.unwrap_or(50).clamp(1, 500),
    })
    .await
    .map_err(|error| state.internal_error(error))?;
    Ok(success_response(&state, clone_rank_output(data, top)))
}

async fn replay_db_handler(
    State(state): State<ApiState>,
    payload: Result<Json<ReplayDbBody>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let payload = parse_json(payload, &state)?;
    let data = replay_db(ReplayDbRequest {
        database: state.database(false),
        strategy: payload.strategy,
        save_run: payload.save_run.unwrap_or(false),
        experiment: payload.experiment,
    })
    .await
    .map_err(|error| state.internal_error(error))?;
    Ok(success_response(&state, data))
}

async fn sweep_db_handler(
    State(state): State<ApiState>,
    payload: Result<Json<SweepDbBody>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let payload = parse_json(payload, &state)?;
    let top = payload.top.unwrap_or(10).clamp(1, 100);
    let data = sweep_db(SweepDbRequest {
        database: state.database(true),
        strategy: payload.strategy,
        sweep: payload.sweep,
        experiment: payload.experiment,
    })
    .await
    .map_err(|error| state.internal_error(error))?;
    Ok(success_response(&state, sweep_db_output(data, top)))
}

async fn experiments_handler(
    State(state): State<ApiState>,
    Query(query): Query<ExperimentsQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let data = list_experiments(ListExperimentsRequest {
        database: state.database(true),
        limit: query.limit.unwrap_or(50).clamp(1, 500),
    })
    .await
    .map_err(|error| state.internal_error(error))?
    .into_iter()
    .map(experiment_output)
    .collect::<Vec<_>>();

    Ok(success_response(&state, data))
}

async fn create_experiment_handler(
    State(state): State<ApiState>,
    payload: Result<Json<CreateExperimentBody>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let payload = parse_json(payload, &state)?;
    let data = create_experiment(CreateExperimentRequest {
        database: state.database(true),
        experiment_id: payload.experiment_id,
        title: payload.title,
        target_wallet: payload.target_wallet,
        thesis: payload.thesis,
        notes: payload.notes.unwrap_or_else(|| json!({})),
    })
    .await
    .map_err(|error| state.internal_error(error))?;

    Ok(success_response(&state, experiment_output(data)))
}

async fn experiment_detail_handler(
    State(state): State<ApiState>,
    Path(experiment_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let Some(detail) = inspect_experiment(InspectExperimentRequest {
        database: state.database(true),
        experiment_id: experiment_id.clone(),
    })
    .await
    .map_err(|error| state.internal_error(error))?
    else {
        return Err(state.not_found_error(
            format!("experiment not found: {experiment_id}"),
            Some(json!({
                "resource": "experiment",
                "id": experiment_id,
            })),
        ));
    };

    Ok(success_response(&state, experiment_detail_output(detail)))
}

async fn create_hypothesis_handler(
    State(state): State<ApiState>,
    payload: Result<Json<CreateHypothesisBody>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let payload = parse_json(payload, &state)?;
    let data = create_hypothesis(CreateHypothesisRequest {
        database: state.database(true),
        hypothesis_id: payload.hypothesis_id,
        experiment_id: payload.experiment_id,
        family: payload.family,
        description: payload.description,
        strategy_config: payload.strategy_config,
        sample_window: payload.sample_window.unwrap_or_else(|| json!({})),
        notes: payload.notes.unwrap_or_else(|| json!({})),
    })
    .await
    .map_err(|error| state.internal_error(error))?;

    Ok(success_response(&state, hypothesis_output(data)))
}

async fn create_evaluation_handler(
    State(state): State<ApiState>,
    payload: Result<Json<CreateEvaluationBody>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let payload = parse_json(payload, &state)?;
    let data = create_evaluation(CreateEvaluationRequest {
        database: state.database(true),
        evaluation_id: payload.evaluation_id,
        experiment_id: payload.experiment_id,
        hypothesis_id: payload.hypothesis_id,
        strategy_run_id: payload.strategy_run_id,
        task_id: payload.task_id,
        target_wallet: payload.target_wallet,
        family: payload.family,
        strategy_name: payload.strategy_name,
        source_type: payload.source_type,
        source_ref: payload.source_ref,
        score_overall: payload.score_overall,
        score_breakdown: payload.score_breakdown.unwrap_or_else(|| json!({})),
        metrics: payload.metrics.unwrap_or_else(|| json!({})),
        failure_tags: payload.failure_tags.unwrap_or_default(),
        artifact_paths: payload.artifact_paths.unwrap_or_else(|| json!([])),
        notes: payload.notes.unwrap_or_else(|| json!({})),
        conclusion: payload.conclusion,
    })
    .await
    .map_err(|error| state.internal_error(error))?;

    Ok(success_response(&state, evaluation_output(data)))
}

async fn ingest_task_handler(
    State(state): State<ApiState>,
    payload: Result<Json<SubmitIngestTaskBody>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let mut payload = parse_json(payload, &state)?;
    payload.request.stream.database_url =
        Some(state.resolve_task_database_url(payload.request.stream.database_url.clone()));
    let store = state
        .task_store()
        .await
        .map_err(|error| state.internal_error(error))?;

    if let Some(idempotency_key) = payload.idempotency_key.as_deref()
        && let Some(existing) = store
            .find_task_run_by_idempotency_key("ingest", idempotency_key)
            .await
            .map_err(|error| state.internal_error(error))?
    {
        return Ok(success_response(&state, task_run_output(existing)));
    }

    let task_id = state.task_id("ingest");
    let row = store
        .insert_task_run(
            &task_id,
            "ingest",
            payload.idempotency_key.as_deref(),
            serde_json::to_value(&payload.request)
                .map_err(|error| state.internal_error(error.into()))?,
        )
        .await
        .map_err(|error| state.internal_error(error))?;

    spawn_ingest_task(state.clone(), task_id, payload.request);
    Ok(success_response(&state, task_run_output(row)))
}

async fn live_paper_task_handler(
    State(state): State<ApiState>,
    payload: Result<Json<SubmitLivePaperTaskBody>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let mut payload = parse_json(payload, &state)?;
    payload.request.stream.database_url =
        Some(state.resolve_task_database_url(payload.request.stream.database_url.clone()));
    let store = state
        .task_store()
        .await
        .map_err(|error| state.internal_error(error))?;

    if let Some(idempotency_key) = payload.idempotency_key.as_deref()
        && let Some(existing) = store
            .find_task_run_by_idempotency_key("live_paper", idempotency_key)
            .await
            .map_err(|error| state.internal_error(error))?
    {
        return Ok(success_response(&state, task_run_output(existing)));
    }

    let task_id = state.task_id("live-paper");
    let row = store
        .insert_task_run(
            &task_id,
            "live_paper",
            payload.idempotency_key.as_deref(),
            serde_json::to_value(&payload.request)
                .map_err(|error| state.internal_error(error.into()))?,
        )
        .await
        .map_err(|error| state.internal_error(error))?;

    spawn_live_paper_task(state.clone(), task_id, payload.request);
    Ok(success_response(&state, task_run_output(row)))
}

async fn task_status_handler(
    State(state): State<ApiState>,
    Path(task_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let store = state
        .task_store()
        .await
        .map_err(|error| state.internal_error(error))?;
    let Some(row) = store
        .get_task_run(&task_id)
        .await
        .map_err(|error| state.internal_error(error))?
    else {
        return Err(state.not_found_error(
            format!("task not found: {task_id}"),
            Some(json!({
                "resource": "task_run",
                "id": task_id,
            })),
        ));
    };

    Ok(success_response(&state, task_run_output(row)))
}

async fn task_cancel_handler(
    State(state): State<ApiState>,
    Path(task_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let store = state
        .task_store()
        .await
        .map_err(|error| state.internal_error(error))?;
    let Some(row) = store
        .request_task_cancel(&task_id)
        .await
        .map_err(|error| state.internal_error(error))?
    else {
        return Err(state.not_found_error(
            format!("task not found: {task_id}"),
            Some(json!({
                "resource": "task_run",
                "id": task_id,
            })),
        ));
    };
    state.task_manager.cancel(&task_id);
    Ok(success_response(&state, task_run_output(row)))
}

fn parse_json<T>(payload: Result<Json<T>, JsonRejection>, state: &ApiState) -> Result<T, ApiError> {
    payload.map(|Json(value)| value).map_err(|error| {
        state.validation_error(
            "invalid json request body",
            Some(json!({ "rejection": error.body_text() })),
        )
    })
}

fn success_response<T: Serialize>(
    state: &ApiState,
    data: T,
) -> (StatusCode, Json<SuccessEnvelope<T>>) {
    (
        StatusCode::OK,
        Json(SuccessEnvelope {
            schema_version: SCHEMA_VERSION,
            ok: true,
            data,
            meta: ResponseMeta {
                request_id: state.request_id(),
            },
        }),
    )
}

fn is_external_dependency_error(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    message.contains("connect")
        || message.contains("connection")
        || message.contains("pool")
        || message.contains("postgres")
        || message.contains("database")
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    request_id: String,
    code: &'static str,
    message: String,
    details: Option<Value>,
    retryable: bool,
}

impl ApiError {
    fn new(
        status: StatusCode,
        request_id: String,
        code: &'static str,
        message: String,
        details: Option<Value>,
        retryable: bool,
    ) -> Self {
        Self {
            status,
            request_id,
            code,
            message,
            details,
            retryable,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ErrorEnvelope {
            schema_version: SCHEMA_VERSION,
            ok: false,
            error: ErrorBody {
                code: self.code,
                message: self.message,
                retryable: self.retryable,
                details: self.details,
            },
            meta: ResponseMeta {
                request_id: self.request_id,
            },
        };
        (self.status, Json(body)).into_response()
    }
}

#[derive(Debug, Serialize)]
struct SuccessEnvelope<T> {
    schema_version: &'static str,
    ok: bool,
    data: T,
    meta: ResponseMeta,
}

#[derive(Debug, Serialize)]
struct ErrorEnvelope {
    schema_version: &'static str,
    ok: bool,
    error: ErrorBody,
    meta: ResponseMeta,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    code: &'static str,
    message: String,
    retryable: bool,
    details: Option<Value>,
}

#[derive(Debug, Serialize)]
struct ResponseMeta {
    request_id: String,
}

#[derive(Debug, Serialize)]
struct HealthzResponse {
    status: &'static str,
}

#[derive(Debug, Default, Clone)]
struct TaskManager {
    handles: Arc<Mutex<HashMap<String, watch::Sender<bool>>>>,
}

impl TaskManager {
    fn register(&self, task_id: String, cancel_tx: watch::Sender<bool>) {
        let mut handles = self.handles.lock().expect("task manager mutex poisoned");
        handles.insert(task_id, cancel_tx);
    }

    fn cancel(&self, task_id: &str) {
        let handles = self.handles.lock().expect("task manager mutex poisoned");
        if let Some(cancel_tx) = handles.get(task_id) {
            let _ = cancel_tx.send(true);
        }
    }

    fn unregister(&self, task_id: &str) {
        let mut handles = self.handles.lock().expect("task manager mutex poisoned");
        handles.remove(task_id);
    }
}

#[derive(Debug, Deserialize)]
struct RunsQuery {
    limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct ExperimentsQuery {
    limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct RunInspectQuery {
    fill_limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct CompareRunsBody {
    left_id: i64,
    right_id: i64,
    fill_limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct AddressInspectBody {
    address: String,
    top_mints_limit: Option<i64>,
    roundtrip_limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct AddressDossierBody {
    address: String,
    experiment_id: Option<String>,
    top_mints_limit: Option<i64>,
    roundtrip_limit: Option<i64>,
    sample_limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct AddressMintShardsBody {
    address: String,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct CloneReportBody {
    address: String,
}

#[derive(Debug, Deserialize)]
struct SuggestNextExperimentBody {
    address: String,
    experiment_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CloneEvalBody {
    address: String,
    run_id: Option<i64>,
    strategy: Option<StrategyConfig>,
    experiment: Option<ExperimentContext>,
}

#[derive(Debug, Deserialize)]
struct InferStrategyBody {
    address: String,
    family: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FitParamsBody {
    address: String,
    family: String,
    top: Option<usize>,
    strategy: StrategyConfig,
    sweep: Option<SweepConfig>,
}

#[derive(Debug, Deserialize)]
struct CloneRankBody {
    address: String,
    scan_limit: Option<i64>,
    top: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct ReplayDbBody {
    strategy: StrategyConfig,
    save_run: Option<bool>,
    experiment: Option<ExperimentContext>,
}

#[derive(Debug, Deserialize)]
struct SweepDbBody {
    strategy: StrategyConfig,
    sweep: SweepConfig,
    top: Option<usize>,
    experiment: Option<ExperimentContext>,
}

#[derive(Debug, Deserialize)]
struct SubmitIngestTaskBody {
    idempotency_key: Option<String>,
    #[serde(flatten)]
    request: IngestTaskRequest,
}

#[derive(Debug, Deserialize)]
struct SubmitLivePaperTaskBody {
    idempotency_key: Option<String>,
    #[serde(flatten)]
    request: LivePaperTaskRequest,
}

#[derive(Debug, Deserialize)]
struct CreateExperimentBody {
    experiment_id: String,
    title: String,
    target_wallet: String,
    thesis: Option<String>,
    notes: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct CreateHypothesisBody {
    hypothesis_id: String,
    experiment_id: String,
    family: String,
    description: String,
    strategy_config: Option<StrategyConfig>,
    sample_window: Option<Value>,
    notes: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct CreateEvaluationBody {
    evaluation_id: String,
    experiment_id: String,
    hypothesis_id: Option<String>,
    strategy_run_id: Option<i64>,
    task_id: Option<String>,
    target_wallet: String,
    family: Option<String>,
    strategy_name: Option<String>,
    source_type: String,
    source_ref: String,
    score_overall: Option<f64>,
    score_breakdown: Option<Value>,
    metrics: Option<Value>,
    failure_tags: Option<Vec<String>>,
    artifact_paths: Option<Value>,
    notes: Option<Value>,
    conclusion: Option<String>,
}

fn spawn_ingest_task(state: ApiState, task_id: String, request: IngestTaskRequest) {
    let (cancel_tx, cancel_rx) = watch::channel(false);
    state.task_manager.register(task_id.clone(), cancel_tx);

    tokio::spawn(async move {
        let result = async {
            let store = state.task_store().await?;
            let _ = store.mark_task_running(&task_id).await?;
            let execution = run_ingest_task(&task_id, request, cancel_rx).await?;
            let _ = store
                .complete_task_run(
                    &task_id,
                    &execution.status,
                    serde_json::to_value(&execution.result)?,
                )
                .await?;
            anyhow::Ok(())
        }
        .await;

        if let Err(error) = result
            && let Ok(store) = state.task_store().await
        {
            let _ = store
                .fail_task_run(
                    &task_id,
                    json!({
                        "code": "TASK_FAILED",
                        "message": error.to_string(),
                    }),
                )
                .await;
        }

        state.task_manager.unregister(&task_id);
    });
}

fn spawn_live_paper_task(state: ApiState, task_id: String, request: LivePaperTaskRequest) {
    let (cancel_tx, cancel_rx) = watch::channel(false);
    state.task_manager.register(task_id.clone(), cancel_tx);

    tokio::spawn(async move {
        let result = async {
            let store = state.task_store().await?;
            let _ = store.mark_task_running(&task_id).await?;
            let execution = run_live_paper_task(&task_id, request, cancel_rx).await?;
            let _ = store
                .complete_task_run(
                    &task_id,
                    &execution.status,
                    serde_json::to_value(&execution.result)?,
                )
                .await?;
            anyhow::Ok(())
        }
        .await;

        if let Err(error) = result
            && let Ok(store) = state.task_store().await
        {
            let _ = store
                .fail_task_run(
                    &task_id,
                    json!({
                        "code": "TASK_FAILED",
                        "message": error.to_string(),
                    }),
                )
                .await;
        }

        state.task_manager.unregister(&task_id);
    });
}
