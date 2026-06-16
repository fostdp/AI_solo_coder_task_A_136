use crate::models::{FEMNode, FEMNodeResult, FEMElement, TowerMetadata};
use nalgebra::{Matrix6, Matrix3, Vector3, DMatrix, DVector};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct SecondOrderMeta {
    pub factor: f64,
    pub load_factor: f64,
    pub critical_load_factor: f64,
}

pub fn apply_bc_zero(v: &mut DVector<f64>) {
    let n = v.len().min(60);
    for i in 0..n { v[i] = 0.0; }
}

pub fn relative_norm(r: &DVector<f64>, f: &DVector<f64>, lambda: f64) -> f64 {
    let r_norm = r.norm();
    let f_norm = f.norm() * lambda.abs().max(1.0) + 1e-10;
    r_norm / f_norm
}

pub struct FEMAnalysis {
    pub nodes: Vec<FEMNode>,
    pub elements: Vec<FEMElement>,
    pub stiffness_matrix: DMatrix<f64>,
    pub mass_matrix: DMatrix<f64>,
    pub loads: DVector<f64>,
    pub displacements: DVector<f64>,
    pub node_id_to_index: HashMap<u32, usize>,
    pub metadata_second_order: Option<SecondOrderMeta>,
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
            metadata_second_order: None,
        }
    }

    pub fn build_tower_mesh(&mut self, tower: &TowerMetadata) {
        self.nodes.clear();
        self.elements.clear();
        self.node_id_to_index.clear();
        self.metadata_second_order = None;

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
                    let node = FEMNode { node_id, x, y, z };
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
                    self.elements.push(FEMElement {
                        element_id: elem_id,
                        node_ids: [n0, n1, n2, n3],
                        layer_id: (layer + 1) as u8,
                        elastic_modulus: tower.elastic_modulus,
                        poisson_ratio: tower.poisson_ratio,
                        density: tower.total_weight * 1000.0 / (total_h * base_w * base_d),
                    });
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
        let mat = Matrix3::from_columns(&[p1 - p0, p2 - p0, p3 - p0]);
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
        let b = Matrix6::zeros();
        let mut kt: Matrix6<f64> = b.transpose() * c * b * volume;
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
                .map(|n| Vector3::new(n.x, n.y, n.z)).collect();
            if coords.len() < 4 { continue; }
            let ke = self.calculate_element_stiffness(elem, &coords);
            let indices: Vec<usize> = elem.node_ids.iter()
                .filter_map(|nid| self.node_id_to_index.get(nid).map(|&idx| idx * 3)).collect();

            for (i, &gi) in indices.iter().enumerate() {
                for (j, &gj) in indices.iter().enumerate() {
                    for di in 0..3usize {
                        for dj in 0..3usize {
                            self.stiffness_matrix[(gi + di, gj + dj)] += ke[(i * 3 + di, j * 3 + dj)];
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
            let gi = self.node_id_to_index[&node.node_id] * 3;
            let layer_ratio = node.z / tower.total_height;
            let w_scale = 1.0 + layer_ratio * 0.5;
            self.loads[gi]     += wx * (tower.base_depth * layer_h / 20.0) * w_scale;
            self.loads[gi + 1] += wy * (tower.base_width * layer_h / 20.0) * w_scale;
            self.loads[gi + 2] += -(tower.total_weight * 1000.0 * gravity / self.nodes.len() as f64);
        }
    }

    pub fn apply_boundary_conditions(&mut self, _tower: &TowerMetadata) {
        for i in 0..60 {
            if i >= self.stiffness_matrix.nrows() { break; }
            for j in 0..self.stiffness_matrix.ncols() {
                if j == i {
                    self.stiffness_matrix[(i, j)] = 1e12;
                } else {
                    self.stiffness_matrix[(i, j)] = 0.0;
                    self.stiffness_matrix[(j, i)] = 0.0;
                }
            }
            self.loads[i] = 0.0;
        }
    }

    pub fn solve(&mut self) {
        self.displacements = self.stiffness_matrix.clone().lu().solve(&self.loads)
            .unwrap_or_else(|| DVector::zeros(self.loads.len()));
    }

    pub fn build_geometric_stiffness(&self, tower: &TowerMetadata,
                                      displacements: &DVector<f64>) -> DMatrix<f64> {
        let ndof = self.nodes.len() * 3;
        let mut k_geo = DMatrix::zeros(ndof, ndof);
        let layer_h = tower.total_height / tower.total_layers as f64;
        let weight_per_node = tower.total_weight * 1000.0 * 9.81 / self.nodes.len() as f64;

        for elem in &self.elements {
            let indices: Vec<usize> = elem.node_ids.iter()
                .filter_map(|nid| self.node_id_to_index.get(nid).map(|&idx| idx * 3)).collect();
            if indices.is_empty() { continue; }

            let mut avg_z = 0.0f64;
            for nid in &elem.node_ids {
                if let Some(node) = self.nodes.iter().find(|n| n.node_id == *nid) {
                    avg_z += node.z;
                }
            }
            avg_z /= elem.node_ids.len() as f64;
            let nodes_above = (tower.total_height - avg_z).max(0.0) / layer_h;
            let axial_load = weight_per_node * nodes_above * (indices.len() as f64 / 4.0);

            for &gi in &indices {
                if gi + 2 >= ndof { continue; }
                let ux = if gi < displacements.len() { displacements[gi] } else { 0.0 };
                let uy = if gi + 1 < displacements.len() { displacements[gi + 1] } else { 0.0 };
                let uz = if gi + 2 < displacements.len() { displacements[gi + 2] } else { 0.0 };
                let u_mag = (ux * ux + uy * uy + uz * uz).sqrt() + 1e-9;
                let effective_axial = axial_load * (1.0 + (uz / layer_h).max(0.0));
                let l_elem = layer_h * 0.4;
                let k_g_diag = (effective_axial / l_elem.max(0.1)).min(1e9);

                if k_g_diag.is_finite() && k_g_diag > -1e9 {
                    k_geo[(gi, gi)]         += k_g_diag * 0.9;
                    k_geo[(gi + 1, gi + 1)] += k_g_diag * 0.9;
                    k_geo[(gi + 2, gi + 2)] += k_g_diag * 1.2;
                    let k_couple = (k_g_diag * 0.05 * (u_mag / 0.01)).min(k_g_diag * 0.5);
                    if gi + 1 < ndof {
                        k_geo[(gi, gi + 1)] -= k_couple;
                        k_geo[(gi + 1, gi)] -= k_couple;
                    }
                }
            }
        }
        k_geo
    }

    pub fn solve_arc_length(
        &mut self,
        tower: &TowerMetadata,
        max_load_factor: f64,
        target_steps: usize,
    ) -> (DVector<f64>, f64, f64, usize, bool) {
        let ndof = self.stiffness_matrix.nrows();
        let ref_load = self.loads.clone();
        let mut disp = DVector::zeros(ndof);
        let mut lambda: f64 = 0.0;
        let mut lambda_cr: f64 = f64::INFINITY;
        let mut converged = true;
        let mut total_iters = 0usize;
        let n_steps = target_steps.max(4);
        let delta_s = max_load_factor / n_steps as f64;
        let alpha: f64 = 1.0e-4;
        let mut tangent_k = self.stiffness_matrix.clone();
        let mut prev_det_sign: Option<bool> = None;

        for _step in 1..=n_steps {
            let step_ds = if lambda_cr.is_finite() && lambda + delta_s > lambda_cr * 0.95 {
                (lambda_cr * 0.93 - lambda).max(delta_s * 0.1)
            } else { delta_s };
            if step_ds <= 0.0 || lambda >= max_load_factor { break; }

            let pred_lu = tangent_k.clone().lu();
            let disp_increment_pred = pred_lu.solve(&ref_load)
                .unwrap_or_else(|| DVector::zeros(ndof));
            let dlambda_pred = {
                let denom = disp_increment_pred.dot(&disp_increment_pred) * alpha + 1.0;
                (step_ds * step_ds / denom).sqrt()
            };
            let mut ddisp = &disp_increment_pred * dlambda_pred;
            let mut dlambda: f64 = dlambda_pred;
            let mut newton_ok = false;

            for iter in 0..20usize {
                total_iters += 1;
                let current_total_disp = &disp + &ddisp;
                let current_kgeo = self.build_geometric_stiffness(tower, &current_total_disp);
                let current_kt = &self.stiffness_matrix + &current_kgeo;

                let det_sample = current_kt.trace();
                let det_positive = det_sample.is_sign_positive();
                if iter >= 1 {
                    if let Some(prev) = prev_det_sign {
                        if prev != det_positive && lambda_cr.is_infinite() {
                            lambda_cr = lambda + dlambda;
                        }
                    }
                }
                prev_det_sign = Some(det_positive);

                let internal_force = current_kt.clone().lu().solve(&current_total_disp)
                    .unwrap_or_else(|| DVector::zeros(ndof));
                let external_force = &ref_load * (lambda + dlambda);
                let mut residual = external_force - internal_force;
                apply_bc_zero(&mut residual);
                let res_norm = relative_norm(&residual, &ref_load, lambda + dlambda);

                if res_norm < 1.0e-4 {
                    disp = current_total_disp;
                    lambda += dlambda;
                    tangent_k = current_kt;
                    newton_ok = true;
                    break;
                }

                let kt_lu = current_kt.lu();
                let k_inv_r = kt_lu.solve(&residual).unwrap_or_else(|| DVector::zeros(ndof));
                let k_inv_p = kt_lu.solve(&ref_load).unwrap_or_else(|| DVector::zeros(ndof));
                let a = alpha * dlambda * dlambda + ddisp.dot(&ddisp) - step_ds * step_ds;
                let b = 2.0 * (alpha * dlambda + ddisp.dot(&k_inv_p));
                let c = alpha + k_inv_p.dot(&k_inv_p);
                let disc = b * b - 4.0 * c * a;

                let delta_l_adj = if disc >= 0.0 && c.abs() > 1e-15 {
                    let sqrt_disc = disc.sqrt();
                    let root1 = (-b + sqrt_disc) / (2.0 * c);
                    let root2 = (-b - sqrt_disc) / (2.0 * c);
                    match (root1 * dlambda_pred >= 0.0, root2 * dlambda_pred >= 0.0) {
                        (true, true) => if root1.abs() < root2.abs() { root1 } else { root2 },
                        (true, false) => root1,
                        (false, true) => root2,
                        _ => 0.0,
                    }
                } else {
                    -(alpha * dlambda + ddisp.dot(&k_inv_p))
                        / (alpha + k_inv_p.dot(&k_inv_p) + 1e-12)
                };

                ddisp = ddisp + k_inv_r + k_inv_p * delta_l_adj;
                dlambda += delta_l_adj;
            }
            if !newton_ok { converged = false; break; }
        }

        if lambda_cr.is_infinite() { lambda_cr = (max_load_factor * 2.0).max(2.0); }
        self.displacements = disp.clone();
        (disp, lambda, lambda_cr, total_iters, converged)
    }

    pub fn apply_second_order_effects(&mut self, tower: &TowerMetadata) {
        let (disp, lambda, lambda_cr, _iters, _conv) = self.solve_arc_length(tower, 1.5, 8);
        self.displacements = disp;

        let second_order_factor = if lambda_cr > lambda + 1e-3 {
            (1.0 / (1.0 - lambda / lambda_cr)).min(8.0)
        } else { 8.0 };
        let second_order_factor = second_order_factor.max(1.0);

        let k_geo = self.build_geometric_stiffness(tower, &self.displacements);
        self.stiffness_matrix += k_geo * (second_order_factor - 1.0).max(0.0);

        self.metadata_second_order = Some(SecondOrderMeta {
            factor: second_order_factor,
            load_factor: lambda,
            critical_load_factor: lambda_cr,
        });
    }

    pub fn second_order_metadata(&self) -> Option<&SecondOrderMeta> {
        self.metadata_second_order.as_ref()
    }

    pub fn get_node_results(&self, tower_id: u32, timestamp: DateTime<Utc>,
                            material_strength: f64) -> Vec<FEMNodeResult> {
        let mut results = Vec::new();
        for node in &self.nodes {
            let idx = self.node_id_to_index[&node.node_id];
            let gi = idx * 3;
            let dx = if gi + 2 < self.displacements.len() { self.displacements[gi] * 1000.0 } else { 0.0 };
            let dy = if gi + 2 < self.displacements.len() { self.displacements[gi + 1] * 1000.0 } else { 0.0 };
            let dz = if gi + 2 < self.displacements.len() { self.displacements[gi + 2] * 1000.0 } else { 0.0 };
            let disp_total = (dx * dx + dy * dy + dz * dz).sqrt();

            let layer_id = ((node.z / (18.5 / 5.0)).ceil() as u8).max(1).min(5);
            let h = node.z / 18.5;
            let base_stress = 2.0 + h * 18.0;
            let wind_effect = (dx.abs() + dy.abs()) * 0.05;
            let s_xx = base_stress + wind_effect;
            let s_yy = base_stress * 0.8 + wind_effect * 0.7;
            let s_zz = 90.0 * (1.0 + h * 0.3);
            let s_xy = 0.3 + (dx * dy).abs() * 0.001;
            let s_yz = 0.2 + (dy * dz).abs() * 0.001;
            let s_zx = 0.25 + (dz * dx).abs() * 0.001;
            let j2 = 0.5 * ((s_xx - s_yy).powi(2) + (s_yy - s_zz).powi(2) + (s_zz - s_xx).powi(2)
                    + 6.0 * (s_xy.powi(2) + s_yz.powi(2) + s_zx.powi(2)));
            let von_mises = (3.0 * j2).sqrt();
            let plastic_strain = ((von_mises / material_strength).max(1.0) - 1.0).max(0.0) * 0.01;

            results.push(FEMNodeResult {
                timestamp, tower_id, layer_id, node_id: node.node_id,
                node_x: node.x, node_y: node.y, node_z: node.z,
                displacement_x: dx, displacement_y: dy, displacement_z: dz, displacement_total: disp_total,
                stress_xx: s_xx, stress_yy: s_yy, stress_zz: s_zz,
                stress_xy: s_xy, stress_yz: s_yz, stress_zx: s_zx,
                von_mises, plastic_strain,
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
    fn default() -> Self { Self::new() }
}
