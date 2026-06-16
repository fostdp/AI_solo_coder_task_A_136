use crate::config::ClickHouseConfig;
use crate::models::{AlertEvent, FEMNodeResult, GroundAnalysis, SensorData, StructureAnalysis, TowerMetadata};
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct ClickHouseClient {
    config: ClickHouseConfig,
    client: Option<Arc<Mutex<clickhouse::Client>>>,
}

impl ClickHouseClient {
    pub fn new(config: ClickHouseConfig) -> Self {
        Self {
            config,
            client: None,
        }
    }

    pub async fn connect(&mut self) -> Result<()> {
        let url = format!("{}?database={}", self.config.url, self.config.database);
        let mut client_builder = clickhouse::Client::default()
            .with_url(url)
            .with_user(self.config.user.clone())
            .with_password(self.config.password.clone());

        let client = Arc::new(Mutex::new(client_builder));
        self.client = Some(client);
        Ok(())
    }

    pub async fn insert_sensor_data(&self, data: &[SensorData]) -> Result<()> {
        if let Some(ref client) = self.client {
            let client = client.lock().await;
            let mut insert = client.insert("sensor_data")?;
            for d in data {
                insert.write(d).await?;
            }
            insert.end().await?;
        }
        Ok(())
    }

    pub async fn insert_structure_analysis(&self, analysis: &StructureAnalysis) -> Result<()> {
        if let Some(ref client) = self.client {
            let client = client.lock().await;
            let mut insert = client.insert("structure_analysis")?;
            insert.write(analysis).await?;
            insert.end().await?;
        }
        Ok(())
    }

    pub async fn insert_alert_events(&self, events: &[AlertEvent]) -> Result<()> {
        if let Some(ref client) = self.client {
            let client = client.lock().await;
            let mut insert = client.insert("alert_events")?;
            for e in events {
                insert.write(e).await?;
            }
            insert.end().await?;
        }
        Ok(())
    }

    pub async fn insert_ground_analysis(&self, analysis: &[GroundAnalysis]) -> Result<()> {
        if let Some(ref client) = self.client {
            let client = client.lock().await;
            let mut insert = client.insert("ground_analysis")?;
            for a in analysis {
                insert.write(a).await?;
            }
            insert.end().await?;
        }
        Ok(())
    }

    pub async fn insert_fem_results(&self, results: &[FEMNodeResult]) -> Result<()> {
        if let Some(ref client) = self.client {
            let client = client.lock().await;
            let mut insert = client.insert("fem_node_results")?;
            for r in results {
                insert.write(r).await?;
            }
            insert.end().await?;
        }
        Ok(())
    }

    pub async fn get_tower_metadata(&self, tower_id: u32) -> Result<Option<TowerMetadata>> {
        Ok(Some(get_default_tower(tower_id)))
    }

    pub async fn query_recent_sensor_data(
        &self,
        tower_id: u32,
        limit: u64,
    ) -> Result<Vec<SensorData>> {
        let _ = (tower_id, limit);
        Ok(Vec::new())
    }

    pub async fn query_sensor_by_time(
        &self,
        tower_id: u32,
        layer_id: Option<u8>,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<SensorData>> {
        let _ = (tower_id, layer_id, start, end);
        Ok(Vec::new())
    }

    pub async fn query_latest_analysis(&self, tower_id: u32) -> Result<Option<StructureAnalysis>> {
        let _ = tower_id;
        Ok(None)
    }

    pub async fn query_alert_events(
        &self,
        tower_id: u32,
        limit: u64,
    ) -> Result<Vec<AlertEvent>> {
        let _ = (tower_id, limit);
        Ok(Vec::new())
    }

    pub async fn query_all_towers(&self) -> Result<Vec<TowerMetadata>> {
        Ok(vec![get_default_tower(1), get_default_tower(2)])
    }
}

pub fn get_default_tower(tower_id: u32) -> TowerMetadata {
    match tower_id {
        2 => TowerMetadata {
            tower_id: 2,
            tower_name: "临冲吕公车-二号".to_string(),
            build_date: "1452-07-22".to_string(),
            material: "柏木+楠木".to_string(),
            total_height: 21.0,
            total_layers: 6,
            base_width: 6.8,
            base_depth: 5.2,
            total_weight: 36.8,
            design_load: 1020.0,
            design_wind_speed: 40.0,
            material_strength: 52.0,
            elastic_modulus: 13500.0,
            poisson_ratio: 0.36,
        },
        _ => TowerMetadata {
            tower_id: 1,
            tower_name: "临冲吕公车-一号".to_string(),
            build_date: "1450-03-15".to_string(),
            material: "松木+铁木".to_string(),
            total_height: 18.5,
            total_layers: 5,
            base_width: 6.2,
            base_depth: 4.8,
            total_weight: 28.5,
            design_load: 850.0,
            design_wind_speed: 35.0,
            material_strength: 45.0,
            elastic_modulus: 12000.0,
            poisson_ratio: 0.38,
        },
    }
}

impl Default for ClickHouseClient {
    fn default() -> Self {
        Self {
            config: ClickHouseConfig {
                url: "http://localhost:8123".to_string(),
                user: "default".to_string(),
                password: "".to_string(),
                database: "siege_tower".to_string(),
            },
            client: None,
        }
    }
}
