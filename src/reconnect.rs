use std::process::Stdio;
use std::thread;
use std::time::Duration;

use anyhow::Result;

/// Run SSH with automatic reconnect on connection loss (exit code 255).
/// Retries up to `max_retries` times with `delay_secs` between attempts.
/// This function never returns `Ok(())` — it always exits via `std::process::exit`.
pub fn run_with_reconnect(args: &[String], max_retries: u32, delay_secs: u64) -> Result<()> {
    let ssh = crate::ssh::find_ssh()?;

    let mut attempt = 0u32;
    loop {
        let status = std::process::Command::new(&ssh)
            .args(args)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()?;

        let code = status.code().unwrap_or(1);

        if code == 255 && attempt < max_retries {
            attempt += 1;
            eprintln!(
                "\x1b[2mConnection lost. Reconnecting ({attempt}/{max_retries})…\x1b[0m"
            );
            thread::sleep(Duration::from_secs(delay_secs));
            continue;
        }

        std::process::exit(code);
    }
}
