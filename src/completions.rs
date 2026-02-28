use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::CommandFactory;
use clap_complete::{Shell, generate};

use crate::cli::Cli;

/// Print completions to stdout. Called by `oken completions generate <shell>`.
pub fn generate_to_stdout(shell: Shell) {
    generate(shell, &mut Cli::command(), "oken", &mut std::io::stdout());
}

/// Resolve the target directory, create it if needed, write the completion
/// file, and print what was done. Called by `oken completions install`.
pub fn install(shell: Option<Shell>, dir: Option<PathBuf>) -> Result<()> {
    let shell = match shell {
        Some(s) => s,
        None => detect_shell()?,
    };

    match shell {
        Shell::Zsh => install_zsh(dir),
        Shell::Bash => install_bash(dir),
        Shell::Fish => install_fish(dir),
        other => bail!(
            "automatic installation is not supported for {other}.\n\
             Generate completions and install manually:\n\
             \n  oken completions generate {other} > <file>\n\
             \nThen follow your shell's documentation for loading completion files."
        ),
    }
}

// ── shell detection ───────────────────────────────────────────────────────────

fn detect_shell() -> Result<Shell> {
    let shell_path = std::env::var("SHELL")
        .context("$SHELL is not set — use --shell to specify one explicitly")?;

    let name = std::path::Path::new(&shell_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&shell_path);

    match name {
        "zsh" => Ok(Shell::Zsh),
        "bash" => Ok(Shell::Bash),
        "fish" => Ok(Shell::Fish),
        "elvish" => Ok(Shell::Elvish),
        "powershell" | "pwsh" => Ok(Shell::PowerShell),
        other => bail!(
            "unrecognised shell '{other}' — use --shell with one of: zsh, bash, fish, elvish, powershell"
        ),
    }
}

// ── zsh ───────────────────────────────────────────────────────────────────────

fn install_zsh(dir: Option<PathBuf>) -> Result<()> {
    let target_dir = match dir {
        Some(d) => {
            std::fs::create_dir_all(&d)
                .with_context(|| format!("could not create {}", d.display()))?;
            d
        }
        None => resolve_zsh_dir()?,
    };

    let file = target_dir.join("_oken");
    write_completions(Shell::Zsh, &file)?;
    println!("Installed zsh completions → {}", file.display());

    // Print fpath hint unless the dir name makes it obvious it's already wired up.
    if !looks_like_fpath_dir(&target_dir) {
        println!(
            "\nAdd to ~/.zshrc (or $ZDOTDIR/.zshrc) if not already there:\n\
             \n  fpath=({dir} $fpath)\n  autoload -Uz compinit && compinit",
            dir = target_dir.display()
        );
    }

    Ok(())
}

/// Priority for the default zsh completion directory:
///   1. $ZDOTDIR/.zfunc  — if $ZDOTDIR is set and the dir exists
///   2. ~/.zfunc         — if it exists
///   3. ~/.config/zsh/.zfunc — if it exists  (common custom setup)
///   4. ~/.zsh/completions   — if it exists
///   5. Create $ZDOTDIR/.zfunc (or ~/.zfunc if $ZDOTDIR unset)
fn resolve_zsh_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("could not determine home directory")?;
    let zdotdir = std::env::var("ZDOTDIR").ok().map(PathBuf::from);

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(ref z) = zdotdir {
        candidates.push(z.join(".zfunc"));
    }
    candidates.push(home.join(".zfunc"));
    candidates.push(home.join(".config/zsh/.zfunc"));
    candidates.push(home.join(".zsh/completions"));

    for c in &candidates {
        if c.is_dir() {
            return Ok(c.clone());
        }
    }

    // Nothing exists — create the preferred default.
    let preferred = zdotdir.map(|z| z.join(".zfunc")).unwrap_or_else(|| home.join(".zfunc"));
    std::fs::create_dir_all(&preferred)
        .with_context(|| format!("could not create {}", preferred.display()))?;
    Ok(preferred)
}

/// Suppress the fpath hint when the directory name already signals it's a
/// standard completion location (user likely has it wired up already).
fn looks_like_fpath_dir(dir: &std::path::Path) -> bool {
    let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
    matches!(name, ".zfunc" | "zfunc" | "completions" | "site-functions")
}

// ── bash ──────────────────────────────────────────────────────────────────────

fn install_bash(dir: Option<PathBuf>) -> Result<()> {
    let target_dir = match dir {
        Some(d) => {
            std::fs::create_dir_all(&d)
                .with_context(|| format!("could not create {}", d.display()))?;
            d
        }
        None => resolve_bash_dir()?,
    };

    let file = target_dir.join("oken");
    write_completions(Shell::Bash, &file)?;
    println!("Installed bash completions → {}", file.display());
    Ok(())
}

/// $BASH_COMPLETION_USER_DIR/completions, or ~/.local/share/bash-completion/completions.
fn resolve_bash_dir() -> Result<PathBuf> {
    let dir = if let Ok(base) = std::env::var("BASH_COMPLETION_USER_DIR") {
        PathBuf::from(base).join("completions")
    } else {
        dirs::data_dir()
            .context("could not determine data directory")?
            .join("bash-completion/completions")
    };
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("could not create {}", dir.display()))?;
    Ok(dir)
}

// ── fish ──────────────────────────────────────────────────────────────────────

fn install_fish(dir: Option<PathBuf>) -> Result<()> {
    let target_dir = match dir {
        Some(d) => {
            std::fs::create_dir_all(&d)
                .with_context(|| format!("could not create {}", d.display()))?;
            d
        }
        None => resolve_fish_dir()?,
    };

    let file = target_dir.join("oken.fish");
    write_completions(Shell::Fish, &file)?;
    println!("Installed fish completions → {}", file.display());
    // fish auto-discovers ~/.config/fish/completions; no hint needed.
    Ok(())
}

/// fish always uses ~/.config/fish/completions regardless of XDG overrides.
fn resolve_fish_dir() -> Result<PathBuf> {
    let dir = dirs::home_dir()
        .context("could not determine home directory")?
        .join(".config/fish/completions");
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("could not create {}", dir.display()))?;
    Ok(dir)
}

// ── shared ────────────────────────────────────────────────────────────────────

fn write_completions(shell: Shell, path: &std::path::Path) -> Result<()> {
    let mut buf = Vec::new();
    generate(shell, &mut Cli::command(), "oken", &mut buf);
    std::fs::write(path, &buf)
        .with_context(|| format!("could not write {}", path.display()))?;
    Ok(())
}
