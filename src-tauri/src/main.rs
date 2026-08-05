#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod db;
mod git;
mod actions;
mod downloader;
mod i18n;

use std::path::PathBuf;
use std::sync::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{Manager, State, Window, Emitter};
use tauri_plugin_dialog::DialogExt;

use db::{Database, BuildRecord};
use git::GitRepo;
use actions::workflow::{WorkflowClient, WorkflowDispatchOptions};
use downloader::Downloader;
use i18n::I18n;

pub struct AppState {
    db: Mutex<Option<Database>>,
    current_project: Mutex<Option<PathBuf>>,
    github_token: Mutex<Option<String>>,
    i18n: Mutex<I18n>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            db: Mutex::new(None),
            current_project: Mutex::new(None),
            github_token: Mutex::new(None),
            i18n: Mutex::new(I18n::new("zh")),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectAnalysis {
    pub is_git_repo: bool,
    pub has_remote: bool,
    pub remote_url: Option<String>,
    pub current_branch: Option<String>,
    pub is_clean: bool,
    pub languages: Vec<String>,
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BuildRequest {
    pub repo_owner: String,
    pub repo_name: String,
    pub workflow_id: String,
    pub ref_name: String,
    pub inputs: serde_json::Value,
    pub project_path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BuildResponse {
    pub run_id: i64,
    pub run_number: i64,
    pub status: String,
    pub html_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HistoryResponse {
    pub records: Vec<BuildRecord>,
    pub total: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DownloadRequest {
    pub artifact_ids: Vec<i64>,
    pub dest_dir: String,
    pub repo_owner: String,
    pub repo_name: String,
}

#[tauri::command]
async fn open_folder_dialog(window: Window, state: State<'_, AppState>) -> Result<String, String> {
    let title = state.i18n.lock().unwrap().get("folder_picker_title");

    match window
        .dialog()
        .file()
        .set_title(&title)
        .blocking_pick_folder()
    {
        Some(path) => path
            .into_path()
            .map(|p| p.to_string_lossy().to_string())
            .map_err(|e| e.to_string()),
        None => Err(state.i18n.lock().unwrap().get("no_folder_selected")),
    }
}

#[tauri::command]
fn analyze_project(project_path: String, state: State<AppState>) -> Result<ProjectAnalysis, String> {
    let i18n = state.i18n.lock().unwrap();
    let path = PathBuf::from(&project_path);
    if !path.exists() || !path.is_dir() {
        return Err(i18n.get("path_not_exist"));
    }

    let repo = GitRepo::new(path.clone());
    let status = repo.status().map_err(|e| e.to_string())?;
    let languages = repo.detect_language();

    Ok(ProjectAnalysis {
        is_git_repo: status.is_repo,
        has_remote: status.has_remote,
        remote_url: status.remote_url,
        current_branch: status.current_branch,
        is_clean: status.is_clean,
        languages,
        path: project_path,
    })
}

#[tauri::command]
fn github_login(token: String, state: State<AppState>) -> Result<bool, String> {
    let client = WorkflowClient::new(token.clone(), "".to_string(), "".to_string());
    let valid = client.validate_token().map_err(|e| e.to_string())?;

    if valid {
        let mut token_guard = state.github_token.lock().unwrap();
        *token_guard = Some(token);
        Ok(true)
    } else {
        let i18n = state.i18n.lock().unwrap();
        Err(i18n.get("token_invalid"))
    }
}

#[tauri::command]
fn github_logout(state: State<AppState>) -> Result<(), String> {
    let mut token_guard = state.github_token.lock().unwrap();
    *token_guard = None;
    Ok(())
}

#[tauri::command]
fn is_logged_in(state: State<AppState>) -> Result<bool, String> {
    let token_guard = state.github_token.lock().unwrap();
    Ok(token_guard.is_some())
}

#[tauri::command]
fn set_language(lang: String, state: State<AppState>) -> Result<String, String> {
    let mut i18n = state.i18n.lock().unwrap();
    i18n.set_lang(&lang);
    Ok(lang)
}

#[tauri::command]
fn get_current_language(state: State<AppState>) -> Result<String, String> {
    let i18n = state.i18n.lock().unwrap();
    Ok(i18n.lang.clone())
}

#[tauri::command]
fn get_string(key: String, state: State<AppState>) -> Result<String, String> {
    let i18n = state.i18n.lock().unwrap();
    Ok(i18n.get(&key))
}

#[tauri::command]
fn trigger_build(
    request: BuildRequest,
    state: State<AppState>,
) -> Result<BuildResponse, String> {
    let token_guard = state.github_token.lock().unwrap();
    let token = token_guard.as_ref().ok_or_else(|| {
        let i18n = state.i18n.lock().unwrap();
        i18n.get("not_logged_in")
    })?;

    let client = WorkflowClient::new(
        token.clone(),
        request.repo_owner.clone(),
        request.repo_name.clone(),
    );

    let mut inputs = std::collections::HashMap::new();
    if let Some(obj) = request.inputs.as_object() {
        for (k, v) in obj {
            if let Some(s) = v.as_str() {
                inputs.insert(k.clone(), s.to_string());
            }
        }
    }

    let options = WorkflowDispatchOptions {
        ref_name: request.ref_name,
        inputs,
    };

    client.dispatch_workflow(&request.workflow_id, &options)
        .map_err(|e| e.to_string())?;

    std::thread::sleep(std::time::Duration::from_secs(2));
    let runs = client.list_runs(Some(&request.workflow_id), 1)
        .map_err(|e| e.to_string())?;

    if let Some(run) = runs.first() {
        if let Some(db) = state.db.lock().unwrap().as_ref() {
            let record = BuildRecord {
                id: 0,
                repo_name: format!("{}/{}", request.repo_owner, request.repo_name),
                workflow_id: request.workflow_id.clone(),
                status: run.status.clone(),
                created_at: run.created_at.clone(),
                artifact_url: None,
                trigger_type: "manual".to_string(),
                platform: std::env::consts::OS.to_string(),
            };
            let _ = db.insert_record(&record);
        }

        Ok(BuildResponse {
            run_id: run.id,
            run_number: run.run_number,
            status: run.status.clone(),
            html_url: run.html_url.clone(),
        })
    } else {
        let i18n = state.i18n.lock().unwrap();
        Err(i18n.get("build_failed"))
    }
}

#[tauri::command]
fn get_build_history(
    repo_name: Option<String>,
    limit: Option<usize>,
    state: State<AppState>,
) -> Result<HistoryResponse, String> {
    let db_guard = state.db.lock().unwrap();
    let db = db_guard.as_ref().ok_or("数据库未初始化")?;

    let limit_val = limit.unwrap_or(50);
    let records = if let Some(repo) = repo_name {
        db.get_records_by_repo(&repo, limit_val)
    } else {
        db.get_all_records(limit_val)
    }.map_err(|e| e.to_string())?;

    let total = db.count_records().map_err(|e| e.to_string())?;

    Ok(HistoryResponse { records, total })
}

#[tauri::command]
fn get_build_status(
    run_id: i64,
    repo_owner: String,
    repo_name: String,
    state: State<AppState>,
) -> Result<serde_json::Value, String> {
    let token_guard = state.github_token.lock().unwrap();
    let token = token_guard.as_ref().ok_or_else(|| {
        let i18n = state.i18n.lock().unwrap();
        i18n.get("not_logged_in")
    })?;

    let client = WorkflowClient::new(token.clone(), repo_owner, repo_name);
    let run = client.get_run_status(run_id).map_err(|e| e.to_string())?;

    Ok(json!({
        "id": run.id,
        "status": run.status,
        "conclusion": run.conclusion,
        "updated_at": run.updated_at,
        "html_url": run.html_url,
    }))
}

#[tauri::command]
fn get_artifacts(
    run_id: i64,
    repo_owner: String,
    repo_name: String,
    state: State<AppState>,
) -> Result<Vec<serde_json::Value>, String> {
    let token_guard = state.github_token.lock().unwrap();
    let token = token_guard.as_ref().ok_or_else(|| {
        let i18n = state.i18n.lock().unwrap();
        i18n.get("not_logged_in")
    })?;

    let client = WorkflowClient::new(token.clone(), repo_owner, repo_name);
    let artifacts = client.list_artifacts(run_id).map_err(|e| e.to_string())?;

    Ok(artifacts
        .into_iter()
        .map(|art| {
            json!({
                "id": art.id,
                "name": art.name,
                "size": art.size_in_bytes,
                "created_at": art.created_at,
                "download_url": art.download_url,
            })
        })
        .collect())
}

#[tauri::command]
fn download_artifacts(
    request: DownloadRequest,
    state: State<AppState>,
    window: Window,
) -> Result<Vec<String>, String> {
    let token_guard = state.github_token.lock().unwrap();
    let token = token_guard.as_ref().ok_or_else(|| {
        let i18n = state.i18n.lock().unwrap();
        i18n.get("not_logged_in")
    })?;

    let dest_dir = PathBuf::from(&request.dest_dir);
    if !dest_dir.exists() {
        std::fs::create_dir_all(&dest_dir).map_err(|e| e.to_string())?;
    }

    let downloader = Downloader::new();
    let client = WorkflowClient::new(
        token.clone(),
        request.repo_owner.clone(),
        request.repo_name.clone(),
    );

    let mut downloaded_files = Vec::new();

    for artifact_id in request.artifact_ids {
        let artifacts = client.list_artifacts(0).map_err(|e| e.to_string())?;
        let artifact = artifacts
            .iter()
            .find(|a| a.id == artifact_id)
            .ok_or_else(|| format!("找不到产物 ID: {}", artifact_id))?;

        let filename = format!("{}.zip", artifact.name);
        let dest_path = dest_dir.join(&filename);

        let _ = downloader.download_with_callback(
            &artifact.download_url,
            &dest_dir,
            Some(&filename),
            Some(token),
            |progress, downloaded, total| {
                let _ = window.emit("download-progress", json!({
                    "artifact_id": artifact_id,
                    "progress": progress,
                    "downloaded": downloaded,
                    "total": total,
                }));
                true
            },
        ).map_err(|e| e.to_string())?;

        downloaded_files.push(dest_path.to_string_lossy().to_string());
    }

    Ok(downloaded_files)
}

#[tauri::command]
fn get_supported_languages() -> Result<Vec<String>, String> {
    Ok(vec![
        "Rust".to_string(),
        "Node.js".to_string(),
        "Python".to_string(),
        "Go".to_string(),
        "Java (Maven)".to_string(),
        "Java (Gradle)".to_string(),
        "C++".to_string(),
        "Docker".to_string(),
    ])
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .setup(|app| {
            let app_handle = app.handle();
            let app_data_dir = app_handle.path().app_data_dir().unwrap();
            std::fs::create_dir_all(&app_data_dir).unwrap();

            let db = Database::new(app_data_dir).unwrap();

            let state = app.state::<AppState>();
            let mut db_guard = state.db.lock().unwrap();
            *db_guard = Some(db);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            open_folder_dialog,
            analyze_project,
            github_login,
            github_logout,
            is_logged_in,
            set_language,
            get_current_language,
            get_string,
            trigger_build,
            get_build_history,
            get_build_status,
            get_artifacts,
            download_artifacts,
            get_supported_languages,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
