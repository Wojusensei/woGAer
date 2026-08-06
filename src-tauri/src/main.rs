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
use tauri::{AppHandle, Emitter, Manager, State, Window};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use db::{Database, BuildRecord};
use git::GitRepo;
use actions::workflow::{WorkflowClient, WorkflowDispatchOptions, WorkflowRun};
use downloader::Downloader;
use i18n::I18n;

async fn run_blocking<F, T>(task: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String> + Send + 'static,
    T: Send + 'static,
{
    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let _ = tx.send(task());
    });
    rx.await.map_err(|_| "后台任务未返回结果".to_string())?
}

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
    pub repo_owner: Option<String>,
    pub repo_name: Option<String>,
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
    pub run_id: i64,
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
    let (repo_owner, repo_name) = status
        .remote_url
        .as_deref()
        .map(git::repo::parse_remote)
        .unwrap_or((None, None));

    Ok(ProjectAnalysis {
        is_git_repo: status.is_repo,
        has_remote: status.has_remote,
        remote_url: status.remote_url,
        repo_owner,
        repo_name,
        current_branch: status.current_branch,
        is_clean: status.is_clean,
        languages,
        path: project_path,
    })
}

#[tauri::command]
async fn github_login(token: String, state: State<'_, AppState>) -> Result<bool, String> {
    let token_for_check = token.clone();
    let valid = run_blocking(move || {
        let client = WorkflowClient::new(token_for_check, String::new(), String::new());
        client.validate_token().map_err(|e| e.to_string())
    })
    .await?;

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
fn set_github_token(token: String, state: State<AppState>) -> Result<(), String> {
    let mut token_guard = state.github_token.lock().unwrap();
    *token_guard = if token.trim().is_empty() {
        None
    } else {
        Some(token.trim().to_string())
    };
    Ok(())
}

#[tauri::command]
fn is_logged_in(state: State<AppState>) -> Result<bool, String> {
    let token_guard = state.github_token.lock().unwrap();
    Ok(token_guard.is_some())
}

#[tauri::command]
async fn get_github_user(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let token = state.github_token.lock().unwrap().clone().ok_or_else(|| {
        let i18n = state.i18n.lock().unwrap();
        i18n.get("not_logged_in")
    })?;

    run_blocking(move || {
        let client = WorkflowClient::new(token, String::new(), String::new());
        client.get_user().map_err(|e| e.to_string())
    })
    .await
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
async fn trigger_build(request: BuildRequest, state: State<'_, AppState>) -> Result<BuildResponse, String> {
    let token = state.github_token.lock().unwrap().clone().ok_or_else(|| {
        let i18n = state.i18n.lock().unwrap();
        i18n.get("not_logged_in")
    })?;

    let token_for_build = token;
    let repo_owner = request.repo_owner.clone();
    let repo_name = request.repo_name.clone();
    let workflow_id = request.workflow_id.clone();

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

    let run = run_blocking(move || -> Result<Option<WorkflowRun>, String> {
        let client = WorkflowClient::new(token_for_build, repo_owner, repo_name);
        if let Err(e) = client.validate_repo_access() {
            let msg = e.to_string();
            return Err(if msg.contains("404") {
                "仓库不存在、无访问权限，或仓库为私有且 Token 无权限".to_string()
            } else if msg.contains("403") {
                "Token 权限不足，请确保勾选了 workflow 权限".to_string()
            } else {
                msg
            });
        }
        client.dispatch_workflow(&workflow_id, &options).map_err(|e| {
            let msg = e.to_string();
            if msg.contains("404") {
                "workflow 文件不存在，请先生成 workflow 并推送到仓库".to_string()
            } else if msg.contains("422") {
                "触发参数无效，请检查分支名或 inputs".to_string()
            } else if msg.contains("403") {
                "Token 权限不足，请确保勾选了 workflow 权限".to_string()
            } else {
                msg
            }
        })?;

        std::thread::sleep(std::time::Duration::from_secs(2));
        let runs = client
            .list_runs(Some(&workflow_id), 1)
            .map_err(|e| e.to_string())?;
        Ok(runs.into_iter().next())
    })
    .await?;

    let run = match run {
        Some(run) => run,
        None => {
            let i18n = state.i18n.lock().unwrap();
            return Err(i18n.get("build_failed"));
        }
    };

    if let Some(db) = state.db.lock().unwrap().as_ref() {
        let record = BuildRecord {
            id: 0,
            run_id: run.id,
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
}

#[tauri::command]
fn get_download_dir(app: AppHandle) -> Result<String, String> {
    let base = app
        .path()
        .download_dir()
        .or_else(|_| app.path().app_data_dir())
        .map_err(|_| "无法获取下载目录".to_string())?;
    Ok(base.join("woGAer").to_string_lossy().to_string())
}

#[tauri::command]
fn reveal_path(path: String, app: AppHandle) -> Result<(), String> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err("路径不存在".to_string());
    }
    app.opener().reveal_item_in_dir(&p).map_err(|e| e.to_string())
}

#[tauri::command]
fn open_url(url: String, app: AppHandle) -> Result<(), String> {
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
async fn device_flow_start(client_id: String) -> Result<serde_json::Value, String> {
    run_blocking(move || -> Result<serde_json::Value, String> {
        let http = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?;
        let resp = http
            .post("https://github.com/login/device/code")
            .header("Accept", "application/json")
            .form(&[("client_id", client_id.as_str()), ("scope", "workflow")])
            .send()
            .map_err(|e| format!("请求失败: {}", e))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(format!("设备码请求失败: {} - {}", status, text));
        }
        let data: serde_json::Value = resp.json().map_err(|e| e.to_string())?;
        Ok(json!({
            "device_code": data["device_code"].as_str().unwrap_or(""),
            "user_code": data["user_code"].as_str().unwrap_or(""),
            "verification_uri": data["verification_uri"].as_str().unwrap_or("https://github.com/login/device"),
            "interval": data["interval"].as_u64().unwrap_or(5),
            "expires_in": data["expires_in"].as_u64().unwrap_or(900),
        }))
    })
    .await
}

#[tauri::command]
async fn device_flow_poll(client_id: String, device_code: String) -> Result<serde_json::Value, String> {
    run_blocking(move || -> Result<serde_json::Value, String> {
        let http = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?;
        let resp = http
            .post("https://github.com/login/oauth/access_token")
            .header("Accept", "application/json")
            .form(&[
                ("client_id", client_id.as_str()),
                ("device_code", device_code.as_str()),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .map_err(|e| format!("请求失败: {}", e))?;
        let data: serde_json::Value = resp.json().map_err(|e| e.to_string())?;
        if let Some(token) = data["access_token"].as_str() {
            return Ok(json!({ "status": "ok", "token": token }));
        }
        let error = data["error"].as_str().unwrap_or("unknown_error");
        let interval = data["interval"].as_u64().unwrap_or(5);
        let status = match error {
            "authorization_pending" => "pending",
            "slow_down" => "slow_down",
            "expired_token" => "expired",
            "access_denied" => "denied",
            _ => "error",
        };
        Ok(json!({ "status": status, "interval": interval, "error": error }))
    })
    .await
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
async fn get_build_status(
    run_id: i64,
    repo_owner: String,
    repo_name: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let token = state.github_token.lock().unwrap().clone().ok_or_else(|| {
        let i18n = state.i18n.lock().unwrap();
        i18n.get("not_logged_in")
    })?;

    let run = run_blocking(move || {
        let client = WorkflowClient::new(token, repo_owner, repo_name);
        client.get_run_status(run_id).map_err(|e| e.to_string())
    })
    .await?;

    Ok(json!({
        "id": run.id,
        "status": run.status,
        "conclusion": run.conclusion,
        "updated_at": run.updated_at,
        "html_url": run.html_url,
    }))
}

#[tauri::command]
async fn get_artifacts(
    run_id: i64,
    repo_owner: String,
    repo_name: String,
    state: State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, String> {
    let token = state.github_token.lock().unwrap().clone().ok_or_else(|| {
        let i18n = state.i18n.lock().unwrap();
        i18n.get("not_logged_in")
    })?;

    let artifacts = run_blocking(move || {
        let client = WorkflowClient::new(token, repo_owner, repo_name);
        client.list_artifacts(run_id).map_err(|e| e.to_string())
    })
    .await?;

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
async fn download_artifacts(
    request: DownloadRequest,
    state: State<'_, AppState>,
    window: Window,
) -> Result<Vec<String>, String> {
    let token = state.github_token.lock().unwrap().clone().ok_or_else(|| {
        let i18n = state.i18n.lock().unwrap();
        i18n.get("not_logged_in")
    })?;

    let dest_dir = PathBuf::from(&request.dest_dir);
    if !dest_dir.exists() {
        std::fs::create_dir_all(&dest_dir).map_err(|e| e.to_string())?;
    }

    run_blocking(move || {
        let downloader = Downloader::new();
        let client = WorkflowClient::new(token.clone(), request.repo_owner, request.repo_name);
        let mut downloaded_files = Vec::new();

        for artifact_id in request.artifact_ids {
            let artifacts = client.list_artifacts(request.run_id).map_err(|e| e.to_string())?;
            let artifact = artifacts
                .iter()
                .find(|a| a.id == artifact_id)
                .ok_or_else(|| format!("找不到产物 ID: {}", artifact_id))?;

            let filename = format!("{}.zip", artifact.name);
            let dest_path = dest_dir.join(&filename);

            downloader.download_with_callback(
                &artifact.download_url,
                &dest_dir,
                Some(&filename),
                Some(&token),
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
    })
    .await
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

#[tauri::command]
async fn generate_workflow(
    project_path: String,
    language: String,
    project_name: String,
) -> Result<String, String> {
    use std::fs;
    use std::path::PathBuf;

    let path = PathBuf::from(&project_path);
    let workflow_dir = path.join(".github").join("workflows");
    fs::create_dir_all(&workflow_dir).map_err(|e| e.to_string())?;

    let content = actions::workflow::WorkflowTemplate::generate(&language, &project_name);
    let file_path = workflow_dir.join("build.yml");
    fs::write(&file_path, content).map_err(|e| e.to_string())?;

    Ok(file_path.to_string_lossy().to_string())
}

#[tauri::command]
async fn push_workflow(project_path: String, state: State<'_, AppState>) -> Result<String, String> {
    let token = state
        .github_token
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| "未登录 GitHub".to_string())?;
    let path = PathBuf::from(&project_path);
    let repo = GitRepo::new(path.clone());
    let status = repo.status().map_err(|e| e.to_string())?;
    let remote_url = status
        .remote_url
        .ok_or_else(|| "项目没有配置远程仓库".to_string())?;
    let (owner, name) = git::repo::parse_remote(&remote_url);
    let owner = owner.ok_or_else(|| "无法解析远程仓库 owner".to_string())?;
    let name = name.ok_or_else(|| "无法解析远程仓库名称".to_string())?;
    let branch = status
        .current_branch
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "main".to_string());
    let workflow_path = path.join(".github").join("workflows").join("build.yml");
    let bytes = std::fs::read(&workflow_path).map_err(|e| format!("读取 workflow 失败: {}", e))?;
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let content = STANDARD.encode(&bytes);

    run_blocking(move || {
        let client = WorkflowClient::new(token, owner, name);
        client
            .push_workflow_file(&branch, "build.yml", &content)
            .map_err(|e| e.to_string())?;
        Ok("Workflow 已推送到 GitHub".to_string())
    })
    .await
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
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
            generate_workflow,
            push_workflow,
            set_github_token,
            get_download_dir,
            get_github_user,
            reveal_path,
            open_url,
            device_flow_start,
            device_flow_poll,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
