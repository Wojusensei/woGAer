use anyhow::{Result, Context, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::json;
pub struct WorkflowTemplate;

impl WorkflowTemplate {
    pub fn generate(language: &str, project_name: &str) -> String {
        match language {
            "Rust" => Self::rust_template(project_name),
            "Node.js" => Self::node_template(project_name),
            "Python" => Self::python_template(project_name),
            "Go" => Self::go_template(project_name),
            _ => Self::default_template(project_name),
        }
    }

    fn rust_template(name: &str) -> String {
        format!(
            r#"name: Build

on:
  push:
    branches: [ main ]
  pull_request:
    branches: [ main ]
  workflow_dispatch:

env:
  CARGO_TERM_COLOR: always

jobs:
  build:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        target: [x86_64-unknown-linux-gnu, x86_64-pc-windows-gnu, x86_64-apple-darwin]
    steps:
    - uses: actions/checkout@v4
    - name: Install Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: stable
        target: ${{{{ matrix.target }}}}
        override: true
    - name: Build
      run: cargo build --release --target ${{{{ matrix.target }}}}
    - name: Upload artifacts
      uses: actions/upload-artifact@v4
      with:
        name: {}-${{{{ matrix.target }}}}
        path: target/${{{{ matrix.target }}}}/release/*

"#,
            name
        )
    }

    fn node_template(name: &str) -> String {
        format!(
            r#"name: Build

on:
  push:
    branches: [ main ]
  pull_request:
    branches: [ main ]
  workflow_dispatch:

jobs:
  build:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        node-version: [20.x, 22.x]
    steps:
    - uses: actions/checkout@v4
    - name: Use Node.js ${{{{ matrix.node-version }}}}
      uses: actions/setup-node@v4
      with:
        node-version: ${{{{ matrix.node-version }}}}
        cache: 'npm'
    - run: npm ci
    - run: npm run build --if-present
    - name: Upload artifacts
      uses: actions/upload-artifact@v4
      with:
        name: {}-${{{{ matrix.node-version }}}}
        path: dist/

"#,
            name
        )
    }

    fn python_template(name: &str) -> String {
        format!(
            r#"name: Build

on:
  push:
    branches: [ main ]
  pull_request:
    branches: [ main ]
  workflow_dispatch:

jobs:
  build:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        python-version: ['3.9', '3.10', '3.11']
    steps:
    - uses: actions/checkout@v4
    - name: Set up Python ${{{{ matrix.python-version }}}}
      uses: actions/setup-python@v5
      with:
        python-version: ${{{{ matrix.python-version }}}}
    - name: Install dependencies
      run: |
        python -m pip install --upgrade pip
        pip install -r requirements.txt
    - name: Build
      run: python setup.py build
    - name: Upload artifacts
      uses: actions/upload-artifact@v4
      with:
        name: {}-${{{{ matrix.python-version }}}}
        path: build/

"#,
            name
        )
    }

    fn go_template(name: &str) -> String {
        format!(
            r#"name: Build

on:
  push:
    branches: [ main ]
  pull_request:
    branches: [ main ]
  workflow_dispatch:

jobs:
  build:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        go-version: [1.21.x, 1.22.x]
    steps:
    - uses: actions/checkout@v4
    - name: Set up Go
      uses: actions/setup-go@v5
      with:
        go-version: ${{{{ matrix.go-version }}}}
    - name: Build
      run: go build -v ./...
    - name: Upload artifacts
      uses: actions/upload-artifact@v4
      with:
        name: {}-${{{{ matrix.go-version }}}}
        path: ./

"#,
            name
        )
    }

    fn default_template(name: &str) -> String {
        format!(
            r#"name: Build

on:
  push:
    branches: [ main ]
  pull_request:
    branches: [ main ]
  workflow_dispatch:

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v4
    - name: Build
      run: echo "No build script configured"
    - name: Upload artifacts
      uses: actions/upload-artifact@v4
      with:
        name: {}
        path: .

"#,
            name
        )
    }
}
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowRun {
    pub id: i64,
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub html_url: String,
    pub run_number: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub id: i64,
    pub name: String,
    pub size_in_bytes: i64,
    pub created_at: String,
    pub download_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDispatchOptions {
    pub ref_name: String,
    pub inputs: HashMap<String, String>,
}

pub struct WorkflowClient {
    token: String,
    repo_owner: String,
    repo_name: String,
    client: reqwest::blocking::Client,
}

impl WorkflowClient {
    pub fn new(token: String, repo_owner: String, repo_name: String) -> Self {
        Self {
            token,
            repo_owner,
            repo_name,
            client: reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap(),
        }
    }

    fn base_url(&self) -> String {
        format!(
            "https://api.github.com/repos/{}/{}",
            self.repo_owner, self.repo_name
        )
    }

    pub fn dispatch_workflow(&self, workflow_id: &str, options: &WorkflowDispatchOptions) -> Result<()> {
        let url = format!("{}/actions/workflows/{}/dispatches", self.base_url(), workflow_id);
        
        let mut payload = serde_json::Map::new();
        payload.insert("ref".to_string(), json!(options.ref_name));
        if !options.inputs.is_empty() {
            payload.insert("inputs".to_string(), json!(options.inputs));
        }

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", "woGAer")
            .json(&payload)
            .send()
            .context("Failed to dispatch workflow")?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            Err(anyhow!("Dispatch failed: {} - {}", status, text))
        }
    }

    pub fn list_runs(&self, workflow_id: Option<&str>, per_page: usize) -> Result<Vec<WorkflowRun>> {
        let mut url = format!("{}/actions/runs", self.base_url());
        let mut params = vec![];
        if let Some(wf) = workflow_id {
            params.push(format!("workflow_id={}", wf));
        }
        params.push(format!("per_page={}", per_page));
        if !params.is_empty() {
            url.push_str(&format!("?{}", params.join("&")));
        }

        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", "woGAer")
            .send()
            .context("Failed to list runs")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(anyhow!("List runs failed: {} - {}", status, text));
        }

        let data: serde_json::Value = response.json()?;
        let runs = data["workflow_runs"]
            .as_array()
            .ok_or_else(|| anyhow!("Invalid response format"))?
            .iter()
            .map(|run| {
                Ok(WorkflowRun {
                    id: run["id"].as_i64().unwrap_or(0),
                    name: run["name"].as_str().unwrap_or("").to_string(),
                    status: run["status"].as_str().unwrap_or("").to_string(),
                    conclusion: run["conclusion"].as_str().map(|s| s.to_string()),
                    created_at: run["created_at"].as_str().unwrap_or("").to_string(),
                    updated_at: run["updated_at"].as_str().unwrap_or("").to_string(),
                    html_url: run["html_url"].as_str().unwrap_or("").to_string(),
                    run_number: run["run_number"].as_i64().unwrap_or(0),
                })
            })
            .collect::<Result<Vec<_>, anyhow::Error>>()?;

        Ok(runs)
    }

    pub fn get_run_status(&self, run_id: i64) -> Result<WorkflowRun> {
        let url = format!("{}/actions/runs/{}", self.base_url(), run_id);

        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", "woGAer")
            .send()
            .context("Failed to get run status")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(anyhow!("Get run failed: {} - {}", status, text));
        }

        let data: serde_json::Value = response.json()?;
        Ok(WorkflowRun {
            id: data["id"].as_i64().unwrap_or(0),
            name: data["name"].as_str().unwrap_or("").to_string(),
            status: data["status"].as_str().unwrap_or("").to_string(),
            conclusion: data["conclusion"].as_str().map(|s| s.to_string()),
            created_at: data["created_at"].as_str().unwrap_or("").to_string(),
            updated_at: data["updated_at"].as_str().unwrap_or("").to_string(),
            html_url: data["html_url"].as_str().unwrap_or("").to_string(),
            run_number: data["run_number"].as_i64().unwrap_or(0),
        })
    }

    pub fn list_artifacts(&self, run_id: i64) -> Result<Vec<Artifact>> {
        let url = format!("{}/actions/runs/{}/artifacts", self.base_url(), run_id);

        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", "woGAer")
            .send()
            .context("Failed to list artifacts")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(anyhow!("List artifacts failed: {} - {}", status, text));
        }

        let data: serde_json::Value = response.json()?;
        let artifacts = data["artifacts"]
            .as_array()
            .ok_or_else(|| anyhow!("Invalid response format"))?
            .iter()
            .map(|art| {
                Ok(Artifact {
                    id: art["id"].as_i64().unwrap_or(0),
                    name: art["name"].as_str().unwrap_or("").to_string(),
                    size_in_bytes: art["size_in_bytes"].as_i64().unwrap_or(0),
                    created_at: art["created_at"].as_str().unwrap_or("").to_string(),
                    download_url: format!(
                        "{}/actions/artifacts/{}/zip",
                        self.base_url(),
                        art["id"].as_i64().unwrap_or(0)
                    ),
                })
            })
            .collect::<Result<Vec<_>, anyhow::Error>>()?;

        Ok(artifacts)
    }

    pub fn download_artifact(&self, artifact_id: i64, dest_path: &std::path::Path) -> Result<()> {
        let url = format!("{}/actions/artifacts/{}/zip", self.base_url(), artifact_id);

        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", "woGAer")
            .send()
            .context("Failed to download artifact")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(anyhow!("Download failed: {} - {}", status, text));
        }

        let bytes = response.bytes()?;
        std::fs::write(dest_path, bytes)?;
        Ok(())
    }

    pub fn list_workflows(&self) -> Result<Vec<serde_json::Value>> {
        let url = format!("{}/actions/workflows", self.base_url());

        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", "woGAer")
            .send()
            .context("Failed to list workflows")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(anyhow!("List workflows failed: {} - {}", status, text));
        }

        let data: serde_json::Value = response.json()?;
        Ok(data["workflows"].as_array().unwrap_or(&vec![]).clone())
    }

    pub fn validate_token(&self) -> Result<bool> {
        let url = "https://api.github.com/user".to_string();

        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", "woGAer")
            .send()
            .context("Failed to validate token")?;

        Ok(response.status().is_success())
    }

    pub fn get_user(&self) -> Result<serde_json::Value> {
        let response = self
            .client
            .get("https://api.github.com/user")
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", "woGAer")
            .send()
            .context("Failed to get GitHub user")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(anyhow!("Get user failed: {} - {}", status, text));
        }

        let data: serde_json::Value = response.json()?;
        Ok(json!({
            "login": data["login"].as_str().unwrap_or(""),
            "name": data["name"].as_str().unwrap_or(""),
            "avatar_url": data["avatar_url"].as_str().unwrap_or(""),
        }))
    }

    pub fn push_workflow_file(&self, branch: &str, filename: &str, content_base64: &str) -> Result<()> {
        let path = format!(
            "{}/contents/.github/workflows/{}",
            self.base_url(),
            filename
        );
        let check_url = format!("{}?ref={}", path, branch);

        let existing_sha = {
            let response = self
                .client
                .get(&check_url)
                .header("Authorization", format!("Bearer {}", self.token))
                .header("Accept", "application/vnd.github.v3+json")
                .header("User-Agent", "woGAer")
                .send()
                .context("Failed to check workflow file")?;

            match response.status().as_u16() {
                200 => {
                    let data: serde_json::Value = response.json()?;
                    Some(data["sha"].as_str().unwrap_or("").to_string())
                }
                404 => None,
                status => {
                    let text = response.text().unwrap_or_default();
                    return Err(anyhow!("Check workflow failed: {} - {}", status, text));
                }
            }
        };

        let mut payload = serde_json::Map::new();
        payload.insert(
            "message".to_string(),
            json!("chore: add GitHub Actions build workflow"),
        );
        payload.insert("content".to_string(), json!(content_base64));
        payload.insert("branch".to_string(), json!(branch));
        if let Some(sha) = existing_sha {
            payload.insert("sha".to_string(), json!(sha));
        }

        let response = self
            .client
            .put(&path)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", "woGAer")
            .json(&payload)
            .send()
            .context("Failed to push workflow")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(anyhow!("Push workflow failed: {} - {}", status, text));
        }

        Ok(())
    }

    pub fn validate_repo_access(&self) -> Result<()> {
        let url = self.base_url();

        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", "woGAer")
            .send()
            .context("Failed to validate repo access")?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            Err(anyhow!("Repo check failed: {} - {}", status, text))
        }
    }
}
