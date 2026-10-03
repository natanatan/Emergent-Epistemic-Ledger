use axum::body::Body;
use axum::http::{Request, StatusCode};
use eel_api::{datadir, router};
use eel_core::event::*;
use eel_core::ontology::*;
use eel_core::*;
use http_body_util::BodyExt;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower::ServiceExt;

async fn call(
    app: &axum::Router,
    method: &str,
    uri: &str,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(
            body.map(|b| Body::from(b.to_string()))
                .unwrap_or_else(Body::empty),
        )
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
}

#[tokio::test]
async fn signed_events_in_projected_state_out() {
    let dir = tempfile::tempdir().unwrap();
    let node = datadir::open_node(dir.path()).await.unwrap();
    let app = router(Arc::new(Mutex::new(node)));

    let key = eel_crypto::KeyPair::from_seed([42; 32]);
    let (_, reg) = key
        .identity_register_event(Timestamp(2_000_000_000_000))
        .unwrap();
    let (s, body) = call(
        &app,
        "POST",
        "/identities",
        Some(serde_json::to_value(&reg).unwrap()),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{body}");
    assert_eq!(body["block_height"], 1);

    let claim = Claim {
        claim_id: "EE-REL-0001".parse().unwrap(),
        version: 1,
        title: "Distinction".into(),
        statement: "A difference that makes a difference.".into(),
        claim_type: vec![ClaimType::Definition],
        ontology_level: "pre-relational".into(),
        status: ClaimStatus::Proposed,
        dependencies: vec![],
        ontological_defensibility: None,
        domain: "ontology".into(),
    };
    let (_, ev) = key
        .author_event(
            Timestamp(2_000_000_000_001),
            BranchId::main(),
            vec![],
            EventPayload::ClaimCreate(ClaimCreate {
                claim,
                relations: vec![],
            }),
        )
        .unwrap();

    // Wrong endpoint for the event type.
    let (s, _) = call(
        &app,
        "POST",
        "/objections",
        Some(serde_json::to_value(&ev).unwrap()),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);

    let (s, body) = call(
        &app,
        "POST",
        "/claims",
        Some(serde_json::to_value(&ev).unwrap()),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{body}");

    // Replaying the same event is rejected.
    let (s, _) = call(
        &app,
        "POST",
        "/events",
        Some(serde_json::to_value(&ev).unwrap()),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);

    let (s, body) = call(&app, "GET", "/claims/EE-REL-0001", None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body["status"], "PROPOSED");
    let (s, body) = call(&app, "GET", "/claims/EE-REL-0001/history", None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body["versions"].as_array().unwrap().len(), 1);
    let (s, _) = call(&app, "GET", "/claims/NOPE", None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, body) = call(&app, "GET", "/ontology/graph", None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body["claims"].as_array().unwrap().len(), 1);
    let (s, body) = call(&app, "GET", "/ontology/state/1", None).await;
    assert_eq!(s, StatusCode::OK);
    assert!(body["state"]["branches"]["main"]["state"]["claims"]
        .as_object()
        .unwrap()
        .is_empty());
    let (s, body) = call(&app, "GET", "/status", None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body["height"], 2);
}
