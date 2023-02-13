use anyhow::Result;

use librad::git::storage::ReadOnly;

use radicle_common::cobs::issue::State;
use radicle_common::cobs::issue::*;
use radicle_common::cobs::{Comment, Reaction, Timestamp};

use radicle_common::project;

#[derive(Default)]
pub struct GroupedIssues {
    pub open: Vec<(IssueId, Issue)>,
    pub closed: Vec<(IssueId, Issue)>,
}

impl From<&GroupedIssues> for Vec<(IssueId, Issue)> {
    fn from(groups: &GroupedIssues) -> Self {
        [groups.open.clone(), groups.closed.clone()].concat()
    }
}

impl From<&Vec<(IssueId, Issue)>> for GroupedIssues {
    fn from(issues: &Vec<(IssueId, Issue)>) -> Self {
        let mut open = issues.clone();
        let mut closed = issues.clone();

        open.retain(|(_, issue)| issue.state() == State::Open);
        closed.retain(|(_, issue)| issue.state() != State::Open);

        Self {
            open: open,
            closed: closed,
        }
    }
}

#[derive(Clone)]
pub enum WrappedComment<R> {
    Root { comment: Comment<()> },
    Reply { comment: Comment<R> },
}

impl<R> WrappedComment<R> {
    pub fn author_info(&self) -> (String, String, Vec<(&Reaction, &usize)>, Timestamp, u16) {
        let (author, body, reactions, timestamp, indent) = match self {
            WrappedComment::Root { comment } => (
                comment.author.name(),
                comment.body.clone(),
                comment.reactions.iter().collect::<Vec<_>>(),
                comment.timestamp,
                0,
            ),
            WrappedComment::Reply { comment } => (
                comment.author.name(),
                comment.body.clone(),
                comment.reactions.iter().collect::<Vec<_>>(),
                comment.timestamp,
                4,
            ),
        };

        (author, body, reactions, timestamp, indent)
    }
}

pub fn load<S: AsRef<ReadOnly>>(
    storage: &S,
    metadata: &project::Metadata,
    store: &IssueStore,
) -> Result<Vec<(IssueId, Issue)>> {
    let mut issues = store.all(&metadata.urn)?;
    resolve(storage, &mut issues);

    Ok(issues)
}

pub fn resolve<S: AsRef<ReadOnly>>(storage: &S, issues: &mut Vec<(IssueId, Issue)>) {
    let _ = issues
        .iter_mut()
        .map(|(_, issue)| issue.resolve(&storage).ok())
        .collect::<Vec<_>>();
}
