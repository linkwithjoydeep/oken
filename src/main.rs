mod cli;
#[allow(dead_code)]
mod config;
#[allow(dead_code)]
mod history;
#[allow(dead_code)]
mod hosts;
#[allow(dead_code)]
mod hosts_toml;
mod picker;
mod ssh;
#[allow(dead_code)]
mod ssh_config;

use std::env;
use std::io::{self, BufRead, Write};

use anyhow::Result;
use clap::Parser;

use cli::{Cli, Command, HostCommand};

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 && !is_known_subcommand(&args[1]) && !is_oken_flag(&args[1]) {
        // Single bare arg that doesn't look like a direct SSH target — maybe a partial filter
        if args.len() == 2 && !args[1].contains('@') && !args[1].starts_with('-') {
            let all_hosts = hosts::list_all_hosts().unwrap_or_default();
            let query = &args[1];
            let exact = all_hosts.iter().find(|h| h.alias == *query);
            let has_other_matches = all_hosts
                .iter()
                .any(|h| h.alias != *query && h.alias.contains(query.as_str()));

            if exact.is_some() && !has_other_matches {
                // Only one match — connect directly
                let host = exact.unwrap();
                let ssh_args = build_ssh_args(host);
                record_host(host);
                print_connecting(&ssh_args);
                return ssh::passthrough(&ssh_args);
            } else {
                // Multiple matches or no exact match → open picker pre-filtered
                match picker::run_picker(Some(query)) {
                    Ok(host) => {
                        let ssh_args = build_ssh_args(&host);
                        record_host(&host);
                        print_connecting(&ssh_args);
                        return ssh::passthrough(&ssh_args);
                    }
                    Err(_) => std::process::exit(0),
                }
            }
        }
        // Multi-arg → passthrough as-is (user typed real SSH args)
        maybe_prompt_save(&args[1..]);
        record_if_connecting(&args[1..]);
        print_connecting(&args[1..]);
        return ssh::passthrough(&args[1..]);
    }

    let cli = Cli::parse();

    match cli.command {
        Some(cmd) => run_subcommand(cmd),
        None => {
            if cli.ssh_args.is_empty() {
                // No args → open picker
                match picker::run_picker(None) {
                    Ok(host) => {
                        let ssh_args = build_ssh_args(&host);
                        record_host(&host);
                        print_connecting(&ssh_args);
                        ssh::passthrough(&ssh_args)
                    }
                    Err(_) => Ok(()), // user cancelled, exit cleanly
                }
            } else {
                maybe_prompt_save(&cli.ssh_args);
                record_if_connecting(&cli.ssh_args);
                print_connecting(&cli.ssh_args);
                ssh::passthrough(&cli.ssh_args)
            }
        }
    }
}

/// Build SSH args from a picker-selected host.
fn build_ssh_args(host: &hosts::Host) -> Vec<String> {
    let mut args = Vec::new();

    match (&host.user, &host.hostname) {
        (Some(user), Some(hostname)) => args.push(format!("{}@{}", user, hostname)),
        (None, Some(hostname)) => args.push(hostname.clone()),
        // ssh_config-only host (no hostname stored) — use alias and let SSH resolve it
        _ => args.push(host.alias.clone()),
    }

    if let Some(port) = host.port {
        args.push("-p".to_string());
        args.push(port.to_string());
    }
    if let Some(ref identity) = host.identity_file {
        args.push("-i".to_string());
        args.push(identity.clone());
    }

    args
}

/// Print a "Connecting to ..." message on stderr before exec-ing into SSH.
/// Uses \r so SSH's output overwrites it naturally.
fn print_connecting(args: &[String]) {
    if let Some(target) = ssh::extract_target_host(args) {
        eprint!("\x1b[2m→ Connecting to {target}…\x1b[0m\r");
        let _ = io::stderr().flush();
    }
}

/// Record a picker-selected host to history using its alias.
/// Silently ignores all errors — history must never block SSH.
fn record_host(host: &hosts::Host) {
    let _ = history::record_connection(
        &host.alias,
        host.hostname.as_deref(),
        host.user.as_deref(),
        host.port,
    );
}

/// Extract the target host from SSH args and record to history DB.
/// Silently ignores all errors — history must never block SSH.
fn record_if_connecting(args: &[String]) {
    if let Some(host) = ssh::extract_target_host(args) {
        let _ = history::record_connection(&host, None, None, None);
    }
}

/// Prompt to save an unknown host on first connect.
/// Best-effort: any I/O or save error is silently ignored.
fn maybe_prompt_save(args: &[String]) {
    let _ = (|| -> Option<()> {
        let target = ssh::extract_target_host_full(args)?;

        // Only prompt for user@host targets
        if !target.contains('@') {
            return None;
        }

        let (user, hostname) = target.split_once('@')?;

        // Check if already known (must match both user AND hostname)
        let all_hosts = hosts::list_all_hosts().unwrap_or_default();
        let host_known = all_hosts.iter().any(|h| {
            h.alias == hostname || h.hostname.as_deref() == Some(hostname)
        });
        let exact_known = all_hosts.iter().any(|h| {
            let host_matches = h.alias == hostname
                || h.hostname.as_deref() == Some(hostname);
            let user_matches = h.user.as_deref() == Some(user);
            (host_matches && user_matches) || h.alias == target
        });
        if exact_known {
            return None;
        }

        // Show a contextual hint and prompt depending on scenario
        let stdin = io::stdin();
        let alias;

        if host_known {
            // Known host, new user — no sensible default, require a name
            eprintln!(
                "\x1b[2mNew user \x1b[0m\x1b[1m{user}\x1b[0m\x1b[2m for known host \x1b[0m\x1b[1m{hostname}\x1b[0m\x1b[2m — save it so you can pick it next time?\x1b[0m",
            );
            eprint!("\x1b[2mSave as (Enter to skip):\x1b[0m ");
            io::stderr().flush().ok()?;

            let line = stdin.lock().lines().next()?.ok()?;
            let input = line.trim().to_string();
            if input.is_empty() {
                return None;
            }
            alias = input;
        } else {
            // Completely new host — default alias is the hostname
            eprintln!(
                "\x1b[2mLooks like a new host. Save \x1b[0m\x1b[1m{target}\x1b[0m\x1b[2m so it shows up in the picker?\x1b[0m",
            );
            eprint!("\x1b[2mSave as (Enter = \x1b[0m{hostname}\x1b[2m, \"n\" to skip):\x1b[0m ");
            io::stderr().flush().ok()?;

            let line = stdin.lock().lines().next()?.ok()?;
            let input = line.trim().to_string();
            if input.eq_ignore_ascii_case("n") || input.eq_ignore_ascii_case("no") {
                return None;
            }
            alias = if input.is_empty() {
                hostname.to_string()
            } else {
                input
            };
        }

        // Prompt for tags
        eprint!("Tags (comma-separated, Enter to skip): ");
        io::stderr().flush().ok()?;

        let tag_line = stdin.lock().lines().next()?.ok()?;
        let tags: Vec<String> = tag_line
            .split(',')
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect();

        // Build entry and save
        let port = ssh::extract_port(args);
        let identity_file = ssh::extract_identity_file(args);
        let entry = hosts_toml::HostEntry {
            hostname: hostname.to_string(),
            user: Some(user.to_string()),
            port,
            identity_file,
            tags,
        };

        let path = hosts_toml_path().ok()?;
        match hosts_toml::add_host(&path, &alias, entry) {
            Ok(()) => eprintln!("Saved host '{alias}'"),
            Err(e) => eprintln!("Warning: could not save host: {e}"),
        }

        Some(())
    })();
}

fn is_known_subcommand(arg: &str) -> bool {
    matches!(
        arg,
        "host"
            | "tunnel"
            | "exec"
            | "snippet"
            | "print"
            | "audit"
            | "keys"
            | "export"
            | "import"
            | "completions"
            | "help"
    )
}

fn is_oken_flag(arg: &str) -> bool {
    matches!(arg, "--help" | "-h" | "--version" | "-V")
}

fn run_subcommand(cmd: Command) -> Result<()> {
    match cmd {
        Command::Host { command } => run_host_command(command),
        other => {
            let name = match other {
                Command::Host { .. } => unreachable!(),
                Command::Tunnel { .. } => "tunnel",
                Command::Exec { .. } => "exec",
                Command::Snippet { .. } => "snippet",
                Command::Print { .. } => "print",
                Command::Audit { .. } => "audit",
                Command::Keys { .. } => "keys",
                Command::Export { .. } => "export",
                Command::Import { .. } => "import",
                Command::Completions { .. } => "completions",
            };
            eprintln!("oken: `{name}` is not yet implemented");
            std::process::exit(1);
        }
    }
}

fn hosts_toml_path() -> Result<std::path::PathBuf> {
    Ok(config::config_dir()?.join("hosts.toml"))
}

fn run_host_command(cmd: HostCommand) -> Result<()> {
    match cmd {
        HostCommand::Add {
            name,
            target,
            port,
            key,
            tag,
        } => {
            let (user, hostname) = if let Some((u, h)) = target.split_once('@') {
                (Some(u.to_string()), h.to_string())
            } else {
                (None, target)
            };

            let entry = hosts_toml::HostEntry {
                hostname,
                user,
                port,
                identity_file: key.map(|p| p.to_string_lossy().to_string()),
                tags: tag,
            };

            let path = hosts_toml_path()?;
            hosts_toml::add_host(&path, &name, entry)?;
            println!("Added host '{name}'");
            Ok(())
        }

        HostCommand::List => {
            let path = hosts_toml_path()?;
            let hosts = hosts_toml::load_hosts_toml(&path)?;

            if hosts.is_empty() {
                println!("No hosts configured. Use `oken host add` to add one.");
                return Ok(());
            }

            // Collect and sort by name for stable output
            let mut entries: Vec<_> = hosts.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));

            // Calculate column widths
            let name_w = entries.iter().map(|(n, _)| n.len()).max().unwrap_or(4).max(4);
            let target_w = entries
                .iter()
                .map(|(_, e)| {
                    match &e.user {
                        Some(u) => u.len() + 1 + e.hostname.len(),
                        None => e.hostname.len(),
                    }
                })
                .max()
                .unwrap_or(6)
                .max(6);

            println!(
                "{:<name_w$}  {:<target_w$}  {:>5}  {}",
                "NAME", "TARGET", "PORT", "TAGS"
            );
            for (name, entry) in &entries {
                let target = match &entry.user {
                    Some(u) => format!("{}@{}", u, entry.hostname),
                    None => entry.hostname.clone(),
                };
                let port = entry
                    .port
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "-".to_string());
                let tags = if entry.tags.is_empty() {
                    "-".to_string()
                } else {
                    entry.tags.join(", ")
                };
                println!(
                    "{:<name_w$}  {:<target_w$}  {:>5}  {}",
                    name, target, port, tags
                );
            }
            Ok(())
        }

        HostCommand::Remove { name } => {
            let path = hosts_toml_path()?;
            hosts_toml::remove_host(&path, &name)?;
            println!("Removed host '{name}'");
            Ok(())
        }

        HostCommand::Edit { .. } => {
            let path = hosts_toml_path()?;
            let editor = env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
            let status = std::process::Command::new(&editor)
                .arg(&path)
                .status()?;
            if !status.success() {
                anyhow::bail!("editor exited with status {}", status);
            }
            Ok(())
        }
    }
}
