use std::fs;
use zed_extension_api::{self as zed, GithubReleaseOptions, LanguageServerId, Result};

const RELEASE_REPO: &str = "abderrahimghazali/zed-dockerfile-linter";

struct DockerfileLinter {
    cached_binary_path: Option<String>,
}

struct HadolintLspBinary {
    path: String,
    environment: Option<Vec<(String, String)>>,
}

impl DockerfileLinter {
    fn language_server_binary(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<HadolintLspBinary> {
        if let Some(path) = worktree.which("hadolint-lsp") {
            return Ok(HadolintLspBinary {
                path,
                environment: Some(worktree.shell_env()),
            });
        }

        if let Some(path) = &self.cached_binary_path {
            if fs::metadata(path).is_ok_and(|stat| stat.is_file()) {
                return Ok(HadolintLspBinary {
                    path: path.clone(),
                    environment: None,
                });
            }
        }

        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::CheckingForUpdate,
        );
        let release = zed::latest_github_release(
            RELEASE_REPO,
            GithubReleaseOptions {
                require_assets: true,
                pre_release: false,
            },
        )?;

        let (platform, arch) = zed::current_platform();
        let asset_name = format!(
            "hadolint-lsp-{os}-{arch}.{ext}",
            os = match platform {
                zed::Os::Mac => "darwin",
                zed::Os::Linux => "linux",
                zed::Os::Windows => "windows",
            },
            arch = match arch {
                zed::Architecture::Aarch64 => "aarch64",
                zed::Architecture::X8664 => "x86_64",
                zed::Architecture::X86 => "i686",
            },
            ext = match platform {
                zed::Os::Windows => "zip",
                _ => "tar.gz",
            },
        );

        let asset = release
            .assets
            .iter()
            .find(|a| a.name == asset_name)
            .ok_or_else(|| {
                format!(
                    "no asset matching {asset_name:?} in release {}",
                    release.version
                )
            })?;

        let version_dir = format!("hadolint-lsp-{}", release.version);
        let bin_name = match platform {
            zed::Os::Windows => "hadolint-lsp.exe",
            _ => "hadolint-lsp",
        };
        let binary_path = format!("{version_dir}/{bin_name}");

        if !fs::metadata(&binary_path).is_ok_and(|stat| stat.is_file()) {
            zed::set_language_server_installation_status(
                language_server_id,
                &zed::LanguageServerInstallationStatus::Downloading,
            );

            zed::download_file(
                &asset.download_url,
                &version_dir,
                match platform {
                    zed::Os::Windows => zed::DownloadedFileType::Zip,
                    _ => zed::DownloadedFileType::GzipTar,
                },
            )
            .map_err(|e| format!("failed to download {asset_name}: {e}"))?;

            zed::make_file_executable(&binary_path)?;

            let entries = fs::read_dir(".").map_err(|e| format!("read_dir failed: {e}"))?;
            for entry in entries.flatten() {
                if entry.file_name().to_str() != Some(&version_dir) {
                    fs::remove_dir_all(entry.path()).ok();
                }
            }
        }

        self.cached_binary_path = Some(binary_path.clone());
        Ok(HadolintLspBinary {
            path: binary_path,
            environment: None,
        })
    }
}

impl zed::Extension for DockerfileLinter {
    fn new() -> Self {
        Self {
            cached_binary_path: None,
        }
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let bin = self.language_server_binary(language_server_id, worktree)?;
        Ok(zed::Command {
            command: bin.path,
            args: vec![],
            env: bin.environment.unwrap_or_default(),
        })
    }
}

zed::register_extension!(DockerfileLinter);
