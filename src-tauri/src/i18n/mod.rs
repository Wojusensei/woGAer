use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct I18n {
    pub lang: String,
    pub strings: HashMap<String, String>,
}

impl I18n {
    pub fn new(lang: &str) -> Self {
        let strings = match lang {
            "en" => en_strings(),
            _ => zh_strings(),
        };
        Self {
            lang: lang.to_string(),
            strings,
        }
    }

    pub fn get(&self, key: &str) -> String {
        self.strings.get(key).cloned().unwrap_or_else(|| key.to_string())
    }

    pub fn set_lang(&mut self, lang: &str) {
        self.lang = lang.to_string();
        self.strings = match lang {
            "en" => en_strings(),
            _ => zh_strings(),
        };
    }
}

fn zh_strings() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("app_name".to_string(), "woGAer".to_string());
    m.insert("app_sub".to_string(), "GitHub Actions · 液态构建工坊".to_string());
    m.insert("btn_pick".to_string(), "选择项目".to_string());
    m.insert("btn_settings".to_string(), "设置".to_string());
    m.insert("btn_history".to_string(), "历史".to_string());
    m.insert("drop_strong".to_string(), "拖拽项目文件夹至此".to_string());
    m.insert("drop_hint".to_string(), "或点击上方「选择项目」浏览".to_string());
    m.insert("desc".to_string(), "高性能 GitHub Actions 自动化打包工具".to_string());
    m.insert("version_prefix".to_string(), "版本号：".to_string());
    m.insert("folder_picker_title".to_string(), "选择项目文件夹".to_string());
    m.insert("no_folder_selected".to_string(), "未选择任何文件夹".to_string());
    m.insert("path_not_exist".to_string(), "路径不存在或不是文件夹".to_string());
    m.insert("token_invalid".to_string(), "Token 无效或权限不足".to_string());
    m.insert("not_logged_in".to_string(), "未登录 GitHub".to_string());
    m.insert("build_triggered".to_string(), "构建已触发".to_string());
    m.insert("build_failed".to_string(), "构建失败".to_string());
    m.insert("artifact_not_found".to_string(), "找不到产物".to_string());
    m.insert("download_started".to_string(), "开始下载".to_string());
    m.insert("download_complete".to_string(), "下载完成".to_string());
    m.insert("status_queued".to_string(), "排队中".to_string());
    m.insert("status_in_progress".to_string(), "进行中".to_string());
    m.insert("status_completed".to_string(), "已完成".to_string());
    m.insert("status_failed".to_string(), "失败".to_string());
    m.insert("status_cancelled".to_string(), "已取消".to_string());
    m.insert("status_unknown".to_string(), "未知状态".to_string());
    m
}

fn en_strings() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("app_name".to_string(), "woGAer".to_string());
    m.insert("app_sub".to_string(), "GitHub Actions · Liquid Build Studio".to_string());
    m.insert("btn_pick".to_string(), "Select Project".to_string());
    m.insert("btn_settings".to_string(), "Settings".to_string());
    m.insert("btn_history".to_string(), "History".to_string());
    m.insert("drop_strong".to_string(), "Drag your project folder here".to_string());
    m.insert("drop_hint".to_string(), "or click 'Select Project' above".to_string());
    m.insert("desc".to_string(), "High-performance GitHub Actions automation packaging tool".to_string());
    m.insert("version_prefix".to_string(), "version: ".to_string());
    m.insert("folder_picker_title".to_string(), "Select Project Folder".to_string());
    m.insert("no_folder_selected".to_string(), "No folder selected".to_string());
    m.insert("path_not_exist".to_string(), "Path does not exist or is not a folder".to_string());
    m.insert("token_invalid".to_string(), "Token invalid or insufficient permissions".to_string());
    m.insert("not_logged_in".to_string(), "Not logged in to GitHub".to_string());
    m.insert("build_triggered".to_string(), "Build triggered".to_string());
    m.insert("build_failed".to_string(), "Build failed".to_string());
    m.insert("artifact_not_found".to_string(), "Artifact not found".to_string());
    m.insert("download_started".to_string(), "Download started".to_string());
    m.insert("download_complete".to_string(), "Download complete".to_string());
    m.insert("status_queued".to_string(), "Queued".to_string());
    m.insert("status_in_progress".to_string(), "In progress".to_string());
    m.insert("status_completed".to_string(), "Completed".to_string());
    m.insert("status_failed".to_string(), "Failed".to_string());
    m.insert("status_cancelled".to_string(), "Cancelled".to_string());
    m.insert("status_unknown".to_string(), "Unknown".to_string());
    m
}