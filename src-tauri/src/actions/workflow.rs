use anyhow::{Result, Context, anyhow};
use serde::{Deserialize, Serialize};
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
        
        let mut payload = HashMap::new();
        payload.insert("ref", options.ref_name.clone());
        if !options.inputs.is_empty() {
            if !options.inputs.is_empty() {
                let inputs_json = serde_json::to_string(&options.inputs).unwrap_or_default();
                payload.insert("inputs", inputs_json);
            }
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

    pub fn validate_repo_access(&self) -> Result<bool> {
        let url = self.base_url();

        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", "woGAer")
            .send()
            .context("Failed to validate repo access")?;

        Ok(response.status().is_success())
    }
}