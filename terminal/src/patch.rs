use radicle_common as common;
use radicle_common::git;

use crate as term;

/// List the given commits in a table.
pub fn list_commits(commits: &[git::Commit]) -> anyhow::Result<()> {
    let mut table = term::Table::default();

    for commit in commits {
        let message = commit
            .summary_bytes()
            .unwrap_or_else(|| commit.message_bytes());
        table.push([
            term::format::secondary(common::fmt::oid(&commit.id())),
            term::format::italic(String::from_utf8_lossy(message)),
        ]);
    }
    table.render();

    Ok(())
}

/// Print commits ahead and behind.
pub fn print_commits_ahead_behind(
    repo: &git::Repository,
    left: git::Oid,
    right: git::Oid,
) -> anyhow::Result<()> {
    let (ahead, behind) = repo.graph_ahead_behind(left, right)?;

    term::info!(
        "{} commit(s) ahead, {} commit(s) behind",
        term::format::positive(ahead),
        if behind > 0 {
            term::format::negative(behind)
        } else {
            term::format::dim(behind)
        }
    );
    Ok(())
}
