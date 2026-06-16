use crate::models::{GroundAnalysis, SoilType, TowerMetadata};
use chrono::Utc;

pub struct GroundAnalyzer;

impl GroundAnalyzer {
    pub fn new() -> Self {
        GroundAnalyzer
    }

    pub fn calculate_applied_pressure(&self, tower: &TowerMetadata, tilt_deg: f64) -> f64 {
        let base_area = tower.base_width * tower.base_depth;
        let weight_kn = tower.total_weight * 9.81;
        let uniform_pressure = weight_kn / base_area;

        let tilt_rad = tilt_deg.to_radians();
        let eccentricity = (tower.total_height / 2.0) * tilt_rad.sin();
        let section_modulus = tower.base_width * tower.base_depth.powi(2) / 6.0;
        let bending_pressure = weight_kn * eccentricity / section_modulus;

        uniform_pressure + bending_pressure.abs()
    }

    pub fn calculate_settlement(
        &self,
        soil: &SoilType,
        applied_pressure_kpa: f64,
        bearing_capacity_kpa: f64,
        layer_thickness_m: f64,
    ) -> f64 {
        let stress_ratio = applied_pressure_kpa / bearing_capacity_kpa;
        let cc = soil.compressibility_index();

        if stress_ratio > 0.95 {
            return 500.0;
        }

        let eo = 0.8;
        let delta_e = cc * (applied_pressure_kpa / 100.0 + 1.0).log10();
        let settlement = (delta_e / (1.0 + eo)) * layer_thickness_m * 1000.0;
        settlement
    }

    pub fn calculate_safety_factor(
        &self,
        soil: &SoilType,
        applied_pressure_kpa: f64,
        bearing_capacity_kpa: f64,
        wind_speed: f64,
    ) -> f64 {
        let pressure_sf = bearing_capacity_kpa / applied_pressure_kpa.max(0.1);
        let friction = soil.friction_coefficient();
        let sliding_resistance = friction * applied_pressure_kpa;

        let air_density = 1.225;
        let cd = 1.3;
        let q = 0.5 * air_density * cd * wind_speed * wind_speed / 1000.0;
        let sliding_force = q * 5.0;

        let sliding_sf = sliding_resistance / sliding_force.max(0.1);

        pressure_sf.min(sliding_sf).min(10.0)
    }

    pub fn calculate_passability(
        &self,
        soil: &SoilType,
        sf: f64,
        settlement_mm: f64,
        diff_settlement_mm: f64,
        max_allowed_settlement: f64,
    ) -> (f64, u8, u8) {
        let sf_score = if sf >= 3.0 {
            100.0
        } else if sf >= 2.0 {
            70.0 + (sf - 2.0) * 30.0
        } else if sf >= 1.5 {
            40.0 + (sf - 1.5) * 60.0
        } else if sf >= 1.0 {
            20.0 + (sf - 1.0) * 40.0
        } else {
            sf * 20.0
        };

        let set_score = if settlement_mm <= max_allowed_settlement {
            100.0
        } else if settlement_mm <= max_allowed_settlement * 2.0 {
            70.0 - (settlement_mm - max_allowed_settlement) / max_allowed_settlement * 70.0
        } else {
            0.0
        };

        let diff_set_score = if diff_settlement_mm <= 20.0 {
            100.0
        } else if diff_settlement_mm <= 50.0 {
            60.0 - (diff_settlement_mm - 20.0) / 30.0 * 60.0
        } else {
            0.0
        };

        let total_score = sf_score * 0.45 + set_score * 0.3 + diff_set_score * 0.25;
        let total_score = total_score.max(0.0).min(100.0);

        let (can_pass, risk_level) = if total_score >= 75.0 {
            (1u8, 1u8)
        } else if total_score >= 50.0 {
            (1u8, 2u8)
        } else if total_score >= 30.0 {
            (0u8, 2u8)
        } else {
            (0u8, 3u8)
        };

        (total_score, can_pass, risk_level)
    }

    pub fn analyze(
        &self,
        tower: &TowerMetadata,
        soil: SoilType,
        wind_speed: f64,
        tilt_deg: f64,
        additional_ground_settlement: Option<f64>,
    ) -> GroundAnalysis {
        let bearing_capacity = soil.bearing_capacity_kpa();
        let applied_pressure = self.calculate_applied_pressure(tower, tilt_deg);
        let sf = self.calculate_safety_factor(&soil, applied_pressure, bearing_capacity, wind_speed);

        let soil_layer_thickness = 2.0;
        let settlement = self.calculate_settlement(
            &soil,
            applied_pressure,
            bearing_capacity,
            soil_layer_thickness,
        ) + additional_ground_settlement.unwrap_or(0.0);

        let diff_settlement = settlement * 0.3 + tilt_deg * 10.0;

        let max_settlement = match soil {
            SoilType::Rock => 10.0,
            SoilType::Sand => 50.0,
            SoilType::Loam => 75.0,
            SoilType::Silt => 100.0,
            SoilType::Clay => 150.0,
        };

        let (score, can_pass, risk_level) = self.calculate_passability(
            &soil, sf, settlement, diff_settlement, max_settlement,
        );

        GroundAnalysis {
            timestamp: Utc::now(),
            tower_id: tower.tower_id,
            soil_type: soil.as_str().to_string(),
            bearing_capacity,
            applied_pressure,
            safety_factor: sf,
            settlement,
            differential_settlement: diff_settlement,
            passability_score: score,
            can_pass,
            risk_level,
        }
    }

    pub fn analyze_all_soils(
        &self,
        tower: &TowerMetadata,
        wind_speed: f64,
        tilt_deg: f64,
    ) -> Vec<GroundAnalysis> {
        let soils = [SoilType::Rock, SoilType::Loam, SoilType::Sand, SoilType::Silt, SoilType::Clay];
        soils.iter().map(|soil| {
            self.analyze(tower, soil.clone(), wind_speed, tilt_deg, None)
        }).collect()
    }
}

impl Default for GroundAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}
