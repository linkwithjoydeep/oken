use std::path::PathBuf;

use clap::{Parser, Subcommand};
pub use clap_complete;

#[derive(Parser)]
#[command(
    name = "oken",
    version = concat!(
        env!("CARGO_PKG_VERSION"),
        " (",
        env!("GIT_HASH"),
        " ",
        env!("BUILD_DATE"),
        ")"
    ),
    about = "A smarter SSH CLI",
    // Don't error on unknown args — they'll be passed to ssh
    disable_help_subcommand = true,
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Filter hosts by tag (open picker pre-filtered, or connect directly if one match)
    #[arg(long)]
    pub tag: Option<String>,

    /// Skip the production-host warning prompt
    #[arg(long)]
    pub yes: bool,

    /// Disable auto-reconnect on connection loss
    #[arg(long = "no-reconnect")]
    pub no_reconnect: bool,

    /// Arguments to pass through to ssh
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub ssh_args: Vec<String>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Manage SSH hosts
    Host {
        #[command(subcommand)]
        command: HostCommand,
    },
    /// Manage SSH tunnels
    Tunnel {
        #[command(subcommand)]
        command: TunnelCommand,
    },
    /// Execute commands on remote hosts
    Exec {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Manage command snippets
    Snippet {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Print the resolved SSH command for a host
    Print {
        /// Alias or host to resolve
        host: String,
    },
    /// View connection history
    Audit {
        /// Number of recent entries to show
        #[arg(short = 'n', long, default_value_t = 50)]
        lines: usize,
    },
    /// Manage SSH keys
    Keys {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Export oken configuration
    Export {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Import oken configuration
    Import {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        shell: clap_complete::Shell,
    },
}

#[derive(Subcommand)]
pub enum HostCommand {
    /// Add a new host
    Add {
        /// Alias name for the host
        name: String,
        /// Target in the form user@host or just host
        target: String,
        /// SSH port
        #[arg(long)]
        port: Option<u16>,
        /// Path to SSH identity file (private key)
        #[arg(long)]
        key: Option<PathBuf>,
        /// Tags for organizing hosts
        #[arg(long, num_args = 1..)]
        tag: Vec<String>,
    },
    /// List all configured hosts
    List,
    /// Remove a host by name
    Remove {
        /// Alias name of the host to remove
        name: String,
    },
    /// Open hosts.toml in $EDITOR
    Edit {
        /// Alias name (currently opens the whole file)
        name: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum TunnelCommand {
    /// Add a new tunnel profile (e.g., oken tunnel add db -L 5432:localhost:5432 prod-db)
    Add {
        /// Tunnel profile name
        name: String,
        /// SSH flags and target host
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Start a saved tunnel in the background
    Start {
        /// Tunnel profile name
        name: String,
    },
    /// Stop a running tunnel
    Stop {
        /// Tunnel profile name
        name: String,
    },
    /// List all tunnel profiles and their status
    List,
}
