use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;

use crate::time_utils;

/// Append a completed session entry to the audit log. Silently ignores errors.
pub fn log_session(alias: &str, target: &str, duration_secs: u64, exit_code: i32) {
    let _ = log_impl(alias, target, duration_secs, exit_code);
}

fn log_impl(alias: &str, target: &str, duration_secs: u64, exit_code: i32) -> Result<()> {
    let path = crate::config::data_dir()?.join("audit.log");
    let ts = current_timestamp();
    // Format: timestamp \t alias \t target \t duration_secs \t exit_code
    let line = format!("{ts}\t{alias}\t{target}\t{duration_secs}\t{exit_code}\n");
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

    // Column widths
    let alias_w = recent
        .iter()
        .filter_map(|l| l.splitn(5, '\t').nth(1))
        .map(|s| s.len())
        .max()
        .unwrap_or(5)
        .max(5);
    let target_w = recent
        .iter()
        .filter_map(|l| l.splitn(5, '\t').nth(2))
        .map(|s| s.len())
        .max()
        .unwrap_or(6)
        .max(6);

    println!(
        "{:<19}  {:<alias_w$}  {:<target_w$}  {:>8}  {}",
        "TIME", "ALIAS", "TARGET", "DURATION", "EXIT"
    );

    for line in &recent {
        let parts: Vec<&str> = line.splitn(5, '\t').collect();
        let ts = parts.first().copied().unwrap_or("");
        let alias = parts.get(1).copied().unwrap_or("");
        let target = parts.get(2).copied().unwrap_or("");
        let duration = parts.get(3).copied().unwrap_or("").parse::<u64>().ok();
        let exit_code = parts.get(4).copied().unwrap_or("").parse::<i32>().ok();

        let display_ts = ts.replace('T', " ").trim_end_matches('Z').to_string();
        let display_dur = duration.map(format_duration).unwrap_or_else(|| "-".into());
        let display_exit = exit_code
            .map(|c| c.to_string())
            .unwrap_or_else(|| "-".into());

        println!(
            "{:<19}  {:<alias_w$}  {:<target_w$}  {:>8}  {}",
            display_ts, alias, target, display_dur, display_exit
        );
    }

    Ok(())
}

fn format_duration(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m {:02}s", secs / 60, secs % 60)
    } else {
        format!("{}h {:02}m", secs / 3600, (secs % 3600) / 60)
    }
}

fn current_timestamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    time_utils::unix_to_iso8601(secs)
}
