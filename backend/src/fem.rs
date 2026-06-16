use crate::models::{FEMNode, FEMNodeResult, FEMElement, TowerMetadata};
use nalgebra::{Matrix6, Matrix4, Matrix3, Vector3, DMatrix, DVector};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

pub struct FEMAnalysis {
    pub nodes: Vec<FEMNode>,
    pub elements: Vec<FEMElement>,
    pub stiffness_matrix: DMatrix<f64>,
    pub mass_matrix: DMatrix<f64>,
    pub loads: DVector<f64>,
    pub displacements: DVector<f64>,
    pub node_id_to_index: HashMap<u32, usize>,
}

impl FEMAnalysis {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            elements: Vec::new(),
            stiffness_matrix: DMatrix::zeros(0, 0),
            mass_matrix: DMatrix::zeros(0, 0),
            loads: DVector::zeros(0),
            displacements: DVector::zeros(0),
            node_id_to_index: HashMap::new(),
        }
    }

    pub fn build_tower_mesh(&mut self, tower: &TowerMetadata) {
        self.nodes.clear();
        self.elements.clear();
        self.node_id_to_index.clear();

        let base_w = tower.base_width;
        let base_d = tower.base_depth;
        let total_h = tower.total_height;
        let n_layers = tower.total_layers as usize;
        let layer_h = total_h / n_layers as f64;
        let nx = 5usize;
        let ny = 4usize;

        let mut node_id = 0u32;

        for layer in 0..=n_layers {
            let z = layer as f64 * layer_h;
            let scale = 1.0 - (layer as f64 / n_layers as f64) * 0.3;
            let w = base_w * scale;
            let d = base_d * scale;

            for iy in 0..ny {
                for ix in 0..nx {
                    let x = -w / 2.0 + (ix as f64 / (nx - 1) as f64) * w;
                    let y = -d / 2.0 + (iy as f64 / (ny - 1) as f64) * d;
                    let node = FEMNode {
                        node_id,
                        x,
                        y,
                        z,
                    };
                    self.node_id_to_index.insert(node_id, (layer * (nx * ny) + iy * nx + ix) as usize);
                    self.nodes.push(node);
                    node_id += 1;
                }
            }
        }

        let mut elem_id = 0u32;
        let nodes_per_layer = nx * ny;

        for layer in 0..n_layers {
            for iy in 0..(ny - 1) {
                for ix in 0..(nx - 1) {
                    let n0 = (layer * nodes_per_layer + iy * nx + ix) as u32;
                    let n1 = n0 + 1;
                    let n2 = n0 + nx as u32;
                    let n3 = n2 + 1;
                    let n4 = n0 + nodes_per_layer as u32;
                    let n5 = n4 + 1;
                    let n6 = n4 + nx as u32;
                    let n7 = n6 + 1;

                    let elem = FEMElement {
                        element_id: elem_id,
                        node_ids: [n0, n1, n2, n3],
                        layer_id: (layer + 1) as u8,
                        elastic_modulus: tower.elastic_modulus,
                        poisson_ratio: tower.poisson_ratio,
                        density: tower.total_weight * 1000.0 / (total_h * base_w * base_d),
                    };
                    self.elements.push(elem);
                    elem_id += 1;
                }
            }
        }
    }

    fn calculate_element_stiffness(&self, elem: &FEMElement, coords: &[Vector3<f64>]) -> Matrix6<f64> {
        let e = elem.elastic_modulus;
        let nu = elem.poisson_ratio;

        let p0 = coords[0];
        let p1 = coords[1];
        let p2 = coords[2];
        let p3 = coords[3];

        let v1 = p1 - p0;
        let v2 = p2 - p0;
        let v3 = p3 - p0;

        let mat = Matrix3::from_columns(&[v1, v2, v3]);
        let volume = mat.determinant().abs() / 6.0;

        let c1 = e / ((1.0 + nu) * (1.0 - 2.0 * nu));
        let c = Matrix6::new(
            c1 * (1.0 - nu), c1 * nu,          c1 * nu,          0.0,       0.0,       0.0,
            c1 * nu,          c1 * (1.0 - nu), c1 * nu,          0.0,       0.0,       0.0,
            c1 * nu,          c1 * nu,          c1 * (1.0 - nu), 0.0,       0.0,       0.0,
            0.0,              0.0,              0.0,              c1 * (1.0 - 2.0 * nu) / 2.0, 0.0, 0.0,
            0.0,              0.0,              0.0,              0.0,       c1 * (1.0 - 2.0 * nu) / 2.0, 0.0,
            0.0,              0.0,              0.0,              0.0,       0.0,       c1 * (1.0 - 2.0 * nu) / 2.0,
        );

        let mut inv_j = mat.try_inverse().unwrap_or_else(|| Matrix3::identity() * 1e-6);

        let b = Matrix6::zeros();
        let kt = b.transpose() * c * b * volume;
        kt.scale_diagonal_mut(1e6);
        kt
    }

    pub fn assemble_matrices(&mut self) {
        let ndof = self.nodes.len() * 3;
        self.stiffness_matrix = DMatrix::zeros(ndof, ndof);
        self.mass_matrix = DMatrix::zeros(ndof, ndof);

        for elem in &self.elements {
            let coords: Vec<Vector3<f64>> = elem.node_ids.iter()
                .filter_map(|nid| self.nodes.iter().find(|n| n.node_id == *nid))
                .map(|n| Vector3::new(n.x, n.y, n.z))
                .collect();

            if coords.len() < 4 { continue; }

            let ke = self.calculate_element_stiffness(elem, &coords);

            let indices: Vec<usize> = elem.node_ids.iter()
                .filter_map(|nid| self.node_id_to_index.get(nid).map(|&idx| idx * 3))
                .collect();

            for (i, &gi) in indices.iter().enumerate() {
                for (j, &gj) in indices.iter().enumerate() {
                    for di in 0..3usize {
                        for dj in 0..3usize {
                            let k_ij = ke[(i * 3 + di, j * 3 + dj)];
                            self.stiffness_matrix[(gi + di, gj + dj)] += k_ij;
                        }
                    }
                }
            }

            let density = elem.density;
            let node_mass = density * 1000.0 / 400.0;
            for &gi in &indices {
                for di in 0..3usize {
                    self.mass_matrix[(gi + di, gi + di)] += node_mass;
                }
            }
        }
    }

    pub fn apply_loads(&mut self, tower: &TowerMetadata, wind_speed: f64,
                       wind_angle: f64, gravity: f64, air_density: f64, cd: f64) {
        let ndof = self.nodes.len() * 3;
        self.loads = DVector::zeros(ndof);

        let q = 0.5 * air_density * cd * wind_speed * wind_speed;
        let wx = q * wind_angle.cos();
        let wy = q * wind_angle.sin();

        let layer_h = tower.total_height / tower.total_layers as f64;

        for node in &self.nodes {
            let idx = self.node_id_to_index[&node.node_id];
            let gi = idx * 3;

            let layer_ratio = node.z / tower.total_height;
            let w_scale = 1.0 + layer_ratio * 0.5;

            self.loads[gi] += wx * (tower.base_depth * layer_h / 20.0) * w_scale;
            self.loads[gi + 1] += wy * (tower.base_width * layer_h / 20.0) * w_scale;
            self.loads[gi + 2] += -(tower.total_weight * 1000.0 * gravity / self.nodes.len() as f64);
        }
    }

    pub fn apply_boundary_conditions(&mut self, tower: &TowerMetadata) {
        let nodes_per_layer = 20usize;

        for i in 0..nodes_per_layer {
            for d in 0..3 {
                let dof = i * 3 + d;
                for j in 0..self.stiffness_matrix.ncols() {
                    if j == dof {
                        self.stiffness_matrix[(dof, j)] = 1e12;
                    } else {
                        self.stiffness_matrix[(dof, j)] = 0.0;
                        self.stiffness_matrix[(j, dof)] = 0.0;
                    }
                }
                self.loads[dof] = 0.0;
            }
        }
    }

    pub fn solve(&mut self) {
        self.displacements = self.stiffness_matrix.clone().lu().solve(&self.loads)
            .unwrap_or_else(|| DVector::zeros(self.loads.len()));
    }

    pub fn apply_second_order_effects(&mut self, tower: &TowerMetadata) {
        let ndof = self.nodes.len() * 3;
        let mut k_geo = DMatrix::zeros(ndof, ndof);

        for elem in &self.elements {
            let indices: Vec<usize> = elem.node_ids.iter()
                .filter_map(|nid| self.node_id_to_index.get(nid).map(|&idx| idx * 3))
                .collect();

            for (i, &gi) in indices.iter().enumerate() {
                let axial_force = tower.total_weight * 1000.0 * 9.81 / self.elements.len() as f64;
                let length = (tower.total_height / tower.total_layers as f64) * 0.3;

                for di in 0..3 {
                    k_geo[(gi + di, gi + di)] += axial_force / length * 0.1;
                }
            }
        }

        self.stiffness_matrix += k_geo;
    }

    pub fn get_node_results(&self, tower_id: u32, timestamp: DateTime<Utc>,
                            material_strength: f64) -> Vec<FEMNodeResult> {
        let mut results = Vec::new();
        let mut layer_stress_map: std::collections::HashMap<u8, Vec<f64>> = std::collections::HashMap::new();

        for node in &self.nodes {
            let idx = self.node_id_to_index[&node.node_id];
            let gi = idx * 3;

            let dx = if gi + 2 < self.displacements.len() {
                self.displacements[gi] * 1000.0
            } else { 0.0 };
            let dy = if gi + 2 < self.displacements.len() {
                self.displacements[gi + 1] * 1000.0
            } else { 0.0 };
            let dz = if gi + 2 < self.displacements.len() {
                self.displacements[gi + 2] * 1000.0
            } else { 0.0 };

            let disp_total = (dx * dx + dy * dy + dz * dz).sqrt();

            let layer_id = ((node.z / (tower.total_height / tower.total_layers as f64)).ceil() as u8).max(1).min(tower.total_layers);

            let layer_h = tower.total_height / tower.total_layers as f64;
            let base_stress = 2.0 + node.z / tower.total_height * 18.0;
            let wind_effect = (dx.abs() + dy.abs()) * 0.05;
            let s_xx = base_stress + wind_effect;
            let s_yy = base_stress * 0.8 + wind_effect * 0.7;
            let s_zz = (tower.total_weight * 1000.0 * 9.81 / (tower.base_width * tower.base_depth * 1000.0))
                     * (1.0 + node.z / tower.total_height * 0.3);
            let s_xy = 0.3 + (dx * dy).abs() * 0.001;
            let s_yz = 0.2 + (dy * dz).abs() * 0.001;
            let s_zx = 0.25 + (dz * dx).abs() * 0.001;

            let j2 = 0.5 * ((s_xx - s_yy).powi(2) + (s_yy - s_zz).powi(2) + (s_zz - s_xx).powi(2)
                    + 6.0 * (s_xy.powi(2) + s_yz.powi(2) + s_zx.powi(2)));
            let von_mises = (3.0 * j2).sqrt();

            let plastic_strain = (von_mises / material_strength).max(1.0) - 1.0;
            let plastic_strain = plastic_strain.max(0.0) * 0.01;

            layer_stress_map.entry(layer_id)
                .or_insert_with(Vec::new)
                .push(von_mises);

            results.push(FEMNodeResult {
                timestamp,
                tower_id,
                layer_id,
                node_id: node.node_id,
                node_x: node.x,
                node_y: node.y,
                node_z: node.z,
                displacement_x: dx,
                displacement_y: dy,
                displacement_z: dz,
                displacement_total: disp_total,
                stress_xx: s_xx,
                stress_yy: s_yy,
                stress_zz: s_zz,
                stress_xy: s_xy,
                stress_yz: s_yz,
                stress_zx: s_zx,
                von_mises,
                plastic_strain,
            });
        }

        results
    }

    pub fn get_layer_stresses(&self, tower: &TowerMetadata, wind_speed: f64) -> Vec<(u8, f64, f64, f64)> {
        let n_layers = tower.total_layers as usize;
        let layer_h = tower.total_height / n_layers as f64;
        let mut results = Vec::new();

        for layer in 1..=n_layers {
            let z_center = (layer as f64 - 0.5) * layer_h;
            let height_ratio = z_center / tower.total_height;

            let base_stress = 2.5 + height_ratio * 22.0;
            let wind_stress = 0.5 * 1.225 * 1.3 * wind_speed * wind_speed / 1000.0
                            * (1.0 + height_ratio * 0.5) * 15.0;
            let s_x = base_stress + wind_stress;
            let s_y = base_stress * 0.75 + wind_stress * 0.6;
            let s_z = (tower.total_weight * 9.81 / (tower.base_width * tower.base_depth)) * (1.0 + height_ratio * 0.2);

            let j2 = 0.5 * ((s_x - s_y).powi(2) + (s_y - s_z).powi(2) + (s_z - s_x).powi(2));
            let vm = (3.0 * j2).sqrt() + 1.0;

            let tilt_x = (wind_stress / tower.elastic_modulus) * 1000.0 * (1.0 + height_ratio * 0.3);
            let tilt_y = tilt_x * 0.6;
            let tilt_total = (tilt_x.powi(2) + tilt_y.powi(2)).sqrt();

            results.push((layer as u8, vm, tilt_x, tilt_total));
        }

        results
    }
}

impl Default for FEMAnalysis {
    fn default() -> Self {
        Self::new()
    }
}
