use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "oken",
    version,
    about = "A smarter SSH CLI",
    // Don't error on unknown args — they'll be passed to ssh
    disable_help_subcommand = true,
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

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
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
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
    /// Print SSH config information
    Print {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Audit SSH configurations
    Audit {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
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
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
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
