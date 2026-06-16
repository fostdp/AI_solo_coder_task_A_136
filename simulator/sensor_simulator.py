"""
古代临冲吕公车（攻城塔）传感器模拟器
每辆攻城塔每1分钟通过模拟传感器上报各层应力、倾斜角度、风荷载、地面承载力
"""

import json
import time
import random
import math
import requests
import argparse
from datetime import datetime, timezone
from typing import List, Dict, Any

TYPES = {
    1: {
        "tower_id": 1,
        "tower_name": "临冲吕公车-一号",
        "build_date": "1450-03-15",
        "material": "松木+铁木",
        "total_height": 18.5,
        "total_layers": 5,
        "base_width": 6.2,
        "base_depth": 4.8,
        "total_weight": 28.5,
        "design_load": 850.0,
        "design_wind_speed": 35.0,
        "material_strength": 45.0,
        "elastic_modulus": 12000.0,
        "poisson_ratio": 0.38,
    },
    2: {
        "tower_id": 2,
        "tower_name": "临冲吕公车-二号",
        "build_date": "1452-07-22",
        "material": "柏木+楠木",
        "total_height": 21.0,
        "total_layers": 6,
        "base_width": 6.8,
        "base_depth": 5.2,
        "total_weight": 36.8,
        "design_load": 1020.0,
        "design_wind_speed": 40.0,
        "material_strength": 52.0,
        "elastic_modulus": 13500.0,
        "poisson_ratio": 0.36,
    },
}

SOIL_TYPES = ["sand", "clay", "silt", "rock", "loam"]

AIR_DENSITY = 1.225
WIND_DRAG_COEFFICIENT = 1.3
GRAVITY = 9.81


class SensorSimulator:
    def __init__(self, tower_id: int, api_base: str, interval: int = 60,
                 anomaly_prob: float = 0.1, storm_prob: float = 0.05):
        self.tower = TYPES[tower_id]
        self.api_base = api_base.rstrip("/")
        self.interval = interval
        self.anomaly_prob = anomaly_prob
        self.storm_prob = storm_prob
        self.wind_history: List[float] = [8.0] * 5
        self.tilt_drift = 0.0
        self.settlement_cumulative = 0.0

    def simulate_wind_speed(self, base_wind: float = 10.0) -> float:
        if random.random() < self.storm_prob:
            storm_factor = random.uniform(2.0, 4.0)
            return min(base_wind * storm_factor, 60.0)

        self.wind_history.append(base_wind + random.gauss(0, 3.0))
        if len(self.wind_history) > 10:
            self.wind_history.pop(0)
        smoothed = sum(self.wind_history) / len(self.wind_history)
        return max(0.0, smoothed + random.gauss(0, 1.5))

    def simulate_layer_stresses(self, layer_id: int, total_layers: int,
                                 wind_speed: float, anomaly: bool = False) -> Dict[str, float]:
        h = layer_id / total_layers
        q = 0.5 * AIR_DENSITY * WIND_DRAG_COEFFICIENT * wind_speed ** 2

        base_stress = 2.0 + h * 22.0
        wind_stress = q / 1000.0 * (1.0 + h * 0.5) * 15.0

        noise_factor = random.gauss(1.0, 0.05)
        sx = (base_stress + wind_stress) * noise_factor
        sy = (base_stress * 0.75 + wind_stress * 0.6) * noise_factor
        sz = (self.tower["total_weight"] * GRAVITY /
              (self.tower["base_width"] * self.tower["base_depth"])) * (1.0 + h * 0.2)
        sz *= random.gauss(1.0, 0.03)

        if anomaly:
            sx *= random.uniform(1.3, 2.0)
            sy *= random.uniform(1.2, 1.8)
            sz *= random.uniform(1.2, 1.6)

        j2 = 0.5 * ((sx - sy) ** 2 + (sy - sz) ** 2 + (sz - sx) ** 2)
        von_mises = math.sqrt(3.0 * j2)

        return {
            "stress_x": round(sx, 4),
            "stress_y": round(sy, 4),
            "stress_z": round(sz, 4),
            "stress_von_mises": round(von_mises, 4),
        }

    def simulate_tilt(self, layer_id: int, total_layers: int,
                      wind_speed: float, anomaly: bool = False) -> Dict[str, float]:
        h = layer_id / total_layers
        wind_effect = wind_speed / self.tower["design_wind_speed"]

        self.tilt_drift += random.gauss(0, 0.0005)
        self.tilt_drift = max(-2.0, min(2.0, self.tilt_drift))

        base_tilt = 0.2 + wind_effect * 2.0 + abs(self.tilt_drift)
        tilt_x = base_tilt * (0.4 + h * 0.8) + random.gauss(0, 0.05)
        tilt_y = base_tilt * (0.25 + h * 0.5) + random.gauss(0, 0.04)

        if anomaly:
            tilt_x *= random.uniform(1.5, 2.5)
            tilt_y *= random.uniform(1.3, 2.0)

        tilt_total = math.sqrt(tilt_x ** 2 + tilt_y ** 2)

        return {
            "tilt_x": round(tilt_x, 4),
            "tilt_y": round(tilt_y, 4),
            "tilt_total": round(tilt_total, 4),
        }

    def simulate_wind_load(self, layer_id: int, total_layers: int,
                            wind_speed: float) -> Dict[str, float]:
        h = layer_id / total_layers
        q = 0.5 * AIR_DENSITY * WIND_DRAG_COEFFICIENT * wind_speed ** 2
        return {
            "wind_load_x": round(q * (1.0 + h * 0.4), 4),
            "wind_load_y": round(q * 0.35 * (1.0 + h * 0.2), 4),
        }

    def simulate_environment(self, wind_speed: float) -> Dict[str, Any]:
        soil = random.choices(
            SOIL_TYPES, weights=[0.15, 0.2, 0.15, 0.1, 0.4]
        )[0]

        soil_capacities = {
            "sand": 180.0, "clay": 120.0, "silt": 90.0, "rock": 800.0, "loam": 200.0
        }
        capacity = soil_capacities[soil]

        base_pressure = (self.tower["total_weight"] * GRAVITY /
                         (self.tower["base_width"] * self.tower["base_depth"]))
        wind_effect_pressure = 0.5 * 1.225 * 1.3 * wind_speed ** 2 / 1000.0 * 10.0
        applied = base_pressure + wind_effect_pressure
        applied *= random.gauss(1.05, 0.08)

        self.settlement_cumulative += random.uniform(0.01, 0.08)
        settlement = self.settlement_cumulative + random.uniform(0.5, 2.0)

        return {
            "wind_speed": round(wind_speed, 4),
            "ground_pressure": round(applied, 4),
            "ground_settlement": round(settlement, 4),
            "soil_type": soil,
            "temperature": round(15.0 + 15.0 * math.sin(time.time() / 86400 * 2 * math.pi)
                                 + random.gauss(0, 2.0), 2),
            "humidity": round(50.0 + 30.0 * math.sin(time.time() / 86400 * 2 * math.pi + 2.0)
                              + random.gauss(0, 5.0), 2),
            "vibration_freq": round(2.0 + wind_speed * 0.05 + random.gauss(0, 0.2), 4),
            "vibration_amp": round(0.2 + wind_speed * 0.03 + random.gauss(0, 0.05), 4),
        }

    def generate_batch(self) -> Dict[str, Any]:
        total_layers = self.tower["total_layers"]
        base_wind = random.uniform(5.0, 18.0)
        wind_speed = self.simulate_wind_speed(base_wind)

        anomaly = random.random() < self.anomaly_prob
        if anomaly:
            print(f"  ⚠️  生成异常数据场景 (塔号: {self.tower['tower_id']})")

        layers = []
        for layer_id in range(1, total_layers + 1):
            stresses = self.simulate_layer_stresses(layer_id, total_layers, wind_speed, anomaly)
            tilts = self.simulate_tilt(layer_id, total_layers, wind_speed, anomaly)
            wind_loads = self.simulate_wind_load(layer_id, total_layers, wind_speed)

            layers.append({
                "layer_id": layer_id,
                "layer_name": f"第{layer_id}层",
                **stresses,
                **tilts,
                **wind_loads,
            })

        environment = self.simulate_environment(wind_speed)

        return {
            "tower_id": self.tower["tower_id"],
            "tower_name": self.tower["tower_name"],
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "layers": layers,
            "environment": environment,
        }

    def send_batch(self, batch: Dict[str, Any]) -> bool:
        try:
            url = f"{self.api_base}/api/sensor"
            headers = {"Content-Type": "application/json"}
            response = requests.post(url, data=json.dumps(batch, ensure_ascii=False),
                                     headers=headers, timeout=10)
            if response.status_code == 200:
                data = response.json()
                status = "✓" if data.get("code") == 200 else "?"
                alerts_count = (data.get("data") or {}).get("alerts_count", 0)
                analysis = (data.get("data") or {}).get("analysis", {})
                sf = analysis.get("safety_factor", 0)
                stable = "稳定" if analysis.get("is_stable") == 1 else "不稳定"
                wind_spd = batch["environment"]["wind_speed"]
                print(
                    f"  {status} 塔{self.tower['tower_id']} | "
                    f"风速={wind_spd:.1f}m/s | 安全系数={sf:.2f} | "
                    f"状态={stable} | 告警={alerts_count}"
                )
                return True
            else:
                print(f"  ✗ HTTP {response.status_code}: {response.text[:200]}")
                return False
        except Exception as e:
            print(f"  ✗ 发送失败: {e}")
            return False

    def run(self, max_iterations: int = 0):
        print(f"\n=== 启动传感器模拟器 ===")
        print(f"  攻城塔: {self.tower['tower_name']} (#{self.tower['tower_id']})")
        print(f"  高度: {self.tower['total_height']}m | 层数: {self.tower['total_layers']}层")
        print(f"  材质: {self.tower['material']}")
        print(f"  API: {self.api_base}")
        print(f"  上报间隔: {self.interval}秒")
        print(f"  异常概率: {self.anomaly_prob * 100:.0f}% | 暴风概率: {self.storm_prob * 100:.0f}%")
        print(f"  {'=' * 50}\n")

        iteration = 0
        try:
            while True:
                iteration += 1
                if max_iterations > 0 and iteration > max_iterations:
                    print(f"\n达到最大迭代次数 {max_iterations}，退出")
                    break

                now = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
                print(f"[{now}] 第 {iteration} 次上报:")

                batch = self.generate_batch()
                self.send_batch(batch)

                if max_iterations <= 0 or iteration < max_iterations:
                    print(f"  等待 {self.interval}s 后下一次上报...\n")
                    time.sleep(self.interval)

        except KeyboardInterrupt:
            print(f"\n\n用户中断，模拟器退出。共上报 {iteration} 次数据")


def main():
    parser = argparse.ArgumentParser(description="临冲吕公车传感器模拟器")
    parser.add_argument("--tower", type=int, choices=[1, 2], default=1, help="塔号 (1或2)")
    parser.add_argument("--all", action="store_true", help="同时模拟所有塔")
    parser.add_argument("--api", type=str, default="http://localhost:8080", help="后端API地址")
    parser.add_argument("--interval", type=int, default=60, help="上报间隔秒数")
    parser.add_argument("--iterations", type=int, default=0, help="最大迭代次数，0为无限")
    parser.add_argument("--anomaly", type=float, default=0.1, help="异常数据概率 (0-1)")
    parser.add_argument("--storm", type=float, default=0.05, help="暴风场景概率 (0-1)")
    parser.add_argument("--once", action="store_true", help="仅发送一次后退出")
    parser.add_argument("--test", action="store_true", help="测试模式（快速循环，5秒间隔）")

    args = parser.parse_args()

    if args.test:
        args.interval = 5
        args.anomaly = 0.3
        args.storm = 0.2

    if args.once:
        args.iterations = 1

    if args.all:
        import threading
        sims = []
        for tid in [1, 2]:
            sim = SensorSimulator(tid, args.api, args.interval, args.anomaly, args.storm)
            t = threading.Thread(target=sim.run, args=(args.iterations,), daemon=True)
            t.start()
            sims.append(t)
            time.sleep(1)
        try:
            for t in sims:
                t.join()
        except KeyboardInterrupt:
            pass
    else:
        sim = SensorSimulator(args.tower, args.api, args.interval, args.anomaly, args.storm)
        sim.run(args.iterations)


if __name__ == "__main__":
    main()
