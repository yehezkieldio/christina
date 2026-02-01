use crate::config::Config;
use compact_str::CompactString;

pub struct AppContextData {
    pub repo: Option<git2::Repository>,
    pub config: Config,
    pub branch_name: Option<CompactString>,
}

impl AppContextData {
    /// Refresh the branch name from the repository.
    pub fn refresh_branch(&mut self) {
        self.branch_name = self.repo.as_ref().and_then(|r| {
            r.head().ok().and_then(|h| {
                if !h.is_branch() {
                    return None;
                }
                let name = h.shorthand()?;
                Some(CompactString::new(name))
            })
        });
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use christina_core::test_helpers::TempRepo;

    #[test]
    fn test_refresh_branch_with_branch() {
        let temp_repo = TempRepo::new();
        temp_repo.commit_file("README.md", "# Test");

        let repo = git2::Repository::open(temp_repo.path()).unwrap();
        let mut context = AppContextData {
            repo: Some(repo),
            config: Config::default(),
            branch_name: None,
        };

        context.refresh_branch();

        assert!(context.branch_name.is_some());
        // Default branch depends on Git config, but should be something like "main" or "master"
        let branch = context.branch_name.unwrap();
        assert!(!branch.is_empty());
    }

    #[test]
    fn test_refresh_branch_detached_head() {
        let temp_repo = TempRepo::new();
        let oid = temp_repo.commit_file("README.md", "# Test");

        let mut context = AppContextData {
            repo: Some(git2::Repository::open(temp_repo.path()).unwrap()),
            config: Config::default(),
            branch_name: Some(CompactString::new("main")),
        };

        {
            let repo = context.repo.as_ref().unwrap();
            repo.set_head_detached(oid).unwrap();
            let commit = repo.find_commit(oid).unwrap();
            repo.checkout_tree(commit.as_object(), None).unwrap();
        }

        context.refresh_branch();

        assert!(context.branch_name.is_none());
    }

    #[test]
    fn test_refresh_branch_no_repo() {
        let mut context = AppContextData {
            repo: None,
            config: Config::default(),
            branch_name: Some(CompactString::new("main")),
        };

        // Should not panic when repo is None
        context.refresh_branch();

        // Should clear branch name when repo is None
        assert!(context.branch_name.is_none());
    }
}
