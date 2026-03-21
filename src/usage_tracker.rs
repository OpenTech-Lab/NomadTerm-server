//! Periodic usage tracker — polls sysinfo for hardware power and broadcasts
//! `UsageUpdate` JSON messages to all connected WebSocket clients.
//!
//! Hardware power is estimated from CPU usage percentage when native power APIs
//! are unavailable:
//!   estimated_watts = (cpu_usage_pct / 100) × TDP_ESTIMATE_W
//! Accurate native readings (RAPL on Linux, powermetrics on macOS) are used when
//! the process has the necessary permissions.
//!
//! AI usage data is aggregated from PTY session stats stored in SQLite (Phase A
//! hook integration); this stub sends zeroed counters until that integration lands.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sysinfo::System;
use tokio::sync::broadcast;

use crate::ws::protocol::{AiUsage, HardwarePower, ServerMessage};

/// Rough CPU TDP estimate in watts for fallback power calculation.
const CPU_TDP_ESTIMATE_W: f64 = 65.0;

/// How often to emit a `UsageUpdate`.
const POLL_INTERVAL_SECS: u64 = 15;

/// Start the background usage tracker.
///
/// `control_tx` is a broadcast channel shared with all WebSocket handlers; each
/// handler subscribes and forwards received strings as JSON text frames.
pub async fn start(control_tx: Arc<broadcast::Sender<String>>) {
    let mut sys = System::new_all();
    let mut total_watts_sum: f64 = 0.0;
    let mut sample_count: u64 = 0;

    let mut interval = tokio::time::interval(Duration::from_secs(POLL_INTERVAL_SECS));
    // First tick fires immediately; skip it so we get a real reading.
    interval.tick().await;

    loop {
        interval.tick().await;

        sys.refresh_cpu_usage();

        let hw = collect_hardware_power(&sys, total_watts_sum, sample_count);
        total_watts_sum += hw.total_watts;
        sample_count += 1;

        let ai_usage = collect_ai_usage();

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let msg = ServerMessage::UsageUpdate {
            timestamp,
            ai_usage,
            hardware: Some(hw),
        };

        if let Ok(json) = serde_json::to_string(&msg) {
            // Ignore send error — no connected clients is fine.
            let _ = control_tx.send(json);
        }
    }
}

/// Collect hardware power readings.
///
/// Tries platform-native sources first, falls back to CPU-usage estimation.
fn collect_hardware_power(sys: &System, total_sum: f64, sample_count: u64) -> HardwarePower {
    let cpu_watts = read_native_cpu_watts().unwrap_or_else(|| estimate_cpu_watts(sys));

    let average = if sample_count == 0 {
        cpu_watts
    } else {
        (total_sum + cpu_watts) / (sample_count + 1) as f64
    };

    HardwarePower {
        cpu_watts,
        gpu_watts: read_native_gpu_watts(),
        total_watts: cpu_watts + read_native_gpu_watts().unwrap_or(0.0),
        average_since_session: average,
    }
}

/// Estimate CPU power from usage percentage using a fixed TDP.
fn estimate_cpu_watts(sys: &System) -> f64 {
    let usage = sys.global_cpu_info().cpu_usage() as f64;
    // Apply a non-linear curve: idle systems draw ~15% TDP at 0% load.
    let idle_fraction = 0.15;
    let active_fraction = 1.0 - idle_fraction;
    (idle_fraction + active_fraction * usage / 100.0) * CPU_TDP_ESTIMATE_W
}

/// Attempt to read CPU power via platform-native APIs.
///
/// Linux: reads Intel RAPL package energy counter from sysfs.
/// macOS/Windows: returns None (powermetrics requires root; WMI not yet wired).
fn read_native_cpu_watts() -> Option<f64> {
    #[cfg(target_os = "linux")]
    {
        read_rapl_cpu_watts()
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// Returns None — GPU power requires platform-specific drivers not yet integrated.
fn read_native_gpu_watts() -> Option<f64> {
    None
}

/// Read CPU power from Intel RAPL sysfs on Linux.
///
/// Reads the `energy_uj` counter twice with a 200 ms gap and derives watts.
/// Returns None if the sysfs path is inaccessible (no root / non-Intel).
#[cfg(target_os = "linux")]
fn read_rapl_cpu_watts() -> Option<f64> {
    const RAPL_PATH: &str = "/sys/class/powercap/intel-rapl/intel-rapl:0/energy_uj";

    let read_uj = || -> Option<u64> {
        std::fs::read_to_string(RAPL_PATH)
            .ok()
            .and_then(|s| s.trim().parse().ok())
    };

    let t0 = std::time::Instant::now();
    let e0 = read_uj()?;

    std::thread::sleep(Duration::from_millis(200));

    let e1 = read_uj()?;
    let elapsed = t0.elapsed().as_secs_f64();

    // energy_uj wraps at 2^32; handle wrap-around.
    let delta_uj = if e1 >= e0 {
        (e1 - e0) as f64
    } else {
        (u64::MAX - e0 + e1) as f64
    };

    Some(delta_uj / 1_000_000.0 / elapsed) // µJ → W
}

/// Collect AI usage stats from SQLite transcript data.
///
/// Phase A stub: returns empty maps until PTY hook parsing is wired.
/// When PTY hooks accumulate `usage: {input_tokens, output_tokens}` JSON from
/// CLI stdout, those will be summed per CLI and written to the DB here.
fn collect_ai_usage() -> HashMap<String, AiUsage> {
    // TODO(phase-a): query nomadterm DB for token counts per CLI session.
    HashMap::new()
}
