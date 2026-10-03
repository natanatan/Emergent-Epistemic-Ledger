//! REST API over a [`PersistentNode`].
//!
//! Write endpoints accept signed [`EpistemicEvent`]s only: private keys never
//! leave the client (decisions D-028). Typed endpoints additionally check that
//! the event has a type that belongs there.

pub mod datadir;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use eel_core::*;
use eel_storage::PersistentNode;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;

pub type SharedNode = Arc<Mutex<PersistentNode>>;

pub const OPENAPI_YAML: &str = include_str!("../../../docs/openapi.yaml");

pub struct ApiError(StatusCode, String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}

fn bad(msg: impl ToString) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, msg.to_string())
}

fn not_found(what: impl std::fmt::Display) -> ApiError {
    ApiError(StatusCode::NOT_FOUND, format!("{what} not found"))
}

type ApiResult = Result<Json<Value>, ApiError>;

#[derive(Serialize)]
struct Accepted {
    event_id: EventId,
    block_height: u64,
    block_hash: BlockHash,
    state_root: StateRoot,
}

#[derive(Deserialize, Default)]
pub struct BranchQuery {
    branch: Option<String>,
}

impl BranchQuery {
    fn branch(&self) -> Result<BranchId, ApiError> {
        self.branch
            .as_deref()
            .unwrap_or("main")
            .parse()
            .map_err(bad)
    }
}

async fn submit(node: &SharedNode, event: EpistemicEvent, allowed: &[EventType]) -> ApiResult {
    if !allowed.is_empty() && !allowed.contains(&event.event_type) {
        return Err(bad(format!(
            "event type {:?} is not accepted at this endpoint (expected one of {:?})",
            event.event_type, allowed
        )));
    }
    let mut n = node.lock().await;
    let id = n.submit(event).await.map_err(bad)?;
    let head = n.ledger.head();
    Ok(Json(
        serde_json::to_value(Accepted {
            event_id: id,
            block_height: head.block.height,
            block_hash: head.block_hash,
            state_root: head.block.state_root,
        })
        .unwrap(),
    ))
}

macro_rules! post_typed {
    ($name:ident, [$($t:ident),*]) => {
        async fn $name(State(node): State<SharedNode>, Json(e): Json<EpistemicEvent>) -> ApiResult {
            submit(&node, e, &[$(EventType::$t),*]).await
        }
    };
}

post_typed!(post_identity, [IdentityRegister]);
post_typed!(post_event, []);
post_typed!(post_claim, [ClaimCreate, ClaimRevise, ClaimSupersede]);
post_typed!(post_evidence, [EvidenceAdd, EvidenceRetract]);
post_typed!(post_objection, [ObjectionAdd, ObjectionReply]);
post_typed!(
    post_validation,
    [ValidationSubmit, ReplicationRegister, ReplicationResult]
);
post_typed!(post_branch, [BranchCreate, BranchMergePropose]);
post_typed!(post_proposal, [CanonicalizationPropose]);
post_typed!(post_artifact, [ArtifactRegister]);
post_typed!(post_asset, [XchangeAssetRegister]);
post_typed!(post_request, [ResearchRequestCreate]);
post_typed!(post_job, [SimulationDefine]);
post_typed!(post_sim_result, [SimulationResult]);

/// Checks that an event's payload refers to the id in the path.
async fn submit_for(
    node: &SharedNode,
    e: EpistemicEvent,
    ty: EventType,
    path_id: &str,
    payload_id: Option<String>,
) -> ApiResult {
    if payload_id.as_deref() != Some(path_id) {
        return Err(bad("the event does not refer to the id in the path"));
    }
    submit(node, e, &[ty]).await
}

async fn accept(
    State(node): State<SharedNode>,
    Path(id): Path<String>,
    Json(e): Json<EpistemicEvent>,
) -> ApiResult {
    let pid = match &e.payload {
        EventPayload::CanonicalizationAccept(d) => Some(d.proposal_id.to_string()),
        _ => None,
    };
    submit_for(&node, e, EventType::CanonicalizationAccept, &id, pid).await
}

async fn reject(
    State(node): State<SharedNode>,
    Path(id): Path<String>,
    Json(e): Json<EpistemicEvent>,
) -> ApiResult {
    let pid = match &e.payload {
        EventPayload::CanonicalizationReject(d) => Some(d.proposal_id.to_string()),
        _ => None,
    };
    submit_for(&node, e, EventType::CanonicalizationReject, &id, pid).await
}

async fn support(
    State(node): State<SharedNode>,
    Path(id): Path<String>,
    Json(e): Json<EpistemicEvent>,
) -> ApiResult {
    let rid = match &e.payload {
        EventPayload::ResearchRequestSupport(c) => Some(c.request_id.to_string()),
        _ => None,
    };
    submit_for(&node, e, EventType::ResearchRequestSupport, &id, rid).await
}

async fn research_result(
    State(node): State<SharedNode>,
    Path(id): Path<String>,
    Json(e): Json<EpistemicEvent>,
) -> ApiResult {
    let rid = match &e.payload {
        EventPayload::ResearchResultSubmit(r) => Some(r.request_id.to_string()),
        _ => None,
    };
    submit_for(&node, e, EventType::ResearchResultSubmit, &id, rid).await
}

async fn verify_result(
    State(node): State<SharedNode>,
    Path(id): Path<String>,
    Json(e): Json<EpistemicEvent>,
) -> ApiResult {
    let rid = match &e.payload {
        EventPayload::SimulationVerify(v) => Some(v.result_id.to_string()),
        _ => None,
    };
    submit_for(&node, e, EventType::SimulationVerify, &id, rid).await
}

fn to_json<T: Serialize>(v: &T) -> ApiResult {
    Ok(Json(serde_json::to_value(v).map_err(|e| {
        ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?))
}

async fn status(State(node): State<SharedNode>) -> ApiResult {
    let n = node.lock().await;
    let head = n.ledger.head();
    let st = n.ledger.state();
    Ok(Json(json!({
        "network": st.network.name,
        "protocol_version": st.network.protocol_version,
        "principles": st.network.principles,
        "height": head.block.height,
        "block_hash": head.block_hash,
        "state_root": head.block.state_root,
        "events": st.events_applied,
        "branches": st.branches.keys().collect::<Vec<_>>(),
    })))
}

async fn get_event(State(node): State<SharedNode>, Path(id): Path<String>) -> ApiResult {
    let id: EventId = id.parse().map_err(bad)?;
    let n = node.lock().await;
    let e = n
        .ledger
        .event(&id)
        .ok_or_else(|| not_found(format!("event {id}")))?;
    Ok(Json(
        json!({ "event_id": id, "block_height": n.ledger.event_height(&id), "event": e }),
    ))
}

async fn get_claim(
    State(node): State<SharedNode>,
    Path(id): Path<String>,
    Query(q): Query<BranchQuery>,
) -> ApiResult {
    let (b, id): (BranchId, ClaimId) = (q.branch()?, id.parse().map_err(bad)?);
    let n = node.lock().await;
    let rec = n
        .ledger
        .state()
        .branch(&b)
        .and_then(|br| br.state.claims.get(&id))
        .ok_or_else(|| not_found(format!("claim {id} on {b}")))?;
    to_json(&rec.current)
}

async fn get_claim_history(
    State(node): State<SharedNode>,
    Path(id): Path<String>,
    Query(q): Query<BranchQuery>,
) -> ApiResult {
    let (b, id): (BranchId, ClaimId) = (q.branch()?, id.parse().map_err(bad)?);
    let n = node.lock().await;
    let rec = n
        .ledger
        .state()
        .branch(&b)
        .and_then(|br| br.state.claims.get(&id))
        .ok_or_else(|| not_found(format!("claim {id} on {b}")))?;
    to_json(rec)
}

async fn get_claim_deps(
    State(node): State<SharedNode>,
    Path(id): Path<String>,
    Query(q): Query<BranchQuery>,
) -> ApiResult {
    let (b, id): (BranchId, ClaimId) = (q.branch()?, id.parse().map_err(bad)?);
    let n = node.lock().await;
    let st = n.ledger.state();
    if !st
        .branch(&b)
        .is_some_and(|br| br.state.claims.contains_key(&id))
    {
        return Err(not_found(format!("claim {id} on {b}")));
    }
    let direct = &st.branches[&b].state.claims[&id].current.dependencies;
    Ok(Json(
        json!({ "claim": id, "direct": direct, "transitive": st.dependency_closure(&b, &id) }),
    ))
}

async fn get_branch(State(node): State<SharedNode>, Path(id): Path<String>) -> ApiResult {
    let id: BranchId = id.parse().map_err(bad)?;
    let n = node.lock().await;
    let b = n
        .ledger
        .state()
        .branch(&id)
        .ok_or_else(|| not_found(format!("branch {id}")))?;
    Ok(Json(json!({
        "branch": b.branch,
        "head": b.head,
        "event_count": b.event_count,
        "claims": b.state.claims.len(),
        "edges": b.state.edges.len(),
    })))
}

async fn list_assets(State(node): State<SharedNode>) -> ApiResult {
    let n = node.lock().await;
    let assets: Vec<_> = n
        .ledger
        .state()
        .xchange_assets
        .values()
        .map(|(a, _)| a)
        .collect();
    to_json(&assets)
}

async fn get_request(State(node): State<SharedNode>, Path(id): Path<String>) -> ApiResult {
    let id: ResearchRequestId = id.parse().map_err(bad)?;
    let n = node.lock().await;
    let st = n.ledger.state();
    let r = st
        .research_requests
        .get(&id)
        .ok_or_else(|| not_found(format!("request {id}")))?;
    let results: Vec<_> = r
        .results
        .iter()
        .filter_map(|e| st.research_results.get(e).map(|x| (e, x)))
        .collect();
    Ok(Json(json!({
        "request": r,
        "pools": eel_xchange::pool_totals(&r.commitments),
        "results": results.iter().map(|(e, x)| json!({ "event_id": e, "result": x })).collect::<Vec<_>>(),
    })))
}

async fn get_job(State(node): State<SharedNode>, Path(id): Path<String>) -> ApiResult {
    let id: SimulationJobId = id.parse().map_err(bad)?;
    let n = node.lock().await;
    let st = n.ledger.state();
    let j = st
        .simulation_jobs
        .get(&id)
        .ok_or_else(|| not_found(format!("job {id}")))?;
    let results: Vec<_> = st
        .simulation_results
        .values()
        .filter(|r| r.result.job_id == id)
        .collect();
    Ok(Json(json!({ "job": j, "results": results })))
}

async fn graph(State(node): State<SharedNode>, Query(q): Query<BranchQuery>) -> ApiResult {
    let b = q.branch()?;
    let n = node.lock().await;
    let br = n
        .ledger
        .state()
        .branch(&b)
        .ok_or_else(|| not_found(format!("branch {b}")))?;
    let nodes: Vec<_> = br
        .state
        .claims
        .values()
        .map(|c| {
            json!({
                "claim_id": c.current.claim_id,
                "version": c.current.version,
                "status": c.current.status,
                "title": c.current.title,
            })
        })
        .collect();
    Ok(Json(
        json!({ "branch": b, "claims": nodes, "edges": br.state.edges }),
    ))
}

async fn canonical(State(node): State<SharedNode>, Query(q): Query<BranchQuery>) -> ApiResult {
    let b = q.branch()?;
    let n = node.lock().await;
    to_json(&n.ledger.state().canonical_claims(&b))
}

async fn state_at(State(node): State<SharedNode>, Path(height): Path<u64>) -> ApiResult {
    let n = node.lock().await;
    let s = n
        .ledger
        .state_at(height)
        .map_err(|e| ApiError(StatusCode::NOT_FOUND, e.to_string()))?;
    Ok(Json(
        json!({ "height": height, "state_root": s.state_root(), "state": s }),
    ))
}

async fn openapi() -> impl IntoResponse {
    ([("content-type", "application/yaml")], OPENAPI_YAML)
}

pub fn router(node: SharedNode) -> Router {
    Router::new()
        .route("/", get(status))
        .route("/status", get(status))
        .route("/openapi.yaml", get(openapi))
        .route("/identities", post(post_identity))
        .route("/events", post(post_event))
        .route("/events/{id}", get(get_event))
        .route("/claims", post(post_claim))
        .route("/claims/{id}", get(get_claim))
        .route("/claims/{id}/history", get(get_claim_history))
        .route("/claims/{id}/dependencies", get(get_claim_deps))
        .route("/evidence", post(post_evidence))
        .route("/objections", post(post_objection))
        .route("/validations", post(post_validation))
        .route("/branches", post(post_branch))
        .route("/branches/{id}", get(get_branch))
        .route("/canonicalization/proposals", post(post_proposal))
        .route("/canonicalization/{id}/accept", post(accept))
        .route("/canonicalization/{id}/reject", post(reject))
        .route("/artifacts", post(post_artifact))
        .route("/xchange/assets", post(post_asset).get(list_assets))
        .route("/research/requests", post(post_request))
        .route("/research/requests/{id}", get(get_request))
        .route("/research/requests/{id}/support", post(support))
        .route("/research/requests/{id}/results", post(research_result))
        .route("/simulations/jobs", post(post_job))
        .route("/simulations/jobs/{id}", get(get_job))
        .route("/simulations/results", post(post_sim_result))
        .route("/simulations/results/{id}/verify", post(verify_result))
        .route("/ontology/graph", get(graph))
        .route("/ontology/canonical", get(canonical))
        .route("/ontology/state/{height}", get(state_at))
        .with_state(node)
}

/// Opens the node in a data directory (initializing the development network
/// if needed) and serves the API until interrupted.
pub async fn serve(
    data_dir: &std::path::Path,
    listen: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let node = datadir::open_node(data_dir).await?;
    let head = node.ledger.head().clone();
    let listener = tokio::net::TcpListener::bind(listen).await?;
    tracing::info!(
        "EEL node listening on http://{} (height {}, state root {})",
        listener.local_addr()?,
        head.block.height,
        head.block.state_root
    );
    println!("EEL node listening on http://{}", listener.local_addr()?);
    let app = router(Arc::new(Mutex::new(node)));
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
