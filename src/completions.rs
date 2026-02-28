use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::CommandFactory;
use clap_complete::{Shell, generate};

use crate::cli::Cli;

/// Resolve the target directory, create it if needed, write the completion
/// file, and print what was done. Called by `oken completions`.
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
             To install manually, pipe completions to a file:\n\
             \n  oken completions --shell {other} > <file>\n\
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

    patch_zshrc(&target_dir)?;
    println!("Reload your shell to activate: exec zsh");
    Ok(())
}

/// Appends the required fpath line (and compinit if missing) to the user's
/// .zshrc. Prints a one-line summary of what was changed.
fn patch_zshrc(fpath_dir: &Path) -> Result<()> {
    let zshrc = resolve_zshrc_path()?;
    let dir_str = fpath_dir.display().to_string();

    let content = std::fs::read_to_string(&zshrc).unwrap_or_default();

    // If the dir is already referenced in an fpath line, nothing to do.
    if content.contains(&dir_str) {
        println!("{} is already configured", zshrc.display());
        return Ok(());
    }

    // Append fpath line; also add compinit if it isn't present yet.
    let needs_compinit = !content.contains("compinit");
    let mut addition = format!("\n# Added by oken completions\nfpath=({dir} $fpath)\n", dir = dir_str);
    if needs_compinit {
        addition.push_str("autoload -Uz compinit && compinit\n");
    }

    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&zshrc)
        .with_context(|| format!("could not open {}", zshrc.display()))?;
    file.write_all(addition.as_bytes())
        .with_context(|| format!("could not write to {}", zshrc.display()))?;

    println!("Patched {} with fpath entry", zshrc.display());
    Ok(())
}

fn resolve_zshrc_path() -> Result<PathBuf> {
    let base = std::env::var("ZDOTDIR")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from("~")));
    Ok(base.join(".zshrc"))
}

/// Priority for the fallback zsh completion directory:
///   1. $ZDOTDIR/.zfunc  — if $ZDOTDIR is set and the dir exists
///   2. ~/.zfunc         — if it exists
///   3. ~/.config/zsh/.zfunc — if it exists
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

    let preferred = zdotdir.map(|z| z.join(".zfunc")).unwrap_or_else(|| home.join(".zfunc"));
    std::fs::create_dir_all(&preferred)
        .with_context(|| format!("could not create {}", preferred.display()))?;
    Ok(preferred)
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

    if !bash_completion_active() {
        println!(
            "\nNote: completions require the bash-completion package to be installed and sourced.\n\
             Install:  brew install bash-completion@2     (macOS)\n\
             or        sudo apt install bash-completion   (Debian/Ubuntu)"
        );
    }
    Ok(())
}

/// Returns true if bash-completion appears to be set up on this system.
fn bash_completion_active() -> bool {
    // System-wide (Linux)
    if Path::new("/usr/share/bash-completion/bash_completion").exists() {
        return true;
    }
    // Homebrew
    let brew_prefix = std::env::var("HOMEBREW_PREFIX").unwrap_or_default();
    for prefix in [brew_prefix.as_str(), "/opt/homebrew", "/usr/local"] {
        if !prefix.is_empty()
            && Path::new(&format!("{prefix}/etc/profile.d/bash_completion.sh")).exists()
        {
            return true;
        }
    }
    false
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
    println!("Completions are active immediately — no further setup needed.");
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
