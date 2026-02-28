mod audit;
mod cli;
mod update_check;
#[allow(dead_code)]
mod config;
#[allow(dead_code)]
mod history;
#[allow(dead_code)]
mod hosts;
#[allow(dead_code)]
mod hosts_toml;
mod oken_config;
mod picker;
mod reconnect;
mod ssh;
#[allow(dead_code)]
mod ssh_config;
mod tunnels;

use std::env;
use std::io::{self, BufRead, Write};

use anyhow::Result;
use clap::Parser;

use clap::CommandFactory;
use clap_complete::generate;

use cli::{Cli, Command, HostCommand, TunnelCommand};

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let cfg = oken_config::load_config();
    update_check::maybe_notify();

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
                let host = exact.unwrap();
                return connect_to_host(host, false, false, &cfg);
            } else {
                match picker::run_picker(Some(query)) {
                    Ok(host) => return connect_to_host(&host, false, false, &cfg),
                    Err(_) => std::process::exit(0),
                }
            }
        }
        // Multi-arg → passthrough as-is (user typed real SSH args)
        return connect_passthrough(&args[1..], false, false, &cfg);
    }

    let cli = Cli::parse();

    match cli.command {
        Some(cmd) => run_subcommand(cmd, &cfg),
        None => {
            // Handle --tag filter
            if let Some(ref tag) = cli.tag {
                let all_hosts = hosts::list_all_hosts().unwrap_or_default();
                let tag_lower = tag.to_lowercase();
                let matches: Vec<_> = all_hosts
                    .iter()
                    .filter(|h| h.tags.iter().any(|t| t.to_lowercase() == tag_lower))
                    .collect();

                return match matches.len() {
                    0 => {
                        eprintln!("oken: no hosts found with tag '{tag}'");
                        std::process::exit(1);
                    }
                    1 => connect_to_host(matches[0], cli.yes, cli.no_reconnect, &cfg),
                    _ => {
                        let initial = format!("#{tag}");
                        match picker::run_picker(Some(&initial)) {
                            Ok(host) => connect_to_host(&host, cli.yes, cli.no_reconnect, &cfg),
                            Err(_) => {
                                std::process::exit(0);
                            }
                        }
                    }
                };
            }

            if cli.ssh_args.is_empty() {
                // No args → open picker
                match picker::run_picker(None) {
                    Ok(host) => connect_to_host(&host, cli.yes, cli.no_reconnect, &cfg),
                    Err(_) => Ok(()), // user cancelled, exit cleanly
                }
            } else {
                connect_passthrough(&cli.ssh_args, cli.yes, cli.no_reconnect, &cfg)
            }
        }
    }
}

/// Connect to a known host with keepalive, prod warning, and optional reconnect.
fn connect_to_host(
    host: &hosts::Host,
    yes: bool,
    no_reconnect: bool,
    cfg: &oken_config::OkenConfig,
) -> Result<()> {
    if !maybe_prod_warning(host, yes, &cfg.danger_tags)? {
        return Ok(());
    }
    let mut ssh_args = build_ssh_args(host);
    let target = ssh_args.first().cloned().unwrap_or_default();
    inject_keepalive(&mut ssh_args, cfg.keepalive_interval);
    record_host(host);
    audit::log_connection(&host.alias, &target);
    print_connecting(&ssh_args);
    if no_reconnect || !cfg.reconnect {
        ssh::passthrough(&ssh_args)
    } else {
        reconnect::run_with_reconnect(&ssh_args, cfg.reconnect_retries, cfg.reconnect_delay_secs)
    }
}

/// Pass raw SSH args through with keepalive injection, prod warning, and optional reconnect.
fn connect_passthrough(
    ssh_args: &[String],
    yes: bool,
    no_reconnect: bool,
    cfg: &oken_config::OkenConfig,
) -> Result<()> {
    maybe_prompt_save(ssh_args);

    // Prod warning: look up target in known hosts
    if !yes {
        if let Some(target) = ssh::extract_target_host(ssh_args) {
            let all = hosts::list_all_hosts().unwrap_or_default();
            if let Some(host) = all
                .iter()
                .find(|h| h.alias == target || h.hostname.as_deref() == Some(target.as_str()))
            {
                if !maybe_prod_warning(host, yes, &cfg.danger_tags)? {
                    return Ok(());
                }
            }
        }
    }

    let mut args = ssh_args.to_vec();
    inject_keepalive(&mut args, cfg.keepalive_interval);
    record_if_connecting(&args);
    if let Some(target) = ssh::extract_target_host_full(ssh_args) {
        audit::log_connection(&target, &target);
    }
    print_connecting(&args);
    if no_reconnect || !cfg.reconnect {
        ssh::passthrough(&args)
    } else {
        reconnect::run_with_reconnect(&args, cfg.reconnect_retries, cfg.reconnect_delay_secs)
    }
}

/// Prepend `-o ServerAliveInterval=N -o ServerAliveCountMax=3` unless already set.
fn inject_keepalive(args: &mut Vec<String>, interval: u32) {
    let already_set = args.iter().any(|a| a.contains("ServerAliveInterval"));
    if !already_set {
        let mut prefix = vec![
            "-o".to_string(),
            format!("ServerAliveInterval={interval}"),
            "-o".to_string(),
            "ServerAliveCountMax=3".to_string(),
        ];
        prefix.append(args);
        *args = prefix;
    }
}

/// Show a warning banner if the host has danger tags. Returns false if the user declines.
fn maybe_prod_warning(host: &hosts::Host, yes: bool, danger_tags: &[String]) -> Result<bool> {
    if yes {
        return Ok(true);
    }
    let danger_matches: Vec<&String> = host
        .tags
        .iter()
        .filter(|t| {
            danger_tags
                .iter()
                .any(|dt| dt.eq_ignore_ascii_case(t.as_str()))
        })
        .collect();

    if danger_matches.is_empty() {
        return Ok(true);
    }

    let tags_str = danger_matches
        .iter()
        .map(|t| t.as_str())
        .collect::<Vec<_>>()
        .join(", ");

    eprintln!(
        "\x1b[1;33m⚠  WARNING:\x1b[0m '{}' is tagged [{}]",
        host.alias, tags_str
    );
    eprint!("Continue? [y/N] ");
    io::stderr().flush()?;

    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;

    Ok(line.trim().eq_ignore_ascii_case("y") || line.trim().eq_ignore_ascii_case("yes"))
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
            | "update"
            | "help"
    )
}

fn is_oken_flag(arg: &str) -> bool {
    matches!(
        arg,
        "--help" | "-h" | "--version" | "-V" | "--tag" | "--yes" | "--no-reconnect"
    )
}

fn run_subcommand(cmd: Command, cfg: &oken_config::OkenConfig) -> Result<()> {
    match cmd {
        Command::Host { command } => run_host_command(command),
        Command::Tunnel { command } => run_tunnel_command(command),
        Command::Print { host } => run_print_command(&host, cfg),
        Command::Exec { .. } => stub("exec"),
        Command::Snippet { .. } => stub("snippet"),
        Command::Audit { lines } => {
            audit::show_recent(lines)?;
            Ok(())
        }
        Command::Keys { .. } => stub("keys"),
        Command::Export { .. } => stub("export"),
        Command::Import { .. } => stub("import"),
        Command::Update => {
            update_check::force_check()?;
            Ok(())
        }
        Command::Completions { shell } => {
            generate(shell, &mut Cli::command(), "oken", &mut std::io::stdout());
            Ok(())
        }
    }
}

fn stub(name: &str) -> Result<()> {
    eprintln!("oken: `{name}` is not yet implemented");
    std::process::exit(1);
}

fn hosts_toml_path() -> Result<std::path::PathBuf> {
    Ok(config::config_dir()?.join("hosts.toml"))
}

fn tunnels_toml_path() -> Result<std::path::PathBuf> {
    Ok(config::config_dir()?.join("tunnels.toml"))
}

fn run_print_command(host_arg: &str, cfg: &oken_config::OkenConfig) -> Result<()> {
    let all = hosts::list_all_hosts()?;
    if let Some(h) = all.iter().find(|h| h.alias == host_arg) {
        let ssh = ssh::find_ssh()?;
        let mut parts = build_ssh_args(h);
        inject_keepalive(&mut parts, cfg.keepalive_interval);
        let mut full = vec![ssh.display().to_string()];
        full.extend(parts);
        println!("{}", full.join(" "));
    } else {
        println!("ssh {host_arg}");
    }
    Ok(())
}

fn run_tunnel_command(cmd: TunnelCommand) -> Result<()> {
    let path = tunnels_toml_path()?;
    match cmd {
        TunnelCommand::Add { name, args } => {
            let host = ssh::extract_target_host_full(&args)
                .ok_or_else(|| anyhow::anyhow!("no target host found in args"))?;

            // Collect ssh flags, excluding all positionals (the host)
            let ssh_flags = extract_ssh_flags(&args);

            tunnels::add_tunnel(&path, &name, tunnels::TunnelEntry { host, ssh_flags })?;
            println!("Added tunnel '{name}'");
            Ok(())
        }

        TunnelCommand::Start { name } => {
            let all = tunnels::load_tunnels(&path)?;
            let entry = all
                .get(&name)
                .ok_or_else(|| anyhow::anyhow!("tunnel '{name}' not found"))?;

            if tunnels::is_running(&name, &entry.host) {
                println!("Tunnel '{name}' is already running");
                return Ok(());
            }

            let sock = tunnels::socket_path(&name)?;
            let ssh = ssh::find_ssh()?;

            let mut cmd_args = vec![
                "-N".to_string(),
                "-M".to_string(),
                "-S".to_string(),
                sock.to_string_lossy().to_string(),
            ];
            cmd_args.extend(entry.ssh_flags.clone());
            cmd_args.push(entry.host.clone());

            std::process::Command::new(&ssh)
                .args(&cmd_args)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .map_err(|e| anyhow::anyhow!("failed to start tunnel: {e}"))?;

            println!("Started tunnel '{name}'");
            Ok(())
        }

        TunnelCommand::Stop { name } => {
            let all = tunnels::load_tunnels(&path)?;
            let entry = all
                .get(&name)
                .ok_or_else(|| anyhow::anyhow!("tunnel '{name}' not found"))?;

            let sock = tunnels::socket_path(&name)?;
            let ssh = ssh::find_ssh()?;

            let status = std::process::Command::new(&ssh)
                .args(["-S", &sock.to_string_lossy(), "-O", "stop", &entry.host])
                .status()
                .map_err(|e| anyhow::anyhow!("failed to stop tunnel: {e}"))?;

            if status.success() {
                println!("Stopped tunnel '{name}'");
            } else {
                anyhow::bail!("failed to stop tunnel '{name}'");
            }
            Ok(())
        }

        TunnelCommand::List => {
            let all = tunnels::load_tunnels(&path)?;
            if all.is_empty() {
                println!("No tunnels configured. Use `oken tunnel add` to add one.");
                return Ok(());
            }

            let mut entries: Vec<_> = all.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));

            let name_w = entries.iter().map(|(n, _)| n.len()).max().unwrap_or(4).max(4);
            let host_w = entries
                .iter()
                .map(|(_, e)| e.host.len())
                .max()
                .unwrap_or(4)
                .max(4);

            println!(
                "{:<name_w$}  {:<host_w$}  {:>7}  {}",
                "NAME", "HOST", "STATUS", "FLAGS"
            );
            for (name, entry) in &entries {
                let status = if tunnels::is_running(name, &entry.host) {
                    "running"
                } else {
                    "stopped"
                };
                let flags = entry.ssh_flags.join(" ");
                println!("{:<name_w$}  {:<host_w$}  {:>7}  {}", name, entry.host, status, flags);
            }
            Ok(())
        }
    }
}

/// Extract only SSH flags (and their values) from args; all positionals are dropped.
fn extract_ssh_flags(args: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    let mut skip_next = false;
    for arg in args {
        if skip_next {
            result.push(arg.clone());
            skip_next = false;
            continue;
        }
        if ssh::FLAGS_WITH_VALUES.contains(&arg.as_str()) {
            result.push(arg.clone());
            skip_next = true;
            continue;
        }
        if arg.starts_with('-') {
            result.push(arg.clone());
            continue;
        }
        // Non-flag positional: skip (it's the host or an unrecognised arg)
    }
    result
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
