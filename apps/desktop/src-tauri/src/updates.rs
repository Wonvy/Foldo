//! Keep GitHub's asset API available when its release-download gateway fails.
use serde::Deserialize;
use std::time::Duration;
use tauri_plugin_updater::{Update, UpdaterExt};

const REPO: &str = "Wonvy/Foldo";
const CHECK_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    id: u64,
    name: String,
    size: u64,
    state: String,
    browser_download_url: String,
}

fn asset_url(release: &Release, asset: &Asset) -> Result<reqwest::Url, String> {
    let expected = format!(
        "https://github.com/{REPO}/releases/download/{}/{}",
        release.tag_name, asset.name
    );
    if release.draft
        || release.prerelease
        || asset.id == 0
        || asset.size == 0
        || asset.state != "uploaded"
        || asset.browser_download_url != expected
    {
        return Err("Invalid published update asset".into());
    }
    reqwest::Url::parse(&format!(
        "https://api.github.com/repos/{REPO}/releases/assets/{}",
        asset.id
    ))
    .map_err(|e| e.to_string())
}

async fn release(version: Option<&str>) -> Result<Release, String> {
    let suffix = version.map_or_else(|| "latest".into(), |v| format!("tags/v{v}"));
    reqwest::Client::builder()
        .user_agent(concat!("Foldo/", env!("CARGO_PKG_VERSION")))
        .timeout(CHECK_TIMEOUT)
        .build()
        .map_err(|e| e.to_string())?
        .get(format!(
            "https://api.github.com/repos/{REPO}/releases/{suffix}"
        ))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}

pub async fn check<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    download_timeout: Duration,
) -> Result<Option<Update>, String> {
    let primary = app
        .updater_builder()
        .timeout(CHECK_TIMEOUT)
        .build()
        .map_err(|e| e.to_string())?
        .check()
        .await;
    let mut update = match primary {
        Ok(update) => update,
        Err(_) => {
            let release = release(None).await?;
            let asset = release
                .assets
                .iter()
                .find(|asset| asset.name == "latest.json")
                .ok_or("Published update manifest is missing")?;
            app.updater_builder()
                .endpoints(vec![asset_url(&release, asset)?])
                .map_err(|e| e.to_string())?
                .header("Accept", "application/octet-stream")
                .map_err(|e| e.to_string())?
                .timeout(CHECK_TIMEOUT)
                .build()
                .map_err(|e| e.to_string())?
                .check()
                .await
                .map_err(|e| e.to_string())?
        }
    };
    if let Some(update) = &mut update {
        update.timeout = Some(download_timeout);
    }
    Ok(update)
}

fn can_retry_download(error: &tauri_plugin_updater::Error, url: &reqwest::Url) -> bool {
    url.host_str() == Some("github.com")
        && matches!(
            error,
            tauri_plugin_updater::Error::Network(_) | tauri_plugin_updater::Error::Reqwest(_)
        )
}

pub async fn download(
    update: &mut Update,
    mut progress: impl FnMut(u64, Option<u64>) + Send,
) -> Result<Vec<u8>, String> {
    for attempt in 0..2 {
        let mut downloaded = 0;
        progress(0, None);
        let result = update
            .download(
                |chunk, total| {
                    downloaded += chunk as u64;
                    progress(downloaded, total);
                },
                || {},
            )
            .await;
        match result {
            Ok(bytes) => return Ok(bytes), // Tauri has verified the package signature.
            Err(error) if attempt == 0 && can_retry_download(&error, &update.download_url) => {
                let release = release(Some(&update.version)).await?;
                if release.tag_name != format!("v{}", update.version) {
                    return Err("Published update version does not match".into());
                }
                let asset = release
                    .assets
                    .iter()
                    .find(|asset| asset.browser_download_url == update.download_url.as_str())
                    .ok_or("Published update package is missing")?;
                update.download_url = asset_url(&release, asset)?;
            }
            Err(error) => return Err(error.to_string()),
        }
    }
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_complete_official_assets_are_accepted() {
        let mut release: Release = serde_json::from_value(serde_json::json!({
            "tag_name": "v0.2.1", "draft": false, "prerelease": false,
            "assets": [{"id": 123, "name": "latest.json", "size": 100, "state": "uploaded",
                "browser_download_url": "https://github.com/Wonvy/Foldo/releases/download/v0.2.1/latest.json"}]
        })).unwrap();
        assert_eq!(
            asset_url(&release, &release.assets[0]).unwrap().as_str(),
            "https://api.github.com/repos/Wonvy/Foldo/releases/assets/123"
        );
        release.draft = true;
        assert!(asset_url(&release, &release.assets[0]).is_err());
        release.draft = false;
        release.assets[0].size = 0;
        assert!(asset_url(&release, &release.assets[0]).is_err());
        release.assets[0].size = 100;
        release.assets[0].browser_download_url = "https://example.com/latest.json".into();
        assert!(asset_url(&release, &release.assets[0]).is_err());
    }

    #[test]
    fn signature_failures_and_api_urls_are_not_retried() {
        let direct = "https://github.com/Wonvy/Foldo/releases/download/v1/package.exe"
            .parse()
            .unwrap();
        let api = "https://api.github.com/repos/Wonvy/Foldo/releases/assets/123"
            .parse()
            .unwrap();
        let network = tauri_plugin_updater::Error::Network("504 Gateway Timeout".into());
        assert!(can_retry_download(&network, &direct));
        assert!(!can_retry_download(&network, &api));
        let signature = tauri_plugin_updater::Error::SignatureUtf8("invalid signature".into());
        assert!(!can_retry_download(&signature, &direct));
    }
}
