import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';

export class Tower3DViewer {
    constructor(canvas, tower) {
        this.canvas = canvas;
        this.tower = tower;
        this.scene = null;
        this.camera = null;
        this.renderer = null;
        this.controls = null;
        this.towerGroup = null;
        this.layerMeshes = [];
        this.stressColors = true;
        this.tiltX = 0;
        this.tiltY = 0;
        this.animating = false;
        this.init();
    }

    init() {
        const rect = this.canvas.parentElement.getBoundingClientRect();
        this.scene = new THREE.Scene();
        this.scene.background = new THREE.Color(0x0a0f1a);
        this.scene.fog = new THREE.FogExp2(0x0a0f1a, 0.015);

        this.camera = new THREE.PerspectiveCamera(45, rect.width / rect.height, 0.1, 1000);
        this.setCameraView('perspective');

        this.renderer = new THREE.WebGLRenderer({
            canvas: this.canvas,
            antialias: true,
            alpha: true
        });
        this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
        this.renderer.setSize(rect.width, rect.height, false);
        this.renderer.shadowMap.enabled = true;
        this.renderer.shadowMap.type = THREE.PCFSoftShadowMap;

        this.controls = new OrbitControls(this.camera, this.renderer.domElement);
        this.controls.enableDamping = true;
        this.controls.dampingFactor = 0.05;
        this.controls.minDistance = 10;
        this.controls.maxDistance = 80;
        this.controls.maxPolarAngle = Math.PI * 0.49;

        this.setupLights();
        this.buildTower();
        this.buildGround();
        this.buildGrid();

        window.addEventListener('resize', () => this.onResize());
        this.animate();
    }

    setupLights() {
        const ambient = new THREE.AmbientLight(0x404060, 0.6);
        this.scene.add(ambient);

        const hemi = new THREE.HemisphereLight(0x87ceeb, 0x3a2a1a, 0.4);
        this.scene.add(hemi);

        const sun = new THREE.DirectionalLight(0xfff5e1, 1.2);
        sun.position.set(25, 35, 20);
        sun.castShadow = true;
        sun.shadow.mapSize.width = 2048;
        sun.shadow.mapSize.height = 2048;
        sun.shadow.camera.near = 0.5;
        sun.shadow.camera.far = 150;
        sun.shadow.camera.left = -40;
        sun.shadow.camera.right = 40;
        sun.shadow.camera.top = 40;
        sun.shadow.camera.bottom = -40;
        this.scene.add(sun);

        const fill = new THREE.DirectionalLight(0x6080a0, 0.4);
        fill.position.set(-20, 10, -15);
        this.scene.add(fill);

        const rim = new THREE.PointLight(0x4080ff, 0.5, 60);
        rim.position.set(-10, 15, -15);
        this.scene.add(rim);
    }

    buildGround() {
        const geo = new THREE.PlaneGeometry(120, 120, 50, 50);
        const pos = geo.attributes.position;
        for (let i = 0; i < pos.count; i++) {
            const x = pos.getX(i);
            const y = pos.getY(i);
            const d = Math.sqrt(x*x + y*y);
            const z = Math.sin(d * 0.08) * 0.15 + (Math.random() - 0.5) * 0.05;
            pos.setZ(i, z);
        }
        geo.computeVertexNormals();
        const mat = new THREE.MeshStandardMaterial({
            color: 0x2a3a2a,
            roughness: 0.95,
            metalness: 0.0,
        });
        const ground = new THREE.Mesh(geo, mat);
        ground.rotation.x = -Math.PI / 2;
        ground.position.y = -0.05;
        ground.receiveShadow = true;
        this.scene.add(ground);

        const shadowGeo = new THREE.CircleGeometry(12, 64);
        const shadowMat = new THREE.MeshBasicMaterial({
            color: 0x000000,
            transparent: true,
            opacity: 0.35
        });
        const shadow = new THREE.Mesh(shadowGeo, shadowMat);
        shadow.rotation.x = -Math.PI / 2;
        shadow.position.y = 0.01;
        this.scene.add(shadow);
    }

    buildGrid() {
        const grid = new THREE.GridHelper(60, 60, 0x2a3a5c, 0x1a2438);
        grid.position.y = 0;
        this.scene.add(grid);

        const axes = new THREE.AxesHelper(5);
        axes.position.set(-14, 0.05, -14);
        this.scene.add(axes);
    }

    buildTower() {
        this.towerGroup = new THREE.Group();
        this.layerMeshes = [];

        const { total_height, total_layers, base_width, base_depth, material_strength } = this.tower;
        const layer_h = total_height / total_layers;

        const platformGeo = new THREE.BoxGeometry(0.2, 0.15, 0.2);
        const platformMat = new THREE.MeshStandardMaterial({
            color: 0x3a2a1a, roughness: 0.8, metalness: 0.1
        });
        for (let i = 0; i < 4; i++) {
            const sign = i % 2 === 0 ? 1 : -1;
            const sign2 = i < 2 ? 1 : -1;
            const wheel = new THREE.Mesh(
                new THREE.CylinderGeometry(0.5, 0.5, 0.25, 24),
                new THREE.MeshStandardMaterial({ color: 0x1a1208, roughness: 0.95, metalness: 0.2 })
            );
            wheel.rotation.z = Math.PI / 2;
            wheel.position.set(
                sign * (base_width / 2 - 0.3),
                0.5,
                sign2 * (base_depth / 2 - 0.3)
            );
            wheel.castShadow = true;
            wheel.receiveShadow = true;
            this.towerGroup.add(wheel);
        }

        for (let layer = 1; layer <= total_layers; layer++) {
            const layerGroup = new THREE.Group();
            const h_ratio = layer / total_layers;
            const scale = 1.0 - h_ratio * 0.3;
            const w = base_width * scale;
            const d = base_depth * scale;
            const y_base = (layer - 1) * layer_h;

            const w_plank = w / 8;
            const woodGeo = new THREE.BoxGeometry(w, layer_h, d);
            const woodMat = this.createWoodMaterial(layer);
            const mainBox = new THREE.Mesh(woodGeo, woodMat);
            mainBox.position.set(0, y_base + layer_h / 2, 0);
            mainBox.castShadow = true;
            mainBox.receiveShadow = true;
            layerGroup.add(mainBox);

            const frameThick = 0.18;
            const frameMat = new THREE.MeshStandardMaterial({
                color: 0x4a3a1a,
                roughness: 0.75,
                metalness: 0.05
            });
            const corners = [
                [-w/2, -d/2], [w/2, -d/2], [-w/2, d/2], [w/2, d/2]
            ];
            for (const [cx, cz] of corners) {
                const post = new THREE.Mesh(
                    new THREE.BoxGeometry(frameThick, layer_h + 0.05, frameThick),
                    frameMat
                );
                post.position.set(cx, y_base + layer_h / 2, cz);
                post.castShadow = true;
                layerGroup.add(post);
            }

            const horizCount = 5;
            for (let hc = 1; hc <= horizCount; hc++) {
                const hy = y_base + (hc / horizCount) * layer_h;
                const beamX = new THREE.Mesh(
                    new THREE.BoxGeometry(w + frameThick * 2, 0.1, frameThick * 0.8),
                    frameMat
                );
                beamX.position.set(0, hy, 0);
                beamX.castShadow = true;
                layerGroup.add(beamX);
                const beamZ = new THREE.Mesh(
                    new THREE.BoxGeometry(frameThick * 0.8, 0.1, d + frameThick * 2),
                    frameMat
                );
                beamZ.position.set(0, hy, 0);
                beamZ.castShadow = true;
                layerGroup.add(beamZ);
            }

            const diagMat = new THREE.MeshStandardMaterial({
                color: 0x5a4018, roughness: 0.8, metalness: 0.05
            });
            for (const side of ['front', 'back', 'left', 'right']) {
                const diag1 = new THREE.Mesh(
                    new THREE.BoxGeometry(0.08, layer_h * 1.3, 0.08),
                    diagMat
                );
                const diag2 = new THREE.Mesh(
                    new THREE.BoxGeometry(0.08, layer_h * 1.3, 0.08),
                    diagMat
                );
                diag1.castShadow = true;
                diag2.castShadow = true;
                const off = 0.06;
                if (side === 'front') {
                    diag1.position.set(w/4, y_base + layer_h/2, d/2 + off);
                    diag1.rotation.z = -0.3;
                    diag2.position.set(-w/4, y_base + layer_h/2, d/2 + off);
                    diag2.rotation.z = 0.3;
                } else if (side === 'back') {
                    diag1.position.set(w/4, y_base + layer_h/2, -d/2 - off);
                    diag1.rotation.z = 0.3;
                    diag2.position.set(-w/4, y_base + layer_h/2, -d/2 - off);
                    diag2.rotation.z = -0.3;
                } else if (side === 'left') {
                    diag1.position.set(-w/2 - off, y_base + layer_h/2, d/4);
                    diag1.rotation.x = 0.3;
                    diag2.position.set(-w/2 - off, y_base + layer_h/2, -d/4);
                    diag2.rotation.x = -0.3;
                } else {
                    diag1.position.set(w/2 + off, y_base + layer_h/2, d/4);
                    diag1.rotation.x = -0.3;
                    diag2.position.set(w/2 + off, y_base + layer_h/2, -d/4);
                    diag2.rotation.x = 0.3;
                }
                layerGroup.add(diag1);
                layerGroup.add(diag2);
            }

            if (layer === total_layers) {
                const roofH = 2.5;
                const roofGeo = new THREE.ConeGeometry(w * 0.85, roofH, 4);
                const roofMat = new THREE.MeshStandardMaterial({
                    color: 0x6a4a1a, roughness: 0.9, metalness: 0.0,
                    side: THREE.DoubleSide
                });
                const roof = new THREE.Mesh(roofGeo, roofMat);
                roof.position.y = y_base + layer_h + roofH / 2;
                roof.rotation.y = Math.PI / 4;
                roof.castShadow = true;
                layerGroup.add(roof);

                const flagPole = new THREE.Mesh(
                    new THREE.CylinderGeometry(0.05, 0.05, 3, 8),
                    new THREE.MeshStandardMaterial({ color: 0x2a2010 })
                );
                flagPole.position.y = y_base + layer_h + roofH + 1.5;
                layerGroup.add(flagPole);

                const flagGeo = new THREE.PlaneGeometry(1.5, 0.9);
                const flagMat = new THREE.MeshStandardMaterial({
                    color: 0x8b0000,
                    side: THREE.DoubleSide,
                    roughness: 0.9
                });
                const flag = new THREE.Mesh(flagGeo, flagMat);
                flag.position.set(0.8, y_base + layer_h + roofH + 2.0, 0);
                layerGroup.add(flag);

                const flagChar = new THREE.Mesh(
                    new THREE.PlaneGeometry(0.5, 0.5),
                    new THREE.MeshBasicMaterial({ color: 0xffcc00 })
                );
                flagChar.position.set(0.8, y_base + layer_h + roofH + 2.0, 0.01);
                layerGroup.add(flagChar);
            }

            if (layer >= 2 && layer < total_layers) {
                for (const side of ['front', 'back']) {
                    const openingW = w * 0.35;
                    const openingH = layer_h * 0.55;
                    const slitGeo = new THREE.BoxGeometry(openingW, openingH, 0.1);
                    const slitMat = new THREE.MeshStandardMaterial({
                        color: 0x0a0a0a,
                        roughness: 1.0,
                        metalness: 0.0
                    });
                    const slit = new THREE.Mesh(slitGeo, slitMat);
                    const zOff = side === 'front' ? d/2 + 0.01 : -d/2 - 0.01;
                    slit.position.set(0, y_base + layer_h/2, zOff);
                    layerGroup.add(slit);

                    const barCount = 4;
                    const barMat = new THREE.MeshStandardMaterial({
                        color: 0x2a2010, roughness: 0.9
                    });
                    for (let b = 1; b < barCount; b++) {
                        const bar = new THREE.Mesh(
                            new THREE.BoxGeometry(0.05, openingH * 0.95, 0.06),
                            barMat
                        );
                        bar.position.set(
                            -openingW/2 + (openingW / barCount) * b,
                            y_base + layer_h/2,
                            zOff + 0.04
                        );
                        layerGroup.add(bar);
                    }
                }
            }

            this.towerGroup.add(layerGroup);
            this.layerMeshes.push({
                layer,
                group: layerGroup,
                baseMaterials: this.collectMaterials(layerGroup),
                stressValue: 0,
                stressRatio: 0
            });
        }

        this.towerGroup.position.y = 0;
        this.scene.add(this.towerGroup);

        const labelCanvas = document.createElement('canvas');
        labelCanvas.width = 256;
        labelCanvas.height = 64;
        const ctx = labelCanvas.getContext('2d');
        ctx.fillStyle = 'rgba(10, 15, 26, 0.85)';
        ctx.roundRect ? ctx.roundRect(0, 0, 256, 64, 10) : ctx.rect(0, 0, 256, 64);
        ctx.fill();
        ctx.strokeStyle = '#3b82f6';
        ctx.lineWidth = 2;
        ctx.stroke();
        ctx.fillStyle = '#e8edf5';
        ctx.font = 'bold 18px "Microsoft YaHei", sans-serif';
        ctx.textAlign = 'center';
        ctx.fillText(this.tower.tower_name, 128, 28);
        ctx.font = '12px "Microsoft YaHei", sans-serif';
        ctx.fillStyle = '#60a5fa';
        ctx.fillText(`H=${this.tower.total_height}m  ${this.tower.total_layers}层`, 128, 48);

        const tex = new THREE.CanvasTexture(labelCanvas);
        const sprite = new THREE.Sprite(new THREE.SpriteMaterial({ map: tex }));
        sprite.position.set(0, total_height + 4, 0);
        sprite.scale.set(6, 1.5, 1);
        this.towerGroup.add(sprite);

        this.legendMaxStress = material_strength;
    }

    createWoodMaterial(layer) {
        const colors = [0x6b4423, 0x7a4e2a, 0x5d3b1c, 0x8b5a2b, 0x6a3f1e];
        const color = colors[layer % colors.length];
        return new THREE.MeshStandardMaterial({
            color,
            roughness: 0.82,
            metalness: 0.03,
        });
    }

    collectMaterials(group) {
        const materials = [];
        group.traverse(obj => {
            if (obj.isMesh && obj.material) {
                if (Array.isArray(obj.material)) {
                    obj.material.forEach((m, i) => {
                        materials.push({ mesh: obj, index: i, base: m.clone() });
                    });
                } else {
                    materials.push({ mesh: obj, index: -1, base: obj.material.clone() });
                }
            }
        });
        return materials;
    }

    updateLayerStresses(layerStresses, criticalStress) {
        for (const ls of layerStresses) {
            const entry = this.layerMeshes.find(m => m.layer === ls.layer);
            if (!entry) continue;

            const ratio = Math.min(ls.stress / criticalStress, 1.0);
            entry.stressValue = ls.stress;
            entry.stressRatio = ratio;

            if (this.stressColors) {
                const color = this.stressColor(ratio);
                for (const m of entry.baseMaterials) {
                    const mat = m.index >= 0 ? m.mesh.material[m.index] : m.mesh.material;
                    if (mat.color && !m.base.name?.includes('frame') && !m.base.name?.includes('diag')) {
                        mat.color.copy(color);
                        mat.emissive = color.clone().multiplyScalar(0.08 * ratio);
                        mat.needsUpdate = true;
                    }
                }
            }
        }
        this.updateLegend();
    }

    stressColor(ratio) {
        ratio = Math.max(0, Math.min(1, ratio));
        const stops = [
            { t: 0.0, c: new THREE.Color(0x10b981) },
            { t: 0.3, c: new THREE.Color(0x6ee7b7) },
            { t: 0.5, c: new THREE.Color(0xfacc15) },
            { t: 0.7, c: new THREE.Color(0xfb923c) },
            { t: 0.9, c: new THREE.Color(0xf87171) },
            { t: 1.0, c: new THREE.Color(0xdc2626) },
        ];
        for (let i = 0; i < stops.length - 1; i++) {
            if (ratio >= stops[i].t && ratio <= stops[i + 1].t) {
                const local = (ratio - stops[i].t) / (stops[i + 1].t - stops[i].t);
                return stops[i].c.clone().lerp(stops[i + 1].c, local);
            }
        }
        return stops[stops.length - 1].c;
    }

    setStressView(enabled) {
        this.stressColors = enabled;
        if (!enabled) {
            for (const entry of this.layerMeshes) {
                for (const m of entry.baseMaterials) {
                    const mat = m.index >= 0 ? m.mesh.material[m.index] : m.mesh.material;
                    if (mat.color) {
                        mat.color.copy(m.base.color);
                        if (mat.emissive) mat.emissive.setHex(0x000000);
                    }
                }
            }
        } else if (this.layerMeshes.some(m => m.stressRatio > 0)) {
            const crit = this.legendMaxStress;
            const stresses = this.layerMeshes.map(m => ({ layer: m.layer, stress: m.stressValue || crit * 0.5 }));
            this.updateLayerStresses(stresses, crit);
        }
    }

    updateLegend() {
        const max = this.legendMaxStress || 45;
        const labels = ['legendMin', 'legendQ1', 'legendMid', 'legendQ3', 'legendMax'];
        const vals = [0, max * 0.25, max * 0.5, max * 0.75, max];
        labels.forEach((id, i) => {
            const el = document.getElementById(id);
            if (el) el.textContent = vals[i].toFixed(1);
        });
    }

    updateTilt(tiltX, tiltY) {
        this.tiltX = tiltX;
        this.tiltY = tiltY;
        if (this.towerGroup) {
            const rx = THREE.MathUtils.degToRad(tiltX);
            const ry = THREE.MathUtils.degToRad(tiltY);
            this.towerGroup.rotation.z = -ry * 0.9;
            this.towerGroup.rotation.x = rx * 0.6;
        }
    }

    setCameraView(view) {
        const h = this.tower.total_height;
        const bw = this.tower.base_width;
        const bd = this.tower.base_depth;
        const d = Math.max(bw, bd, h) * 2.2;

        switch (view) {
            case 'front':
                this.camera.position.set(0, h * 0.5, d);
                break;
            case 'side':
                this.camera.position.set(d, h * 0.5, 0);
                break;
            case 'top':
                this.camera.position.set(0, d * 1.2, 0.01);
                break;
            case 'iso':
                this.camera.position.set(d * 0.8, d * 0.7, d * 0.8);
                break;
            case 'perspective':
            default:
                this.camera.position.set(-d * 0.9, h * 0.7, d * 0.9);
        }

        if (this.controls) {
            this.controls.target.set(0, h * 0.4, 0);
            this.controls.update();
        } else {
            this.camera.lookAt(0, h * 0.4, 0);
        }
    }

    onResize() {
        const rect = this.canvas.parentElement.getBoundingClientRect();
        this.camera.aspect = rect.width / rect.height;
        this.camera.updateProjectionMatrix();
        this.renderer.setSize(rect.width, rect.height, false);
    }

    animate() {
        this.animating = true;
        const tick = () => {
            if (!this.animating) return;
            requestAnimationFrame(tick);
            this.controls?.update();

            const t = Date.now() * 0.001;
            if (this.towerGroup) {
                this.towerGroup.children.forEach(child => {
                    if (child.type === 'Sprite') {
                        child.material.opacity = 0.95 + Math.sin(t * 2) * 0.05;
                    }
                });
            }

            this.renderer.render(this.scene, this.camera);
        };
        tick();
    }

    dispose() {
        this.animating = false;
        this.renderer?.dispose();
        this.controls?.dispose();
    }
}
