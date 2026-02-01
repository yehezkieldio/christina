use crate::config::Config;
use christina_git::GitRepository;
use compact_str::CompactString;

pub struct AppContextData {
    pub repo: Option<GitRepository>,
    pub config: Config,
    pub branch_name: Option<CompactString>,
}

impl AppContextData {
    /// Refresh the branch name from the repository.
    pub fn refresh_branch(&mut self) {
        self.branch_name = self.repo.as_ref().and_then(|r| {
            r.inner().head().ok().and_then(|h| {
                let name = h.shorthand()?;
                Some(CompactString::new(name))
            }) // Detached HEAD yields None
        });
    }
}
