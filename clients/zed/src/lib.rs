mod server;

use serde_json::Value;
use zed::settings::LspSettings;
use zed_extension_api as zed;

struct LspfAnalysis;

impl zed::Extension for LspfAnalysis {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        let settings = LspSettings::for_worktree(language_server_id.as_ref(), worktree)?;
        let binary = settings.binary.unwrap_or(zed::settings::CommandSettings {
            path: None,
            arguments: None,
            env: None,
        });
        // Zed also handles this override in its host. Keep the adapter correct
        // when invoked directly, without consulting PATH or an unpinned release.
        let command = if let Some(path) = binary.path {
            path
        } else {
            let (os, arch) = zed::current_platform();
            let target = server::target(os, arch)?;
            let path = server::cache_path(target);
            server::ensure_cached(&path, |temporary| {
                zed::set_language_server_installation_status(
                    language_server_id,
                    &zed::LanguageServerInstallationStatus::Downloading,
                );
                zed::download_file(
                    &server::download_url(target),
                    &temporary.to_string_lossy(),
                    zed::DownloadedFileType::Uncompressed,
                )?;
                zed::make_file_executable(&temporary.to_string_lossy())
            })
            .map_err(|error| {
                format!(
                    "Cannot install LSPF Analysis {}: {error}. Check your network and extension \
                     download permissions, or set lsp.lspf-analysis.binary.path and \
                     binary.arguments to [\"serve\", \"--stdio\"] in Zed settings.",
                    server::version()
                )
            })?;
            zed::set_language_server_installation_status(
                language_server_id,
                &zed::LanguageServerInstallationStatus::None,
            );
            std::env::current_dir()
                .map_err(|error| error.to_string())?
                .join(path)
                .to_string_lossy()
                .into_owned()
        };
        Ok(zed::Command {
            command,
            args: binary
                .arguments
                .unwrap_or_else(|| vec!["serve".into(), "--stdio".into()]),
            env: binary.env.unwrap_or_default().into_iter().collect(),
        })
    }

    fn language_server_initialization_options(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<Option<Value>> {
        configuration(language_server_id, worktree)
    }

    fn language_server_workspace_configuration(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<Option<Value>> {
        configuration(language_server_id, worktree)
    }
}

// One source for both messages: the server replaces its configuration rather
// than merging a didChangeConfiguration notification into initializationOptions.
fn configuration(
    language_server_id: &zed::LanguageServerId,
    worktree: &zed::Worktree,
) -> zed::Result<Option<Value>> {
    let settings = LspSettings::for_worktree(language_server_id.as_ref(), worktree)?;
    server::configuration(settings.settings).map(Some)
}

zed::register_extension!(LspfAnalysis);
