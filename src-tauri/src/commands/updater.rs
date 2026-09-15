use std::time::Duration;

use serde::Serialize;
use tauri::{ipc::Channel, Manager, Resource, ResourceId, State, Webview};
use tauri_plugin_updater::Update;

use crate::state::app_state::AppState;

/// The verified bytes stay bound to the update that authenticated them.
struct DownloadedAppUpdate {
    update: Update,
    bytes: Vec<u8>,
}

impl Resource for DownloadedAppUpdate {}

#[derive(Clone, Serialize)]
#[serde(tag = "event", content = "data")]
pub enum AppUpdateDownloadEvent {
    #[serde(rename_all = "camelCase")]
    Started {
        content_length: Option<u64>,
    },
    #[serde(rename_all = "camelCase")]
    Progress {
        chunk_length: usize,
    },
    Finished,
}

#[derive(Debug, Serialize)]
pub struct AppUpdateInstallError {
    kind: &'static str,
    message: String,
}

/// Download and authenticate through Tauri without stopping active transfers.
#[tauri::command]
pub async fn download_app_update(
    webview: Webview,
    update_rid: ResourceId,
    on_event: Channel<AppUpdateDownloadEvent>,
) -> Result<ResourceId, String> {
    let update = webview
        .resources_table()
        .get::<Update>(update_rid)
        .map_err(|error| error.to_string())?;
    let mut update = (*update).clone();
    update.timeout = Some(Duration::from_secs(3600));
    let mut started = false;
    let bytes = update
        .download(
            |chunk_length, content_length| {
                if !started {
                    started = true;
                    let _ = on_event.send(AppUpdateDownloadEvent::Started { content_length });
                }
                let _ = on_event.send(AppUpdateDownloadEvent::Progress { chunk_length });
            },
            || {
                let _ = on_event.send(AppUpdateDownloadEvent::Finished);
            },
        )
        .await
        .map_err(|error| error.to_string())?;
    Ok(webview
        .resources_table()
        .add(DownloadedAppUpdate { update, bytes }))
}

/// This is the only permitted installation path; the frontend cannot release its gate.
#[tauri::command]
pub async fn install_app_update(
    webview: Webview,
    state: State<'_, AppState>,
    bytes_rid: ResourceId,
) -> Result<(), AppUpdateInstallError> {
    let downloaded = webview
        .resources_table()
        .get::<DownloadedAppUpdate>(bytes_rid)
        .map_err(|error| AppUpdateInstallError {
            kind: "install",
            message: error.to_string(),
        })?;
    state
        .transfer_registry
        .begin_update()
        .await
        .map_err(|message| AppUpdateInstallError {
            kind: "busy",
            message,
        })?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        downloaded
            .update
            .install(&downloaded.bytes)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())
    .and_then(|result| result);
    if let Err(error) = result {
        state.transfer_registry.end_update().await;
        return Err(AppUpdateInstallError {
            kind: "install",
            message: error,
        });
    }
    let _ = webview.resources_table().close(bytes_rid);
    // Keep the gate closed after replacement, even if the UI cannot relaunch.
    Ok(())
}
