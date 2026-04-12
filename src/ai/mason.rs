use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use flate2::read::GzDecoder;
use tokio::sync::mpsc;
use futures_util::StreamExt;
use std::os::unix::fs::PermissionsExt;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LspServiceStatus {
    NotInstalled,
    Downloading(f32),
    Installed,
    Running,
    Paused,
    Error(String),
}

#[derive(Debug)]
pub enum MasonEvent {
    Progress(String, f32),
    Complete(String),
    Error(String, String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspExtension {
    pub name: String,
    pub binary_name: String,
    pub github_repo: String,
    pub status: LspServiceStatus,
    pub supported_extensions: Vec<String>,
}

pub struct MasonManager {
    pub registry: HashMap<String, LspExtension>,
    base_path: PathBuf,
}

impl MasonManager {
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let base_path = home.join(".z3n").join("mason");
        
        let mut registry = HashMap::new();
        
        // Default Registry: Rust Analyzer
        registry.insert("rust-analyzer".to_string(), LspExtension {
            name: "rust-analyzer".to_string(),
            binary_name: "rust-analyzer".to_string(),
            github_repo: "rust-lang/rust-analyzer".to_string(),
            status: LspServiceStatus::NotInstalled,
            supported_extensions: vec!["rs".to_string()],
        });

        // Default Registry: Pyright (Python)
        registry.insert("pyright".to_string(), LspExtension {
            name: "pyright".to_string(),
            binary_name: "pyright-langserver".to_string(),
            github_repo: "microsoft/pyright".to_string(),
            status: LspServiceStatus::NotInstalled,
            supported_extensions: vec!["py".to_string()],
        });

        Self { registry, base_path }
    }

    pub fn get_status(&self, name: &str) -> LspServiceStatus {
        self.registry.get(name).map(|e| e.status.clone()).unwrap_or(LspServiceStatus::NotInstalled)
    }

    pub fn set_status(&mut self, name: &str, status: LspServiceStatus) {
        if let Some(ext) = self.registry.get_mut(name) {
            ext.status = status;
        }
    }

    pub fn binary_path(&self, name: &str) -> Option<PathBuf> {
        self.registry.get(name).map(|e| self.base_path.join("bin").join(&e.binary_name))
    }

    pub fn trigger_install(&self, name: String, sender: mpsc::UnboundedSender<MasonEvent>) {
        if let Some(ext) = self.registry.get(&name) {
            let repo = ext.github_repo.clone();
            let bin_name = ext.binary_name.clone();
            let name_clone = name.clone();
            let base_path = self.base_path.clone();

            tokio::spawn(async move {
                let client = reqwest::Client::builder()
                    .user_agent("Z3N-Editor-Mason")
                    .build()
                    .unwrap();

                // 1. Fetch latest release info
                let release_url = format!("https://api.github.com/repos/{}/releases/latest", repo);
                let response = client.get(&release_url).send().await;

                if let Err(e) = response {
                    let _ = sender.send(MasonEvent::Error(name_clone, e.to_string()));
                    return;
                }

                let release_json: serde_json::Value = response.unwrap().json().await.unwrap();
                let assets = release_json["assets"].as_array();

                if assets.is_none() {
                    let _ = sender.send(MasonEvent::Error(name_clone, "No release assets found".to_string()));
                    return;
                }

                // 2. Identify correct asset for Linux x86_64
                let mut download_url = None;
                for asset in assets.unwrap() {
                    let asset_name = asset["name"].as_str().unwrap_or("");
                    if asset_name.contains("linux") && (asset_name.contains("x86_64") || asset_name.contains("amd64")) {
                        download_url = Some(asset["browser_download_url"].as_str().unwrap().to_string());
                        break;
                    }
                }

                let url = if let Some(u) = download_url { u } else {
                    let _ = sender.send(MasonEvent::Error(name_clone, "No Linux x86_64 asset found in release".to_string()));
                    return;
                };

                // 3. Download Binary Stream
                let response = client.get(&url).send().await;
                if let Err(e) = response {
                    let _ = sender.send(MasonEvent::Error(name_clone, e.to_string()));
                    return;
                }

                let res = response.unwrap();
                let total_size = res.content_length().unwrap_or(1);
                let mut downloaded: u64 = 0;
                let mut buffer = Vec::new();
                let mut stream = res.bytes_stream();

                while let Some(item) = stream.next().await {
                    let chunk = match item {
                        Ok(c) => c,
                        Err(e) => {
                            let _ = sender.send(MasonEvent::Error(name_clone, e.to_string()));
                            return;
                        }
                    };
                    downloaded += chunk.len() as u64;
                    buffer.extend_from_slice(&chunk);

                    let progress = (downloaded as f32 / total_size as f32).min(1.0);
                    let _ = sender.send(MasonEvent::Progress(name_clone.clone(), progress));
                }

                // 4. Save & Decompress if needed
                let bin_dir = base_path.join("bin");
                let _ = fs::create_dir_all(&bin_dir);
                let out_path = bin_dir.join(&bin_name);

                if url.ends_with(".gz") {
                    let mut decoder = GzDecoder::new(&buffer[..]);
                    let mut decompressed = Vec::new();
                    if let Err(e) = decoder.read_to_end(&mut decompressed) {
                        let _ = sender.send(MasonEvent::Error(name_clone, format!("Gzip extraction failed: {}", e)));
                        return;
                    }
                    if let Err(e) = fs::write(&out_path, decompressed) {
                        let _ = sender.send(MasonEvent::Error(name_clone, format!("Failed to save binary: {}", e)));
                        return;
                    }
                } else {
                    if let Err(e) = fs::write(&out_path, buffer) {
                        let _ = sender.send(MasonEvent::Error(name_clone, format!("Failed to save binary: {}", e)));
                        return;
                    }
                }

                // 5. Set Execution Permissions
                let _ = fs::set_permissions(&out_path, fs::Permissions::from_mode(0o755));

                let _ = sender.send(MasonEvent::Complete(name_clone));
            });
        }
    }
}
