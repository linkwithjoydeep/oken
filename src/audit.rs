use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;

/// Append a connection entry to the audit log. Silently ignores errors.
pub fn log_connection(alias: &str, target: &str) {
    let _ = log_impl(alias, target);
}

fn log_impl(alias: &str, target: &str) -> Result<()> {
    let path = crate::config::data_dir()?.join("audit.log");
    let ts = current_timestamp();
    let line = format!("{ts}\t{alias}\t{target}\n");
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    file.write_all(line.as_bytes())?;
    Ok(())
}

/// Display the last `n` audit log entries.
pub fn show_recent(n: usize) -> Result<()> {
    let path = crate::config::data_dir()?.join("audit.log");
    if !path.exists() {
        println!("No audit log found. Connect to some hosts first.");
        return Ok(());
    }

    let content = std::fs::read_to_string(&path)?;
    let all_lines: Vec<&str> = content.lines().collect();
    if all_lines.is_empty() {
        println!("No connections recorded.");
        return Ok(());
    }

    let start = all_lines.len().saturating_sub(n);
    let recent: Vec<&str> = all_lines[start..].iter().rev().cloned().collect();

    let ts_w = 19; // "2026-02-28 10:00:00"
    let alias_w = recent
        .iter()
        .filter_map(|l| l.splitn(3, '\t').nth(1))
        .map(|s| s.len())
        .max()
        .unwrap_or(5)
        .max(5);

    println!("{:<ts_w$}  {:<alias_w$}  {}", "TIME", "ALIAS", "TARGET");
    for line in &recent {
        let parts: Vec<&str> = line.splitn(3, '\t').collect();
        let (ts, alias, target) = match parts.as_slice() {
            [ts, alias, target] => (*ts, *alias, *target),
            [ts, alias] => (*ts, *alias, ""),
            _ => continue,
        };
        let display_ts = ts.replace('T', " ").trim_end_matches('Z').to_string();
        println!("{:<ts_w$}  {:<alias_w$}  {}", display_ts, alias, target);
    }

    Ok(())
}

fn current_timestamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    unix_to_iso8601(secs)
}

fn unix_to_iso8601(secs: u64) -> String {
    let days = secs / 86400;
    let tod = secs % 86400;
    let h = tod / 3600;
    let m = (tod % 3600) / 60;
    let s = tod % 60;
    let (y, mo, d) = civil_from_days(days as i64);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

/// Convert days since Unix epoch to (year, month, day).
/// Uses the algorithm from https://howardhinnant.github.io/date_algorithms.html
fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
}
