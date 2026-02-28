use std::env;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};

/// Find the system `ssh` binary, skipping our own binary if oken is aliased as `ssh`.
pub(crate) fn find_ssh() -> Result<PathBuf> {
    let our_exe = env::current_exe().ok();

    // Search PATH for `ssh`, skipping any entry that resolves to our own binary
    if let Ok(path_var) = env::var("PATH") {
        for dir in env::split_paths(&path_var) {
            let candidate = dir.join("ssh");
            if candidate.is_file() {
                // Skip if this is actually us (oken aliased as ssh)
                if let Some(ref ours) = our_exe {
                    if same_file(&candidate, ours) {
                        continue;
                    }
                }
                return Ok(candidate);
            }
        }
    }

    // Fallback to well-known paths
    for path in ["/usr/bin/ssh", "/usr/local/bin/ssh"] {
        let p = PathBuf::from(path);
        if p.is_file() {
            if let Some(ref ours) = our_exe {
                if same_file(&p, ours) {
                    continue;
                }
            }
            return Ok(p);
        }
    }

    bail!("could not find ssh binary on PATH")
}

/// Check if two paths refer to the same file (following symlinks).
fn same_file(a: &PathBuf, b: &PathBuf) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// SSH flags that take a following argument value.
pub(crate) const FLAGS_WITH_VALUES: &[&str] = &[
    "-p", "-i", "-o", "-l", "-L", "-R", "-D", "-F", "-J", "-W", "-b", "-c", "-m", "-e", "-S", "-E",
    "-B", "-w", "-O",
];

/// Extract the target host from SSH arguments.
/// Skips flags (and their values) to find the first positional argument.
pub fn extract_target_host(args: &[String]) -> Option<String> {
    let mut skip_next = false;
    for arg in args {
        if skip_next {
            skip_next = false;
            continue;
        }
        if FLAGS_WITH_VALUES.contains(&arg.as_str()) {
            skip_next = true;
            continue;
        }
        // Skip flags like -v, -N, -T, etc.
        if arg.starts_with('-') {
            continue;
        }
        // First positional argument is the target (possibly user@host)
        let host = if let Some((_user, host)) = arg.split_once('@') {
            host
        } else {
            arg.as_str()
        };
        return Some(host.to_string());
    }
    None
}

/// Like `extract_target_host()` but returns the full `user@host` string
/// instead of stripping the user part.
pub fn extract_target_host_full(args: &[String]) -> Option<String> {
    let mut skip_next = false;
    for arg in args {
        if skip_next {
            skip_next = false;
            continue;
        }
        if FLAGS_WITH_VALUES.contains(&arg.as_str()) {
            skip_next = true;
            continue;
        }
        if arg.starts_with('-') {
            continue;
        }
        return Some(arg.clone());
    }
    None
}

/// Extract the port from SSH arguments (scans for `-p <port>`).
pub fn extract_port(args: &[String]) -> Option<u16> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "-p" {
            return iter.next().and_then(|v| v.parse().ok());
        }
    }
    None
}

/// Extract the identity file from SSH arguments (scans for `-i <path>`).
pub fn extract_identity_file(args: &[String]) -> Option<String> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "-i" {
            return iter.next().cloned();
        }
    }
    None
}

/// Replace the current process with `ssh`, passing through all arguments.
/// On Unix this uses exec() so signals, TTY, and exit codes work perfectly.
pub fn passthrough(args: &[String]) -> Result<()> {
    let ssh = find_ssh().context("failed to locate ssh")?;

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = std::process::Command::new(&ssh).args(args).exec();
        // exec() only returns on error
        bail!("failed to exec ssh at {}: {}", ssh.display(), err);
    }

    #[cfg(not(unix))]
    {
        let status = std::process::Command::new(&ssh)
            .args(args)
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status()
            .context(format!("failed to run ssh at {}", ssh.display()))?;
        std::process::exit(status.code().unwrap_or(1));
    }
}
