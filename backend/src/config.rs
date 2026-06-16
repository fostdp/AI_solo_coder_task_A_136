use serde::Deserialize;
use std::sync::OnceLock;

static CONFIG: OnceLock<AppConfig> = OnceLock::new();

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub clickhouse: ClickHouseConfig,
    pub mqtt: MqttConfig,
    pub alert: AlertConfig,
    pub simulation: SimulationConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClickHouseConfig {
    pub url: String,
    pub user: String,
    pub password: String,
    pub database: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MqttConfig {
    pub broker: String,
    pub port: u16,
    pub client_id: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub alert_topic: String,
    pub sensor_topic: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AlertConfig {
    pub tilt_warning_threshold: f64,
    pub tilt_danger_threshold: f64,
    pub stress_warning_ratio: f64,
    pub stress_danger_ratio: f64,
    pub wind_warning_ratio: f64,
    pub wind_danger_ratio: f64,
    pub ground_warning_ratio: f64,
    pub ground_danger_ratio: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SimulationConfig {
    pub gravity: f64,
    pub air_density: f64,
    pub wind_drag_coefficient: f64,
    pub safety_factor_min: f64,
    pub second_order_enabled: bool,
}

impl AppConfig {
    pub fn load() -> Result<Self, config::ConfigError> {
        let settings = config::Config::builder()
            .add_source(config::File::with_name("config").required(false))
            .add_source(config::Environment::with_prefix("SIEGE").separator("_"))
            .set_default("server.host", "0.0.0.0")?
            .set_default("server.port", 8080)?
            .set_default("clickhouse.url", "http://localhost:8123")?
            .set_default("clickhouse.user", "default")?
            .set_default("clickhouse.password", "")?
            .set_default("clickhouse.database", "siege_tower")?
            .set_default("mqtt.broker", "localhost")?
            .set_default("mqtt.port", 1883)?
            .set_default("mqtt.client_id", "siege-tower-server")?
            .set_default("mqtt.alert_topic", "siege/tower/alert")?
            .set_default("mqtt.sensor_topic", "siege/tower/sensor")?
            .set_default("alert.tilt_warning_threshold", 3.0)?
            .set_default("alert.tilt_danger_threshold", 5.0)?
            .set_default("alert.stress_warning_ratio", 0.75)?
            .set_default("alert.stress_danger_ratio", 0.90)?
            .set_default("alert.wind_warning_ratio", 0.80)?
            .set_default("alert.wind_danger_ratio", 0.95)?
            .set_default("alert.ground_warning_ratio", 0.80)?
            .set_default("alert.ground_danger_ratio", 0.95)?
            .set_default("simulation.gravity", 9.81)?
            .set_default("simulation.air_density", 1.225)?
            .set_default("simulation.wind_drag_coefficient", 1.3)?
            .set_default("simulation.safety_factor_min", 1.5)?
            .set_default("simulation.second_order_enabled", true)?
            .build()?;

        settings.try_deserialize()
    }

    pub fn init() -> &'static Self {
        CONFIG.get_or_init(|| Self::load().unwrap_or_default())
    }

    pub fn get() -> &'static Self {
        CONFIG.get().expect("Config not initialized")
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self::load().unwrap()
    }
}
