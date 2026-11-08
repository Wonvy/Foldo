//! Independent regex workbenches, bound to the condition that opened each window.
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Mutex};
use tauri::{Emitter, Manager};

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    id: String,
    pattern: String,
    case_sensitive: bool,
    can_apply: bool,
}

#[derive(Default)]
pub struct Sessions(Mutex<HashMap<String, Session>>);

fn label(id: &str) -> Result<String, String> {
    if id.len() > 64 || id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err("Invalid regex session".into());
    }
    Ok(format!("regex-{id}"))
}

#[tauri::command]
pub async fn open_regex_lab(app: tauri::AppHandle, session: Session) -> Result<(), String> {
    let name = label(&session.id)?;
    if let Some(window) = app.get_webview_window(&name) {
        window.show().map_err(|e| e.to_string())?;
        return window.set_focus().map_err(|e| e.to_string());
    }
    app.state::<Sessions>()
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .insert(session.id.clone(), session.clone());
    let result = tauri::WebviewWindowBuilder::new(
        &app,
        &name,
        tauri::WebviewUrl::App(format!("index.html#regex/{}", session.id).into()),
    )
    .title(foldo_core::t!("Foldo — 正则助手"))
    .inner_size(1060.0, 780.0)
    .min_inner_size(760.0, 560.0)
    .resizable(true)
    .build();
    if let Err(error) = result {
        app.state::<Sessions>()
            .0
            .lock()
            .map_err(|e| e.to_string())?
            .remove(&session.id);
        return Err(error.to_string());
    }
    Ok(())
}

#[tauri::command]
pub async fn regex_session(app: tauri::AppHandle, id: String) -> Result<Session, String> {
    app.state::<Sessions>()
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .get(&id)
        .cloned()
        .ok_or_else(|| "Regex session closed".into())
}

#[tauri::command]
pub async fn apply_regex_lab(
    app: tauri::AppHandle,
    id: String,
    pattern: String,
    case_sensitive: bool,
) -> Result<(), String> {
    let mut session = regex_session(app.clone(), id.clone()).await?;
    if !session.can_apply {
        return Err("No target condition".into());
    }
    foldo_core::regex_debug::test(&pattern, "", case_sensitive)?;
    session.pattern = pattern;
    session.case_sensitive = case_sensitive;
    app.emit_to("main", &format!("regex-result-{id}"), session)
        .map_err(|e| e.to_string())?;
    close_regex_lab(app, id).await
}

#[tauri::command]
pub async fn close_regex_lab(app: tauri::AppHandle, id: String) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(&label(&id)?) {
        window.close().map_err(|e| e.to_string())?;
    }
    forget(&app, &format!("regex-{id}"));
    Ok(())
}

pub fn forget(app: &tauri::AppHandle, name: &str) {
    if let Some(id) = name.strip_prefix("regex-")
        && let Ok(mut sessions) = app.state::<Sessions>().0.lock()
    {
        sessions.remove(id);
    }
}

#[derive(Serialize)]
pub struct Samples {
    names: Vec<String>,
    ignored: usize,
}

#[tauri::command]
pub async fn regex_file_samples(paths: Vec<String>) -> Samples {
    let mut names = Vec::new();
    let mut bytes = 0;
    let mut ignored = 0;
    for path in paths {
        let path = std::path::Path::new(&path);
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let valid = path
            .symlink_metadata()
            .is_ok_and(|m| m.is_file() || m.is_dir());
        if !valid
            || name.is_empty()
            || name.contains(['\n', '\r'])
            || names.len() >= 100
            || bytes + name.len() + 1 > 65536
        {
            ignored += 1;
        } else if !names.iter().any(|n| n == name) {
            names.push(name.to_owned());
            bytes += name.len() + 1;
        }
    }
    Samples { names, ignored }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sample_import_uses_only_names_and_ignores_missing_paths() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("测试[001].png");
        std::fs::write(&file, "unchanged").unwrap();
        let folder = root.path().join("2026-02-19_洞洞板");
        std::fs::create_dir(&folder).unwrap();
        let paths = [file.clone(), folder, root.path().join("missing")]
            .map(|p| p.to_string_lossy().into_owned())
            .to_vec();
        let samples = tauri::async_runtime::block_on(regex_file_samples(paths));
        assert_eq!(samples.names, ["测试[001].png", "2026-02-19_洞洞板"]);
        assert_eq!(samples.ignored, 1);
        assert_eq!(std::fs::read_to_string(file).unwrap(), "unchanged");
    }
    #[test]
    fn session_labels_cannot_escape_the_regex_namespace() {
        assert!(label("abc-123").is_ok());
        for bad in ["", "../main", "main#float", "a/b"] {
            assert!(label(bad).is_err());
        }
    }
}
