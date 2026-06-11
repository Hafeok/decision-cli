///! Safety checks for .env and .gitignore (FT-114).
use std::fs;
use std::io::{self, Write};
use std::path::Path;

use super::InitError;

/// Outcome of gitignore safety check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitignoreOutcome {
    /// .env was already listed in .gitignore.
    Unchanged,
    /// .env was appended to existing .gitignore.
    Appended,
    /// .gitignore was created with .env as its only line.
    Created,
}

/// Bootstrap .env.example if it doesn't exist.
/// Leaves existing .env.example untouched (operator may have customized it).
pub fn bootstrap_env_example(repo_root: &Path) -> Result<bool, InitError> {
    let env_example = repo_root.join(".env.example");
    if env_example.exists() {
        return Ok(false); // Already exists, don't touch it
    }

    let template = r#"# Decision-CLI orchestrator credentials.
# Copy to .env and fill in your values. .env is gitignored.

# Scaleway Claude inference endpoint (default provider).
SCW_SECRET_KEY=

# Future providers: add here as their capability bindings ship.
"#;

    fs::write(&env_example, template)
        .map_err(|e| InitError::PersistFailed(format!("Failed to write .env.example: {}", e)))?;

    Ok(true) // Created
}

/// Check if .env is in .gitignore, and append if missing.
/// Prompts in TTY mode unless --yes is set.
pub fn ensure_env_gitignored(
    repo_root: &Path,
    auto_confirm: bool,
) -> Result<GitignoreOutcome, InitError> {
    let gitignore = repo_root.join(".gitignore");

    // If .gitignore doesn't exist, create it with .env
    if !gitignore.exists() {
        fs::write(&gitignore, ".env\n")
            .map_err(|e| InitError::PersistFailed(format!("Failed to create .gitignore: {}", e)))?;
        return Ok(GitignoreOutcome::Created);
    }

    // Check if .gitignore is actually a file (not a directory)
    let metadata = fs::metadata(&gitignore).map_err(|e| {
        InitError::PersistFailed(format!("Failed to read .gitignore metadata: {}", e))
    })?;
    if !metadata.is_file() {
        return Err(InitError::PersistFailed(
            ".gitignore exists but is not a file (directory or symlink?)".to_string(),
        ));
    }

    // Read existing .gitignore
    let content = fs::read_to_string(&gitignore)
        .map_err(|e| InitError::PersistFailed(format!("Failed to read .gitignore: {}", e)))?;

    // Check if .env or /.env is already listed
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == ".env" || trimmed == "/.env" {
            return Ok(GitignoreOutcome::Unchanged);
        }
    }

    // .env is missing — ask to append (or auto-confirm)
    let should_append = if auto_confirm || !is_tty() {
        true
    } else {
        prompt_append()?
    };

    if !should_append {
        return Err(InitError::PersistFailed(
            "Operator declined to append .env to .gitignore".to_string(),
        ));
    }

    // Append .env to .gitignore
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(&gitignore)
        .map_err(|e| {
            InitError::PersistFailed(format!("Failed to open .gitignore for append: {}", e))
        })?;

    // Ensure the file ends with a newline before appending
    let needs_newline = !content.is_empty() && !content.ends_with('\n');
    if needs_newline {
        file.write_all(b"\n").map_err(|e| {
            InitError::PersistFailed(format!("Failed to write to .gitignore: {}", e))
        })?;
    }

    file.write_all(b".env\n")
        .map_err(|e| InitError::PersistFailed(format!("Failed to write to .gitignore: {}", e)))?;

    Ok(GitignoreOutcome::Appended)
}

fn is_tty() -> bool {
    use std::io::IsTerminal;
    io::stdin().is_terminal()
}

fn prompt_append() -> Result<bool, InitError> {
    print!(".env is not in .gitignore — append? [Y/n] ");
    io::stdout().flush().ok();

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| InitError::Internal(format!("Failed to read stdin: {}", e)))?;

    let response = input.trim().to_lowercase();
    Ok(response.is_empty() || response == "y" || response == "yes")
}
