use std::sync::Arc;
use std::time::Duration;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tower_http::compression::CompressionLayer;

use siege_tower_backend::alert::AlertManager;
use siege_tower_backend::config::AppConfig;
use siege_tower_backend::database::ClickHouseClient;
use siege_tower_backend::fem::FEMAnalysis;
use siege_tower_backend::ground::GroundAnalyzer;
use siege_tower_backend::handlers::AppState;
use siege_tower_backend::mqtt_client::MqttService;
use siege_tower_backend::routes::create_routes;
use siege_tower_backend::stability::StabilityAnalyzer;

use tracing::{info, error, warn};
use tokio::sync::broadcast;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "siege_tower_backend=info,tower_http=info".into()),
        )
        .with_target(true)
        .with_thread_ids(true)
        .init();

    info!("=== 古代临冲吕公车结构仿真与稳定性分析系统 ===");
    info!("正在加载配置...");

    let config = match AppConfig::load() {
        Ok(cfg) => {
            info!("配置加载成功");
            cfg
        }
        Err(e) => {
            error!("配置加载失败: {:?}", e);
            return Err(anyhow::anyhow!("配置加载失败: {}", e));
        }
    };

    info!("正在初始化 ClickHouse 客户端...");
    let mut db = ClickHouseClient::new(config.clickhouse.clone());
    match db.connect().await {
        Ok(_) => info!("ClickHouse 连接初始化完成"),
        Err(e) => warn!("ClickHouse 连接失败（将以本地模式运行）: {:?}", e),
    }

    info!("正在初始化 MQTT 客户端...");
    let mut mqtt = MqttService::new(config.mqtt.clone());
    match mqtt.connect().await {
        Ok(_) => info!("MQTT 连接初始化完成"),
        Err(e) => warn!("MQTT 连接失败（将以本地模式运行）: {:?}", e),
    }

    let (alert_tx, _) = broadcast::channel::<siege_tower_backend::models::AlertEvent>(128);
    let (analysis_tx, _) = broadcast::channel::<siege_tower_backend::models::StructureAnalysis>(64);
    let (sensor_tx, _) = broadcast::channel::<Vec<siege_tower_backend::models::SensorData>>(256);

    let state = Arc::new(AppState {
        config: config.clone(),
        db,
        mqtt: parking_lot::Mutex::new(mqtt),
        alert_manager: parking_lot::Mutex::new(AlertManager::new()),
        fem: parking_lot::Mutex::new(FEMAnalysis::new()),
        stability: StabilityAnalyzer::new(),
        ground: GroundAnalyzer::new(),
        alert_tx,
        analysis_tx,
        sensor_tx,
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any)
        .max_age(Duration::from_secs(86400));

    let app = create_routes(state)
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .layer(cors);

    let addr = format!("{}:{}", config.server.host, config.server.port);
    info!("HTTP 服务正在启动: {}", addr);
    info!("API 端点:");
    info!("  GET  /api/health                        - 健康检查");
    info!("  GET  /api/towers                        - 获取所有攻城塔");
    info!("  GET  /api/towers/:id                    - 获取塔详情");
    info!("  POST /api/sensor                        - 接收传感器数据");
    info!("  GET  /api/towers/:id/sensor             - 查询传感器数据");
    info!("  GET  /api/towers/:id/analysis           - 获取最新分析");
    info!("  GET  /api/towers/:id/analysis/custom    - 自定义参数分析");
    info!("  GET  /api/towers/:id/ground             - 地面适应性分析");
    info!("  GET  /api/towers/:id/alerts             - 查询告警事件");
    info!("  GET  /api/towers/:id/fem/mesh           - 有限元网格");
    info!("  GET  /api/stream/sensor                 - SSE 传感器数据流");
    info!("  GET  /api/stream/analysis               - SSE 分析结果流");
    info!("  GET  /api/stream/alerts                 - SSE 告警流");
    info!("");
    info!("MQTT Broker: {}:{}", config.mqtt.broker, config.mqtt.port);
    info!("  告警主题: {}", config.mqtt.alert_topic);
    info!("  传感器主题: {}", config.mqtt.sensor_topic);
    info!("");
    info!("服务已就绪，开始监听连接...");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app.into_make_service()).await?;

    Ok(())
}
