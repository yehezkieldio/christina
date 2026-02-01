use christina_core::app::cmd::{Cmd, ToastSeverity};
use christina_core::app::msg::Msg;
use crate::app::context::AppContextData;

/// Execute a command and return messages to feed back into the system
pub async fn execute_cmd(cmd: Cmd, ctx: &AppContextData) -> anyhow::Result<Vec<Msg>> {
    match cmd {
        Cmd::RefreshGitStatus => {
            let repo = ctx.repo.as_ref().ok_or_else(|| {
                anyhow::anyhow!("No git repository available")
            })?;

            // Get git file information
            let files = repo.get_all_files()?;
            
            // Get staged and unstaged file paths
            let staged: Vec<String> = files
                .iter()
                .filter(|f| f.is_staged())
                .map(|f| f.path.to_string())
                .collect();
                
            let unstaged: Vec<String> = files
                .iter()
                .filter(|f| !f.is_staged())
                .map(|f| f.path.to_string())
                .collect();

            // Get branch name
            let branch = ctx.branch_name
                .as_ref()
                .map(|s| s.to_string())
                .unwrap_or_else(|| "HEAD".to_string());

            Ok(vec![Msg::GitStatusRefreshed {
                files,
                staged,
                unstaged,
                branch,
            }])
        }
        
        Cmd::StageFiles { paths } => {
            let repo = ctx.repo.as_ref().ok_or_else(|| {
                anyhow::anyhow!("No git repository available")
            })?;

            // Convert FilePath to PathBuf with status for staging
            let files_with_status: Vec<_> = paths
                .iter()
                .map(|path| {
                    let path_buf = std::path::PathBuf::from(path.as_str());
                    // We don't know the exact status, but stage_files will handle it
                    (path_buf, christina_core::GitFileStatus::Modified)
                })
                .collect();

            repo.stage_files(&files_with_status)?;
            
            Ok(vec![Msg::FilesStaged { paths }])
        }
        
        Cmd::UnstageFiles { paths } => {
            let repo = ctx.repo.as_ref().ok_or_else(|| {
                anyhow::anyhow!("No git repository available")
            })?;

            // Convert FilePath to PathBuf
            let path_bufs: Vec<_> = paths
                .iter()
                .map(|path| std::path::PathBuf::from(path.as_str()))
                .collect();

            repo.unstage_files(&path_bufs)?;
            
            Ok(vec![Msg::FilesUnstaged { paths }])
        }
        
        Cmd::CommitMessage { message } => {
            let repo = ctx.repo.as_ref().ok_or_else(|| {
                anyhow::anyhow!("No git repository available")
            })?;

            repo.create_commit(&message)?;
            
            // No messages to return - the caller should refresh status
            Ok(vec![])
        }
        
        Cmd::StartGeneration => {
            // Generation is handled by the event loop
            // This is a no-op at the command execution level
            Ok(vec![])
        }
        
        Cmd::CancelGeneration => {
            // Cancellation is handled by the event loop
            // This is a no-op at the command execution level
            Ok(vec![])
        }
        
        Cmd::ShowToast { message, severity } => {
            // Toast display is handled by the UI layer
            // This is a no-op at the command execution level
            Ok(vec![])
        }
        
        Cmd::NavigateTo { state } => {
            Ok(vec![Msg::NavigateTo { state }])
        }
        
        Cmd::Quit => {
            Ok(vec![Msg::Quit])
        }
    }
}
