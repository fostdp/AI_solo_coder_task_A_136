use crate::alert::AlertManager;
use crate::config::AppConfig;
use crate::database::{ClickHouseClient, get_default_tower};
use crate::fem::FEMAnalysis;
use crate::ground::GroundAnalyzer;
use crate::models::*;
use crate::mqtt_client::MqttService;
use crate::stability::StabilityAnalyzer;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
    response::IntoResponse,
    Json,
};
use parking_lot::Mutex;
use serde::Deserialize;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_stream::StreamExt;
use tracing::{info, warn};

pub struct AppState {
    pub config: AppConfig,
    pub db: ClickHouseClient,
    pub mqtt: Mutex<MqttService>,
    pub alert_manager: Mutex<AlertManager>,
    pub fem: Mutex<FEMAnalysis>,
    pub stability: StabilityAnalyzer,
    pub ground: GroundAnalyzer,
    pub alert_tx: broadcast::Sender<AlertEvent>,
    pub analysis_tx: broadcast::Sender<StructureAnalysis>,
    pub sensor_tx: broadcast::Sender<Vec<SensorData>>,
}

pub type SharedState = Arc<AppState>;

#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    pub limit: Option<u64>,
    pub offset: Option<u64>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub layer_id: Option<u8>,
}

#[derive(Debug, Deserialize)]
pub struct AnalysisQuery {
    pub wind_speed: Option<f64>,
    pub tilt_deg: Option<f64>,
    pub soil_type: Option<String>,
    pub moisture_pct: Option<f64>,
}

pub async fn health() -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({
        "status": "ok",
        "service": "siege-tower-backend",
        "version": "1.0.0"
    })))
}

pub async fn get_all_towers(
    State(state): State<SharedState>,
) -> impl IntoResponse {
    match state.db.query_all_towers().await {
        Ok(towers) => (StatusCode::OK, Json(ApiResponse::success(towers))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<Vec<TowerMetadata>>::error(500, e.to_string())))
    }
}

pub async fn get_tower(
    State(state): State<SharedState>,
    Path(tower_id): Path<u32>,
) -> impl IntoResponse {
    let tower = get_default_tower(tower_id);
    (StatusCode::OK, Json(ApiResponse::success(tower)))
}

pub async fn receive_sensor_data(
    State(state): State<SharedState>,
    Json(batch): Json<BatchSensorData>,
) -> impl IntoResponse {
    info!("接收传感器数据: tower={} layers={}", batch.tower_id, batch.layers.len());

    let mut sensor_data_list = Vec::new();
    let tower = get_default_tower(batch.tower_id);

    for layer in &batch.layers {
        let sd = SensorData {
            timestamp: batch.timestamp,
            tower_id: batch.tower_id,
            tower_name: batch.tower_name.clone(),
            layer_id: layer.layer_id,
            layer_name: layer.layer_name.clone(),
            stress_x: layer.stress_x,
            stress_y: layer.stress_y,
            stress_z: layer.stress_z,
            stress_von_mises: layer.stress_von_mises,
            tilt_x: layer.tilt_x,
            tilt_y: layer.tilt_y,
            tilt_total: layer.tilt_total,
            wind_load_x: layer.wind_load_x,
            wind_load_y: layer.wind_load_y,
            wind_speed: batch.environment.wind_speed,
            ground_pressure: batch.environment.ground_pressure,
            ground_settlement: batch.environment.ground_settlement,
            soil_type: batch.environment.soil_type.clone(),
            temperature: batch.environment.temperature,
            humidity: batch.environment.humidity,
            vibration_freq: batch.environment.vibration_freq,
            vibration_amp: batch.environment.vibration_amp,
            is_alert: 0,
            alert_level: 0,
        };
        sensor_data_list.push(sd);
    }

    let _ = state.db.insert_sensor_data(&sensor_data_list).await;
    let _ = state.sensor_tx.send(sensor_data_list.clone());

    let mqtt = state.mqtt.lock();
    if mqtt.is_connected() {
        let val = serde_json::to_value(&sensor_data_list).unwrap_or_default();
        let _ = mqtt.publish_sensor_data(batch.tower_id, &val).await;
    }
    drop(mqtt);

    let mut fem = state.fem.lock();
    fem.build_tower_mesh(&tower);
    fem.assemble_matrices();
    let sim = &state.config.simulation;
    fem.apply_loads(&tower, batch.environment.wind_speed, 0.0, sim.gravity, sim.air_density, sim.wind_drag_coefficient);
    fem.apply_boundary_conditions(&tower);
    fem.solve();
    if sim.second_order_enabled {
        fem.apply_second_order_effects(&tower);
    }
    let fem_results = fem.get_node_results(batch.tower_id, batch.timestamp, tower.material_strength);
    drop(fem);

    let _ = state.db.insert_fem_results(&fem_results).await;

    let analysis = state.stability.check_stability(&tower, &sensor_data_list, &state.config);
    let _ = state.db.insert_structure_analysis(&analysis).await;
    let _ = state.analysis_tx.send(analysis.clone());

    let mqtt = state.mqtt.lock();
    if mqtt.is_connected() {
        let val = serde_json::to_value(&analysis).unwrap_or_default();
        let _ = mqtt.publish_analysis_result(batch.tower_id, &val).await;
    }
    drop(mqtt);

    let mut alert_manager = state.alert_manager.lock();
    let mut all_alerts = Vec::new();

    let sensor_alerts = alert_manager.check_sensor_alerts(&tower, &sensor_data_list, &state.config.alert);
    all_alerts.extend(sensor_alerts);

    let struct_alerts = alert_manager.check_structure_alerts(&analysis, state.config.simulation.safety_factor_min);
    all_alerts.extend(struct_alerts);

    for alert in &all_alerts {
        let _ = state.alert_tx.send(alert.clone());
    }

    if !all_alerts.is_empty() {
        let _ = state.db.insert_alert_events(&all_alerts).await;

        let mqtt = state.mqtt.lock();
        if mqtt.is_connected() {
            let _ = mqtt.broadcast_alerts(&all_alerts).await;
        }
        drop(mqtt);

        warn!("触发 {} 条告警事件", all_alerts.len());
    }

    (StatusCode::OK, Json(ApiResponse::success(serde_json::json!({
        "received": sensor_data_list.len(),
        "analysis": analysis,
        "alerts_count": all_alerts.len(),
        "alerts": all_alerts
    }))))
}

pub async fn query_sensor_data(
    State(state): State<SharedState>,
    Path(tower_id): Path<u32>,
    Query(params): Query<PaginationQuery>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(100).min(1000);

    match state.db.query_recent_sensor_data(tower_id, limit).await {
        Ok(data) => (StatusCode::OK, Json(ApiResponse::success(data))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<Vec<SensorData>>::error(500, e.to_string())))
    }
}

pub async fn get_latest_analysis(
    State(state): State<SharedState>,
    Path(tower_id): Path<u32>,
) -> impl IntoResponse {
    match state.db.query_latest_analysis(tower_id).await {
        Ok(Some(analysis)) => (StatusCode::OK, Json(ApiResponse::success(analysis))),
        Ok(None) => {
            let tower = get_default_tower(tower_id);
            let dummy_data = generate_dummy_sensor_data(&tower, 15.0);
            let analysis = state.stability.check_stability(&tower, &dummy_data, &state.config);
            (StatusCode::OK, Json(ApiResponse::success(analysis)))
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<StructureAnalysis>::error(500, e.to_string())))
    }
}

pub async fn run_custom_analysis(
    State(state): State<SharedState>,
    Path(tower_id): Path<u32>,
    Query(params): Query<AnalysisQuery>,
) -> impl IntoResponse {
    let tower = get_default_tower(tower_id);
    let wind_speed = params.wind_speed.unwrap_or(20.0);
    let tilt_deg = params.tilt_deg.unwrap_or(1.0);
    let soil = params.soil_type.as_deref().unwrap_or("loam");

    let soil_type = match soil {
        "sand" => SoilType::Sand,
        "clay" => SoilType::Clay,
        "silt" => SoilType::Silt,
        "rock" => SoilType::Rock,
        _ => SoilType::Loam,
    };

    let sensor_data = generate_dummy_sensor_data_with_params(&tower, wind_speed, tilt_deg);
    let analysis = state.stability.check_stability(&tower, &sensor_data, &state.config);
    let moisture = params.moisture_pct;
    let ground = state.ground.analyze(&tower, soil_type, wind_speed, tilt_deg, None, moisture);
    let all_grounds = state.ground.analyze_all_soils(&tower, wind_speed, tilt_deg, moisture);

    let mut fem = state.fem.lock();
    fem.build_tower_mesh(&tower);
    fem.assemble_matrices();
    let sim = &state.config.simulation;
    fem.apply_loads(&tower, wind_speed, 0.0, sim.gravity, sim.air_density, sim.wind_drag_coefficient);
    fem.apply_boundary_conditions(&tower);
    fem.solve();
    let fem_results = fem.get_node_results(tower_id, chrono::Utc::now(), tower.material_strength);
    let layer_stresses = fem.get_layer_stresses(&tower, wind_speed);
    drop(fem);

    (StatusCode::OK, Json(ApiResponse::success(serde_json::json!({
        "structure": analysis,
        "ground_current": ground,
        "ground_all_soils": all_grounds,
        "fem_sample": &fem_results.iter().take(20).cloned().collect::<Vec<_>>(),
        "fem_total_nodes": fem_results.len(),
        "layer_stresses": layer_stresses
    }))))
}

pub async fn run_ground_analysis(
    State(state): State<SharedState>,
    Path(tower_id): Path<u32>,
    Query(params): Query<AnalysisQuery>,
) -> impl IntoResponse {
    let tower = get_default_tower(tower_id);
    let wind_speed = params.wind_speed.unwrap_or(15.0);
    let tilt_deg = params.tilt_deg.unwrap_or(0.5);
    let moisture = params.moisture_pct;

    let all_grounds = state.ground.analyze_all_soils(&tower, wind_speed, tilt_deg, moisture);
    let _ = state.db.insert_ground_analysis(&all_grounds).await;

    (StatusCode::OK, Json(ApiResponse::success(all_grounds)))
}

pub async fn query_alert_events(
    State(state): State<SharedState>,
    Path(tower_id): Path<u32>,
    Query(params): Query<PaginationQuery>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(50).min(500);
    match state.db.query_alert_events(tower_id, limit).await {
        Ok(events) => (StatusCode::OK, Json(ApiResponse::success(events))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<Vec<AlertEvent>>::error(500, e.to_string())))
    }
}

pub async fn get_fem_mesh(
    State(state): State<SharedState>,
    Path(tower_id): Path<u32>,
) -> impl IntoResponse {
    let tower = get_default_tower(tower_id);
    let mut fem = state.fem.lock();
    fem.build_tower_mesh(&tower);

    let nodes: Vec<FEMNode> = fem.nodes.clone();
    let elements: Vec<FEMElement> = fem.elements.clone();
    drop(fem);

    (StatusCode::OK, Json(ApiResponse::success(serde_json::json!({
        "tower": tower,
        "nodes_count": nodes.len(),
        "elements_count": elements.len(),
        "nodes": nodes,
        "elements": elements
    }))))
}

pub async fn sse_sensor_stream(
    State(state): State<SharedState>,
) -> Sse<impl futures_core::Stream<Item = std::result::Result<Event, Infallible>>> {
    let mut rx = state.sensor_tx.subscribe();

    let stream = async_stream::stream! {
        loop {
            match rx.recv().await {
                Ok(data) => {
                    let json = serde_json::to_string(&data).unwrap_or_default();
                    yield Ok(Event::default().event("sensor").data(json));
                }
                Err(_) => break,
            }
        }
    };

    Sse::new(stream).keep_alive(KeepAlive::default())
}

pub async fn sse_analysis_stream(
    State(state): State<SharedState>,
) -> Sse<impl futures_core::Stream<Item = std::result::Result<Event, Infallible>>> {
    let mut rx = state.analysis_tx.subscribe();

    let stream = async_stream::stream! {
        loop {
            match rx.recv().await {
                Ok(data) => {
                    let json = serde_json::to_string(&data).unwrap_or_default();
                    yield Ok(Event::default().event("analysis").data(json));
                }
                Err(_) => break,
            }
        }
    };

    Sse::new(stream).keep_alive(KeepAlive::default())
}

pub async fn sse_alert_stream(
    State(state): State<SharedState>,
) -> Sse<impl futures_core::Stream<Item = std::result::Result<Event, Infallible>>> {
    let mut rx = state.alert_tx.subscribe();

    let stream = async_stream::stream! {
        loop {
            match rx.recv().await {
                Ok(data) => {
                    let json = serde_json::to_string(&data).unwrap_or_default();
                    yield Ok(Event::default().event("alert").data(json));
                }
                Err(_) => break,
            }
        }
    };

    Sse::new(stream).keep_alive(KeepAlive::default())
}

fn generate_dummy_sensor_data(tower: &TowerMetadata, wind_speed: f64) -> Vec<SensorData> {
    generate_dummy_sensor_data_with_params(tower, wind_speed, 1.0)
}

fn generate_dummy_sensor_data_with_params(
    tower: &TowerMetadata,
    wind_speed: f64,
    base_tilt: f64,
) -> Vec<SensorData> {
    let mut data = Vec::new();
    let now = chrono::Utc::now();

    for layer in 1..=tower.total_layers {
        let h = layer as f64 / tower.total_layers as f64;
        let q = 0.5 * 1.225 * 1.3 * wind_speed * wind_speed;

        let base_stress = 2.0 + h * 20.0;
        let wind_stress = q / 1000.0 * (1.0 + h * 0.5) * 12.0;

        let sx = base_stress + wind_stress;
        let sy = base_stress * 0.7 + wind_stress * 0.5;
        let sz = (tower.total_weight * 9.81 / (tower.base_width * tower.base_depth)) * (1.0 + h * 0.2);

        let j2 = 0.5 * ((sx - sy).powi(2) + (sy - sz).powi(2) + (sz - sx).powi(2));
        let vm = (3.0 * j2).sqrt();

        let tilt_x = base_tilt * (0.5 + h) * 1.2;
        let tilt_y = base_tilt * (0.3 + h * 0.6);
        let tilt_total = (tilt_x.powi(2) + tilt_y.powi(2)).sqrt();

        let sd = SensorData {
            timestamp: now,
            tower_id: tower.tower_id,
            tower_name: tower.tower_name.clone(),
            layer_id: layer,
            layer_name: format!("第{}层", layer),
            stress_x: sx,
            stress_y: sy,
            stress_z: sz,
            stress_von_mises: vm,
            tilt_x,
            tilt_y,
            tilt_total,
            wind_load_x: q * (1.0 + h * 0.4),
            wind_load_y: q * 0.4,
            wind_speed,
            ground_pressure: (tower.total_weight * 9.81 / (tower.base_width * tower.base_depth)) * 1.1,
            ground_settlement: 2.5 + h * 5.0,
            soil_type: "loam".to_string(),
            temperature: 22.5,
            humidity: 65.0,
            vibration_freq: 2.5 + h * 1.5,
            vibration_amp: 0.3 + h * 1.2,
            is_alert: 0,
            alert_level: 0,
        };
        data.push(sd);
    }
    data
}
