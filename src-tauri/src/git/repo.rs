use std::path::PathBuf;
use std::fs;
use gix::{Repository, ThreadSafeRepository};
use anyhow::{Result, Context, anyhow};
use serde::{Serialize, Deserialize};

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

    pub fn open(&self) -> Result<Repository> {
        Repository::open(&self.path)
            .with_context(|| format!("Failed to open Git repository at {:?}", self.path))
    }

    pub fn init(&self) -> Result<()> {
        if self.path.join(".git").exists() {
            return Ok(());
        }
        Repository::init(&self.path)?;
        Ok(())
    }

    pub fn status(&self) -> Result<RepoStatus> {
        let repo_result = Repository::open(&self.path);
        if repo_result.is_err() {
            return Ok(RepoStatus {
                is_repo: false,
                has_remote: false,
                remote_url: None,
                current_branch: None,
                is_clean: false,
            });
        }

        let repo = repo_result.unwrap();
        let head = repo.head();
        let current_branch = head.ok().and_then(|h| h.shorthand().map(|s| s.to_string()));

        let remote = repo.find_remote("origin").ok();
        let remote_url = remote.as_ref().and_then(|r| r.url().map(|u| u.to_string()));
        let has_remote = remote_url.is_some();

        let mut is_clean = true;
        if let Ok(mut statuses) = repo.status(gix::status::Platform::default()) {
            if statuses.next().is_some() {
                is_clean = false;
            }
        }

        Ok(RepoStatus {
            is_repo: true,
            has_remote,
            remote_url,
            current_branch,
            is_clean,
        })
    }

    pub fn add_all(&self) -> Result<()> {
        let repo = self.open()?;
        let mut index = repo.index()?;
        index.add_all(["*"].iter(), gix::index::entry::Mode::default(), &mut |_| Ok(true))?;
        index.write_changes()?;
        Ok(())
    }

    pub fn commit(&self, message: &str) -> Result<()> {
        let repo = self.open()?;
        let mut index = repo.index()?;
        let tree_id = index.write_tree()?;
        let tree = repo.find_object(tree_id)?.into_tree();

        let head = repo.head();
        let parent = if let Ok(head) = head {
            if head.is_branch() {
                let oid = head.target().context("No target")?;
                Some(repo.find_object(oid)?.into_commit())
            } else {
                None
            }
        } else {
            None
        };

        let signature = gix::actor::Signature::new_committer()?;
        let parents = parent.as_ref().map(|p| vec![p]).unwrap_or_default();
        let commit_id = repo.commit(
            Some(message),
            &signature,
            &signature,
            &tree,
            parents.iter().map(|p| p.id).collect::<Vec<_>>().as_slice(),
        )?;

        let head_ref = repo.find_reference("HEAD")?;
        if let Some(oid) = head_ref.target() {
            if head_ref.is_branch() {
                let mut refedit = repo.edit_reference(head_ref.name.as_ref())?;
                refedit.set_target(commit_id, gix::refs::transaction::RefChange::Update {
                    log: Some(gix::refs::transaction::LogChange {
                        message: format!("commit: {}", message),
                        mode: gix::refs::transaction::PreviousValue::MustExist,
                    }),
                })?;
                refedit.commit()?;
            }
        }

        Ok(())
    }

    pub fn add_remote(&self, url: &str) -> Result<()> {
        let repo = self.open()?;
        if repo.find_remote("origin").is_ok() {
            return Ok(());
        }
        repo.remote("origin", url)?;
        Ok(())
    }

    pub fn push(&self, branch: &str, remote: &str) -> Result<()> {
        let repo = self.open()?;
        let remote = repo.find_remote(remote)?;
        let refspec = format!("refs/heads/{}:refs/heads/{}", branch, branch);
        let mut connection = remote.connect(gix::protocol::transport::Direction::Push)?;
        connection
            .push(gix::refspec::RefSpec::from_bytes(refspec.as_bytes())?)
            .execute()?;
        Ok(())
    }

    pub fn detect_language(&self) -> Vec<String> {
        let mut detected = Vec::new();
        let entries = fs::read_dir(&self.path).unwrap_or_default();

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