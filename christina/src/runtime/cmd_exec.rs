use christina_core::app::cmd::{Cmd, ToastSeverity};
use christina_core::app::msg::Msg;
use crate::app::context::AppContextData;
use crate::io::git::adapter;

/// Execute a command and return messages to feed back into the system
pub async fn execute_cmd(cmd: Cmd, ctx: &AppContextData) -> anyhow::Result<Vec<Msg>> {
    match cmd {
        Cmd::RefreshGitStatus => {
            let snapshot = adapter::status()?;
            Ok(vec![Msg::GitStatusRefreshed {
                files: snapshot.files,
                staged: snapshot.staged,
                unstaged: snapshot.unstaged,
                branch: snapshot.branch,
            }])
        }
        
        Cmd::StageFiles { paths } => {
            let repo = ctx.repo.as_ref().ok_or_else(|| {
                anyhow::anyhow!("No git repository available")
            })?;

            let path_strings: Vec<String> =
                paths.iter().map(|path| path.as_str().to_string()).collect();
            adapter::stage_files(repo, &path_strings)?;
            
            Ok(vec![Msg::FilesStaged { paths }])
        }
        
        Cmd::UnstageFiles { paths } => {
            let repo = ctx.repo.as_ref().ok_or_else(|| {
                anyhow::anyhow!("No git repository available")
            })?;

            let path_strings: Vec<String> =
                paths.iter().map(|path| path.as_str().to_string()).collect();
            adapter::unstage_files(repo, &path_strings)?;
            
            Ok(vec![Msg::FilesUnstaged { paths }])
        }
        
        Cmd::CommitMessage { message } => {
            let repo = ctx.repo.as_ref().ok_or_else(|| {
                anyhow::anyhow!("No git repository available")
            })?;

            adapter::create_commit(repo, message.as_ref())?;
            
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
