use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

pub fn base_env() -> HashMap<String, String> {
    let mut env: HashMap<String, String> = std::env::vars().collect();
    env.insert("GIT_TERMINAL_PROMPT".into(), "0".into());
    env
}

pub fn git<I, S>(dir: &Path, args: I) -> Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let args: Vec<std::ffi::OsString> = args
        .into_iter()
        .map(|arg| arg.as_ref().to_os_string())
        .collect();
    let output = Command::new("git")
        .args(&args)
        .current_dir(dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .with_context(|| format!("running git in {}", dir.display()))?;
    if !output.status.success() {
        bail!(
            "git {} failed in {}: {}\n{}",
            args.iter()
                .map(|arg| arg.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" "),
            dir.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn git_inherit<I, S>(dir: &Path, args: I) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let status = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| format!("running git in {}", dir.display()))?;
    if !status.success() {
        bail!("git command failed in {}: {}", dir.display(), status);
    }
    Ok(())
}

pub fn run_shell(command: &str, cwd: &Path, env: &HashMap<String, String>) -> Result<()> {
    if command.trim().is_empty() {
        return Ok(());
    }
    let script = format!("set -euo pipefail\n{command}");
    let output = Command::new("bash")
        .arg("-c")
        .arg(script)
        .current_dir(cwd)
        .envs(env)
        .output()
        .with_context(|| format!("running a command in {}", cwd.display()))?;
    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "command failed ({}): {}\n{}\n{}",
            output.status,
            command,
            stdout.trim(),
            stderr.trim()
        );
    }
    Ok(())
}

pub fn run_cr(
    bin: &str,
    args: &[String],
    cwd: &Path,
    env: &HashMap<String, String>,
) -> Result<String> {
    let output = Command::new(bin)
        .args(args)
        .current_dir(cwd)
        .envs(env)
        .output()
        .with_context(|| format!("running {bin}"))?;
    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "{bin} {} failed ({}):\n{}\n{}",
            args.join(" "),
            output.status,
            stdout.trim(),
            stderr.trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn parse_outputs(path: &Path) -> Result<HashMap<String, String>> {
    let mut outputs = HashMap::new();
    if !path.exists() {
        return Ok(outputs);
    }
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    for line in text.lines() {
        if let Some((key, value)) = line.split_once('=') {
            if !key.is_empty() {
                outputs.insert(key.to_string(), value.to_string());
            }
        }
    }
    Ok(outputs)
}

pub fn expand_artifact(pattern: &str, cwd: &Path) -> Result<Vec<PathBuf>> {
    if pattern.trim().is_empty() {
        return Ok(Vec::new());
    }
    let pattern = Path::new(pattern);
    let full = if pattern.is_absolute() {
        pattern.to_path_buf()
    } else {
        cwd.join(pattern)
    };
    let mut matches: Vec<PathBuf> = glob::glob(&full.to_string_lossy())?
        .filter_map(|entry| entry.ok())
        .collect();
    matches.sort();
    Ok(matches)
}

pub fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst).with_context(|| format!("creating {}", dst.display()))?;
    for entry in fs::read_dir(src).with_context(|| format!("reading {}", src.display()))? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let target = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)
                .with_context(|| format!("copying {}", entry.path().display()))?;
        }
    }
    Ok(())
}

pub fn template_dir(explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        if path.is_dir() {
            return Ok(path.to_path_buf());
        }
        bail!("template directory {} does not exist", path.display());
    }
    let local = Path::new("sandbox");
    if local.is_dir() {
        return Ok(local.to_path_buf());
    }
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("sandbox");
    if manifest.is_dir() {
        return Ok(manifest);
    }
    bail!("sandbox template not found; pass --template or run from the harness directory")
}

pub fn write_file(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    let mut file =
        fs::File::create(path).with_context(|| format!("creating {}", path.display()))?;
    file.write_all(contents.as_bytes())
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_github_outputs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out");
        fs::write(&path, "version=1.2.3\ntag=v1.2.3\nsuperseded=false\n").unwrap();
        let outputs = parse_outputs(&path).unwrap();
        assert_eq!(outputs.get("version").map(String::as_str), Some("1.2.3"));
        assert_eq!(outputs.get("tag").map(String::as_str), Some("v1.2.3"));
    }

    #[test]
    fn expands_artifacts() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.tgz"), "a").unwrap();
        fs::write(dir.path().join("b.tgz"), "b").unwrap();
        let matches = expand_artifact("*.tgz", dir.path()).unwrap();
        assert_eq!(matches.len(), 2);
    }
}
