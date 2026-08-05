use std::path::PathBuf;
use std::process::Command;
use anyhow::{Result, Context, anyhow};
use serde::{Serialize, Deserialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoStatus {
    pub is_repo: bool,
    pub has_remote: bool,
    pub remote_url: Option<String>,
    pub current_branch: Option<String>,
    pub is_clean: bool,
}

pub struct GitRepo {
    path: PathBuf,
}

impl GitRepo {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn run_git(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("git")
            .current_dir(&self.path)
            .args(args)
            .output()
            .context("Failed to execute git command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Git command failed: {}", stderr));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub fn is_git_repo(&self) -> bool {
        self.run_git(&["rev-parse", "--git-dir"]).is_ok()
    }

    pub fn init(&self) -> Result<()> {
        if self.is_git_repo() {
            return Ok(());
        }
        self.run_git(&["init"])?;
        Ok(())
    }

    pub fn status(&self) -> Result<RepoStatus> {
        if !self.is_git_repo() {
            return Ok(RepoStatus {
                is_repo: false,
                has_remote: false,
                remote_url: None,
                current_branch: None,
                is_clean: false,
            });
        }

        let branch = self.run_git(&["branch", "--show-current"]).ok();
        let remote_url = self.run_git(&["remote", "get-url", "origin"]).ok();
        let has_remote = remote_url.is_some();

        let status_output = self.run_git(&["status", "--porcelain"]).unwrap_or_default();
        let is_clean = status_output.is_empty();

        Ok(RepoStatus {
            is_repo: true,
            has_remote,
            remote_url,
            current_branch: branch,
            is_clean,
        })
    }

    pub fn add_all(&self) -> Result<()> {
        self.run_git(&["add", "."])?;
        Ok(())
    }

    pub fn commit(&self, message: &str) -> Result<()> {
        self.run_git(&["commit", "-m", message])?;
        Ok(())
    }

    pub fn add_remote(&self, url: &str) -> Result<()> {
        if self.run_git(&["remote", "get-url", "origin"]).is_ok() {
            return Ok(());
        }
        self.run_git(&["remote", "add", "origin", url])?;
        Ok(())
    }

    pub fn push(&self, branch: &str) -> Result<()> {
        self.run_git(&["push", "-u", "origin", branch])?;
        Ok(())
    }

    pub fn detect_language(&self) -> Vec<String> {
        let mut detected = Vec::new();
        let entries = match fs::read_dir(&self.path) {
            Ok(e) => e,
            Err(_) => return detected,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");

            match filename {
                "Cargo.toml" => detected.push("Rust".to_string()),
                "package.json" => detected.push("Node.js".to_string()),
                "requirements.txt" => detected.push("Python".to_string()),
                "go.mod" => detected.push("Go".to_string()),
                "pom.xml" => detected.push("Java (Maven)".to_string()),
                "build.gradle" => detected.push("Java (Gradle)".to_string()),
                "CMakeLists.txt" => detected.push("C++".to_string()),
                "Dockerfile" => detected.push("Docker".to_string()),
                _ => {}
            }
        }

        detected
    }
}