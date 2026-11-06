"use client";

import { useSimulation } from "@/features/simulation/SimulationProvider";
import { frontendConfig } from "@/config/frontend";

export function SimulationControls() {
  const sim = useSimulation();
  if (frontendConfig.dataMode !== "mock") return null;
  return (
    <div className="panel sim-bar" aria-label="Simulation controls">
      <span className="metric-label">SIMULATION</span>
      <span className={`badge ${sim.running ? "ok" : "warn"}`}>
        {sim.running ? "RUNNING" : "PAUSED"}
      </span>
      <button type="button" className="btn" onClick={sim.pause}>
        Pause
      </button>
      <button type="button" className="btn" onClick={sim.resume}>
        Resume
      </button>
      <button type="button" className="btn" onClick={sim.reset}>
        Reset
      </button>
      <button type="button" className="btn primary" onClick={sim.generateSignal}>
        Generate Signal
      </button>
      <button type="button" className="btn primary" onClick={sim.generateTrade}>
        Generate Trade
      </button>
      <span className="metric-sub muted">
        Mock-only — never sent to Rust backend
      </span>
    </div>
  );
}
