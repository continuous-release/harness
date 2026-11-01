use std::ffi::OsString;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::process::{copy_dir_all, git_inherit};

/// Recreate a sandbox repository from the template and move its `base` tag.
pub fn seed(repository: &str, token: &str, template: &Path) -> Result<()> {
    let dir = tempfile::Builder::new()
        .prefix("cr-seed-")
        .tempdir()
        .context("creating the seed directory")?;
    let remote = format!("https://x-access-token:{token}@github.com/{repository}.git");

    let clone_args: Vec<OsString> = vec![
        "clone".into(),
        remote.into(),
        dir.path().as_os_str().to_os_string(),
    ];
    git_inherit(Path::new("."), clone_args)?;
    git_inherit(
        dir.path(),
        ["config", "user.email", "harness@continuous-release.invalid"],
    )?;
    git_inherit(
        dir.path(),
        ["config", "user.name", "continuous-release harness"],
    )?;

    for entry in fs::read_dir(dir.path())? {
        let entry = entry?;
        if entry.file_name() == ".git" {
            continue;
        }
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            fs::remove_dir_all(entry.path())?;
        } else {
            fs::remove_file(entry.path())?;
        }
    }
    copy_dir_all(template, dir.path())?;

    git_inherit(dir.path(), ["add", "-A"])?;
    git_inherit(dir.path(), ["commit", "-m", "sandbox: base [skip release]"])?;
    git_inherit(dir.path(), ["tag", "-f", "base"])?;
    git_inherit(dir.path(), ["push", "--force", "origin", "main"])?;
    git_inherit(dir.path(), ["push", "--force", "origin", "refs/tags/base"])?;

    println!("seeded {repository} from {}", template.display());
    Ok(())
}
