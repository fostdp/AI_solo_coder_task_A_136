use crate::handlers::*;
use crate::handlers::SharedState;
use axum::{
    routing::{get, post},
    Router,
};

pub fn create_routes(state: SharedState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/towers", get(get_all_towers))
        .route("/api/towers/:tower_id", get(get_tower))
        .route("/api/sensor", post(receive_sensor_data))
        .route("/api/towers/:tower_id/sensor", get(query_sensor_data))
        .route("/api/towers/:tower_id/analysis", get(get_latest_analysis))
        .route("/api/towers/:tower_id/analysis/custom", get(run_custom_analysis))
        .route("/api/towers/:tower_id/ground", get(run_ground_analysis))
        .route("/api/towers/:tower_id/alerts", get(query_alert_events))
        .route("/api/towers/:tower_id/fem/mesh", get(get_fem_mesh))
        .route("/api/stream/sensor", get(sse_sensor_stream))
        .route("/api/stream/analysis", get(sse_analysis_stream))
        .route("/api/stream/alerts", get(sse_alert_stream))
        .with_state(state)
}
