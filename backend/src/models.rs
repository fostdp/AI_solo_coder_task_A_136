use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SoilType {
    Sand,
    Clay,
    Silt,
    Rock,
    Loam,
}

impl SoilType {
    pub fn bearing_capacity_kpa(&self) -> f64 {
        match self {
            SoilType::Sand => 180.0,
            SoilType::Clay => 120.0,
            SoilType::Silt => 90.0,
            SoilType::Rock => 800.0,
            SoilType::Loam => 200.0,
        }
    }

    pub fn friction_coefficient(&self) -> f64 {
        match self {
            SoilType::Sand => 0.45,
            SoilType::Clay => 0.25,
            SoilType::Silt => 0.30,
            SoilType::Rock => 0.65,
            SoilType::Loam => 0.40,
        }
    }

    pub fn compressibility_index(&self) -> f64 {
        match self {
            SoilType::Sand => 0.02,
            SoilType::Clay => 0.35,
            SoilType::Silt => 0.20,
            SoilType::Rock => 0.001,
            SoilType::Loam => 0.08,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            SoilType::Sand => "sand",
            SoilType::Clay => "clay",
            SoilType::Silt => "silt",
            SoilType::Rock => "rock",
            SoilType::Loam => "loam",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TowerMetadata {
    pub tower_id: u32,
    pub tower_name: String,
    pub build_date: String,
    pub material: String,
    pub total_height: f64,
    pub total_layers: u8,
    pub base_width: f64,
    pub base_depth: f64,
    pub total_weight: f64,
    pub design_load: f64,
    pub design_wind_speed: f64,
    pub material_strength: f64,
    pub elastic_modulus: f64,
    pub poisson_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorData {
    #[serde(default = "chrono_timestamp")]
    pub timestamp: DateTime<Utc>,
    pub tower_id: u32,
    pub tower_name: String,
    pub layer_id: u8,
    pub layer_name: String,
    pub stress_x: f64,
    pub stress_y: f64,
    pub stress_z: f64,
    pub stress_von_mises: f64,
    pub tilt_x: f64,
    pub tilt_y: f64,
    pub tilt_total: f64,
    pub wind_load_x: f64,
    pub wind_load_y: f64,
    pub wind_speed: f64,
    pub ground_pressure: f64,
    pub ground_settlement: f64,
    pub soil_type: String,
    pub temperature: f64,
    pub humidity: f64,
    pub vibration_freq: f64,
    pub vibration_amp: f64,
    #[serde(default)]
    pub is_alert: u8,
    #[serde(default)]
    pub alert_level: u8,
}

fn chrono_timestamp() -> DateTime<Utc> {
    Utc::now()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FEMNode {
    pub node_id: u32,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FEMElement {
    pub element_id: u32,
    pub node_ids: [u32; 4],
    pub layer_id: u8,
    pub elastic_modulus: f64,
    pub poisson_ratio: f64,
    pub density: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FEMNodeResult {
    pub timestamp: DateTime<Utc>,
    pub tower_id: u32,
    pub layer_id: u8,
    pub node_id: u32,
    pub node_x: f64,
    pub node_y: f64,
    pub node_z: f64,
    pub displacement_x: f64,
    pub displacement_y: f64,
    pub displacement_z: f64,
    pub displacement_total: f64,
    pub stress_xx: f64,
    pub stress_yy: f64,
    pub stress_zz: f64,
    pub stress_xy: f64,
    pub stress_yz: f64,
    pub stress_zx: f64,
    pub von_mises: f64,
    pub plastic_strain: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureAnalysis {
    pub timestamp: DateTime<Utc>,
    pub tower_id: u32,
    pub tower_name: String,
    pub safety_factor: f64,
    pub critical_stress: f64,
    pub max_stress: f64,
    pub max_stress_layer: u8,
    pub max_tilt: f64,
    pub max_tilt_layer: u8,
    pub wind_resistance_limit: f64,
    pub current_wind_factor: f64,
    pub ground_capacity_ratio: f64,
    pub is_stable: u8,
    pub stability_margin: f64,
    pub second_order_effect: f64,
    pub natural_frequency: f64,
    pub damping_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertType {
    TiltExceed,
    StressCritical,
    WindOverload,
    GroundFailure,
    VibrationExceed,
    StructureInstability,
}

impl AlertType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AlertType::TiltExceed => "tilt_exceed",
            AlertType::StressCritical => "stress_critical",
            AlertType::WindOverload => "wind_overload",
            AlertType::GroundFailure => "ground_failure",
            AlertType::VibrationExceed => "vibration_exceed",
            AlertType::StructureInstability => "structure_instability",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            AlertType::TiltExceed => "倾斜角度超限",
            AlertType::StressCritical => "应力接近临界值",
            AlertType::WindOverload => "风荷载超载",
            AlertType::GroundFailure => "地面承载失效",
            AlertType::VibrationExceed => "振动超限",
            AlertType::StructureInstability => "结构失稳预警",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertEvent {
    pub timestamp: DateTime<Utc>,
    pub event_id: Uuid,
    pub tower_id: u32,
    pub tower_name: String,
    pub alert_type: String,
    pub alert_level: u8,
    pub layer_id: u8,
    pub metric_name: String,
    pub metric_value: f64,
    pub threshold: f64,
    pub description: String,
    #[serde(default)]
    pub is_acknowledged: u8,
    pub acknowledged_at: Option<DateTime<Utc>>,
    pub acknowledged_by: Option<String>,
}

impl AlertEvent {
    pub fn new(
        tower_id: u32,
        tower_name: String,
        alert_type: AlertType,
        alert_level: u8,
        layer_id: u8,
        metric_name: String,
        metric_value: f64,
        threshold: f64,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            event_id: Uuid::new_v4(),
            tower_id,
            tower_name,
            alert_type: alert_type.as_str().to_string(),
            alert_level,
            layer_id,
            metric_name,
            metric_value,
            threshold,
            description: alert_type.description().to_string(),
            is_acknowledged: 0,
            acknowledged_at: None,
            acknowledged_by: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundAnalysis {
    pub timestamp: DateTime<Utc>,
    pub tower_id: u32,
    pub soil_type: String,
    pub bearing_capacity: f64,
    pub applied_pressure: f64,
    pub safety_factor: f64,
    pub settlement: f64,
    pub differential_settlement: f64,
    pub passability_score: f64,
    pub can_pass: u8,
    pub risk_level: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchSensorData {
    pub tower_id: u32,
    pub tower_name: String,
    pub timestamp: DateTime<Utc>,
    pub layers: Vec<SensorLayerData>,
    pub environment: EnvironmentData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorLayerData {
    pub layer_id: u8,
    pub layer_name: String,
    pub stress_x: f64,
    pub stress_y: f64,
    pub stress_z: f64,
    pub stress_von_mises: f64,
    pub tilt_x: f64,
    pub tilt_y: f64,
    pub tilt_total: f64,
    pub wind_load_x: f64,
    pub wind_load_y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentData {
    pub wind_speed: f64,
    pub ground_pressure: f64,
    pub ground_settlement: f64,
    pub soil_type: String,
    pub temperature: f64,
    pub humidity: f64,
    pub vibration_freq: f64,
    pub vibration_amp: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub code: u32,
    pub message: String,
    pub data: Option<T>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            code: 200,
            message: "success".to_string(),
            data: Some(data),
        }
    }

    pub fn error(code: u32, message: String) -> Self {
        Self {
            code,
            message,
            data: None,
        }
    }
}
