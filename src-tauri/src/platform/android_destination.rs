use serde::{Deserialize, Serialize};
use tauri::{
    plugin::{Builder, PluginHandle, TauriPlugin},
    AppHandle, Manager, Runtime,
};

const PLUGIN_IDENTIFIER: &str = "app.dukto";

pub struct AndroidDestination<R: Runtime>(PluginHandle<R>);

impl<R: Runtime> Clone for AndroidDestination<R> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

#[derive(Debug, Deserialize)]
struct PickDirectoryResponse {
    path: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AndroidSelectedFile {
    pub is_dir: bool,
    pub name: String,
    pub path: String,
    pub size: u64,
}

#[derive(Debug, Deserialize)]
struct PickFilesResponse {
    files: Option<Vec<AndroidSelectedFile>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PickFilesRequest {
    multiple: bool,
}

#[derive(Serialize)]
struct ReleaseFilesRequest<'a> {
    paths: &'a [String],
}

impl<R: Runtime> AndroidDestination<R> {
    pub async fn pick_directory(&self) -> Result<Option<String>, String> {
        self.0
            .run_mobile_plugin_async::<PickDirectoryResponse>("pickDirectory", ())
            .await
            .map(|response| response.path)
            .map_err(|error| error.to_string())
    }

    pub async fn pick_files(
        &self,
        multiple: bool,
    ) -> Result<Option<Vec<AndroidSelectedFile>>, String> {
        self.0
            .run_mobile_plugin_async::<PickFilesResponse>(
                "pickFiles",
                PickFilesRequest { multiple },
            )
            .await
            .map(|response| response.files)
            .map_err(|error| error.to_string())
    }

    pub async fn release_files(&self, paths: &[String]) -> Result<(), String> {
        self.0
            .run_mobile_plugin_async::<()>("releaseFiles", ReleaseFilesRequest { paths })
            .await
            .map_err(|error| error.to_string())
    }

    pub async fn open_privacy_policy(&self) -> Result<(), String> {
        self.0
            .run_mobile_plugin_async::<()>("openPrivacyPolicy", ())
            .await
            .map_err(|error| error.to_string())
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("destination")
        .setup(|app: &AppHandle<R>, api| {
            let handle = api.register_android_plugin(PLUGIN_IDENTIFIER, "DestinationPlugin")?;
            app.manage(AndroidDestination(handle));
            Ok(())
        })
        .build()
}
