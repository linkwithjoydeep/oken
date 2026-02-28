use std::thread;
use std::time::Duration;

use anyhow::Result;

/// Run SSH with automatic reconnect on connection loss (exit code 255).
/// Retries up to `max_retries` times with `delay_secs` between attempts.
/// Returns the final exit code so the caller can log it and exit cleanly.
pub fn run_with_reconnect(args: &[String], max_retries: u32, delay_secs: u64) -> Result<i32> {
    let mut attempt = 0u32;
    loop {
        let code = crate::ssh::run(args)?;

        if code == 255 && attempt < max_retries {
            attempt += 1;
            eprintln!(
                "\x1b[2mConnection lost. Reconnecting ({attempt}/{max_retries})…\x1b[0m"
            );
            thread::sleep(Duration::from_secs(delay_secs));
            continue;
        }

        return Ok(code);
    }
}
