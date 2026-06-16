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

fn env_or<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}
fn env_or_str(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

impl Default for AppConfig {
    fn default() -> Self {
        let _ = dotenvy::dotenv().ok();
        Self {
            server: ServerConfig {
                host: env_or_str("SIEGE_SERVER_HOST", "0.0.0.0".into()),
                port: env_or("SIEGE_SERVER_PORT", 8080u16),
            },
            clickhouse: ClickHouseConfig {
                url: env_or_str("SIEGE_CLICKHOUSE_URL", "http://localhost:8123".into()),
                user: env_or_str("SIEGE_CLICKHOUSE_USER", "default".into()),
                password: env_or_str("SIEGE_CLICKHOUSE_PASSWORD", "".into()),
                database: env_or_str("SIEGE_CLICKHOUSE_DATABASE", "siege_tower".into()),
            },
            mqtt: MqttConfig {
                broker: env_or_str("SIEGE_MQTT_BROKER", "localhost".into()),
                port: env_or("SIEGE_MQTT_PORT", 1883u16),
                client_id: env_or_str("SIEGE_MQTT_CLIENT_ID", "siege-tower-server".into()),
                username: std::env::var("SIEGE_MQTT_USERNAME").ok(),
                password: std::env::var("SIEGE_MQTT_PASSWORD").ok(),
                alert_topic: env_or_str("SIEGE_MQTT_ALERT_TOPIC", "siege/tower/alert".into()),
                sensor_topic: env_or_str("SIEGE_MQTT_SENSOR_TOPIC", "siege/tower/sensor".into()),
            },
            alert: AlertConfig {
                tilt_warning_threshold: env_or("SIEGE_ALERT_TILT_WARNING", 3.0f64),
                tilt_danger_threshold: env_or("SIEGE_ALERT_TILT_DANGER", 5.0f64),
                stress_warning_ratio: env_or("SIEGE_ALERT_STRESS_WARNING", 0.75f64),
                stress_danger_ratio: env_or("SIEGE_ALERT_STRESS_DANGER", 0.90f64),
                wind_warning_ratio: env_or("SIEGE_ALERT_WIND_WARNING", 0.80f64),
                wind_danger_ratio: env_or("SIEGE_ALERT_WIND_DANGER", 0.95f64),
                ground_warning_ratio: env_or("SIEGE_ALERT_GROUND_WARNING", 0.80f64),
                ground_danger_ratio: env_or("SIEGE_ALERT_GROUND_DANGER", 0.95f64),
            },
            simulation: SimulationConfig {
                gravity: env_or("SIEGE_SIM_GRAVITY", 9.81f64),
                air_density: env_or("SIEGE_SIM_AIR_DENSITY", 1.225f64),
                wind_drag_coefficient: env_or("SIEGE_SIM_WIND_DRAG", 1.3f64),
                safety_factor_min: env_or("SIEGE_SIM_SAFETY_FACTOR_MIN", 1.5f64),
                second_order_enabled: env_or("SIEGE_SIM_SECOND_ORDER", true),
            },
        }
    }
}

impl AppConfig {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self::default())
    }

    pub fn init() -> &'static Self {
        CONFIG.get_or_init(Self::default)
    }

    pub fn get() -> &'static Self {
        CONFIG.get().expect("Config not initialized")
    }
}
