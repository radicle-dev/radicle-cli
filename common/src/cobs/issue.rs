#![allow(clippy::large_enum_variant)]
use std::collections::{HashMap, HashSet};
use std::convert::{TryFrom, TryInto};
use std::ops::{ControlFlow, Deref};
use std::str::FromStr;

use automerge::{Automerge, AutomergeError, ObjType, ScalarValue, Value};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

use librad::collaborative_objects::{
    CollaborativeObjects, EntryContents, History, NewObjectSpec, ObjectId, TypeName,
    UpdateObjectSpec,
};
use librad::git::identities::local::LocalIdentity;
use librad::git::storage::ReadOnly;
use librad::git::Urn;

use crate::cobs::shared;
use crate::cobs::shared::*;

lazy_static! {
    pub static ref TYPENAME: TypeName = FromStr::from_str("xyz.radicle.issue").unwrap();
}

/// Identifier for an issue.
pub type IssueId = ObjectId;

/// Reason why an issue was closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CloseReason {
    Solved,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", tag = "status")]
pub enum State {
    Open,
    Closed { reason: CloseReason },
}

impl State {
    fn lifecycle_message(self) -> String {
        match self {
            State::Open => "Open issue".to_owned(),
            State::Closed { .. } => "Close issue".to_owned(),
        }
    }
}

impl From<State> for ScalarValue {
    fn from(state: State) -> Self {
        match state {
            State::Open => ScalarValue::from("open"),
            State::Closed {
                reason: CloseReason::Solved,
            } => ScalarValue::from("solved"),
            State::Closed {
                reason: CloseReason::Other,
            } => ScalarValue::from("closed"),
        }
    }
}

impl<'a> FromValue<'a> for State {
    fn from_value(value: Value) -> Result<Self, ValueError> {
        let state = value.to_str().ok_or(ValueError::InvalidType)?;

        match state {
            "open" => Ok(Self::Open),
            "closed" => Ok(Self::Closed {
                reason: CloseReason::Other,
            }),
            "solved" => Ok(Self::Closed {
                reason: CloseReason::Solved,
            }),
            _ => Err(ValueError::InvalidValue(value.to_string())),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub author: Author,
    pub title: String,
    pub state: State,
    pub comment: Comment,
    pub discussion: Discussion,
    pub labels: HashSet<Label>,
    pub timestamp: Timestamp,
}

impl Issue {
    pub fn author(&self) -> &Author {
        &self.author
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn state(&self) -> State {
        self.state
    }

    pub fn description(&self) -> &str {
        &self.comment.body
    }

    pub fn reactions(&self) -> &HashMap<Reaction, usize> {
        &self.comment.reactions
    }

    pub fn comments(&self) -> &[Comment<Replies>] {
        &self.discussion
    }

    pub fn labels(&self) -> &HashSet<Label> {
        &self.labels
    }

    pub fn timestamp(&self) -> Timestamp {
        self.timestamp
    }

    pub fn resolve<S: AsRef<ReadOnly>>(&mut self, storage: &S) -> Result<(), ResolveError> {
        self.author.resolve(storage)?;
        self.comment.resolve(storage)?;

        for comment in &mut self.discussion {
            comment.resolve(storage)?;
        }
        Ok(())
    }
}

impl Cob for Issue {
    fn type_name() -> &'static TypeName {
        &TYPENAME
    }

    fn from_history(history: &History) -> Result<Self, anyhow::Error> {
        let doc = history.traverse(Automerge::new(), |mut doc, entry| {
            match entry.contents() {
                EntryContents::Automerge(bytes) => {
                    match automerge::Change::from_bytes(bytes.clone()) {
                        Ok(change) => {
                            doc.apply_changes([change]).ok();
                        }
                        Err(_err) => {
                            // Ignore
                        }
                    }
                }
            }
            ControlFlow::Continue(doc)
        });
        let issue = Issue::try_from(doc)?;

        Ok(issue)
    }
}

impl TryFrom<&History> for Issue {
    type Error = anyhow::Error;

    fn try_from(history: &History) -> Result<Self, Self::Error> {
        Issue::from_history(history)
    }
}

impl TryFrom<Automerge> for Issue {
    type Error = DocumentError;

    fn try_from(doc: Automerge) -> Result<Self, Self::Error> {
        let doc = Document::new(&doc);
        let (_obj, obj_id) = doc.get(automerge::ObjId::Root, "issue")?;
        let title = doc.val(&obj_id, "title")?;
        let (_, comment_id) = doc.get(&obj_id, "comment")?;
        let peer = doc.val(&obj_id, "peer")?;
        let author = doc
            .val(&obj_id, "author")
            .map(|urn: Urn| Author::new(urn, peer))?;
        let state = doc.val(&obj_id, "state")?;
        let timestamp = doc.val(&obj_id, "timestamp")?;

        let comment = shared::lookup::comment(doc, &comment_id)?;
        let discussion: Discussion = doc.list(&obj_id, "discussion", shared::lookup::thread)?;
        let labels: HashSet<Label> = doc.keys(&obj_id, "labels")?;

        Ok(Self {
            title,
            state,
            author,
            comment,
            discussion,
            labels,
            timestamp,
        })
    }
}

pub struct IssueStore<'a> {
    store: &'a Store<'a>,
}

impl<'a> Deref for IssueStore<'a> {
    type Target = Store<'a>;

    fn deref(&self) -> &Self::Target {
        self.store
    }
}

impl<'a> IssueStore<'a> {
    pub fn new(store: &'a Store<'a>) -> Self {
        Self { store }
    }

    pub fn create(
        &self,
        project: &Urn,
        title: &str,
        description: &str,
        labels: &[Label],
    ) -> Result<IssueId, Error> {
        let author = self.author();
        let timestamp = Timestamp::now();
        let history = events::create(&author, title, description, timestamp, labels)?;

        cobs::create(history, project, &self.whoami, self.store)
    }

    pub fn remove(&self, _project: &Urn, _issue_id: &IssueId) -> Result<(), Error> {
        todo!()
    }

    pub fn comment(&self, project: &Urn, issue_id: &IssueId, body: &str) -> Result<IssueId, Error> {
        let author = self.author();
        let mut issue = self.get_raw(project, issue_id)?.unwrap();
        let timestamp = Timestamp::now();
        let changes = events::comment(&mut issue, &author, body, timestamp)?;
        let cob = self
            .store
            .update(
                &self.whoami,
                project,
                UpdateObjectSpec {
                    object_id: *issue_id,
                    typename: TYPENAME.clone(),
                    message: Some("Add comment".to_owned()),
                    changes,
                },
            )
            .unwrap();

        Ok(*cob.id()) // TODO: Return something other than doc id.
    }

    pub fn lifecycle(&self, project: &Urn, issue_id: &IssueId, state: State) -> Result<(), Error> {
        let author = self.whoami.urn();
        let mut issue = self.get_raw(project, issue_id)?.unwrap();
        let changes = events::lifecycle(&mut issue, &author, state)?;
        let _cob = self
            .store
            .update(
                &self.whoami,
                project,
                UpdateObjectSpec {
                    object_id: *issue_id,
                    typename: TYPENAME.clone(),
                    message: Some("Add comment".to_owned()),
                    changes,
                },
            )
            .unwrap();

        Ok(())
    }

    pub fn label(&self, project: &Urn, issue_id: &IssueId, labels: &[Label]) -> Result<(), Error> {
        let author = self.whoami.urn();
        let mut issue = self.get_raw(project, issue_id)?.unwrap();
        let changes = events::label(&mut issue, &author, labels)?;
        let _cob = self
            .store
            .update(
                &self.whoami,
                project,
                UpdateObjectSpec {
                    object_id: *issue_id,
                    typename: TYPENAME.clone(),
                    message: Some("Add label".to_owned()),
                    changes,
                },
            )
            .unwrap();

        Ok(())
    }

    pub fn react(
        &self,
        project: &Urn,
        issue_id: &IssueId,
        comment_id: CommentId,
        reaction: Reaction,
    ) -> Result<(), Error> {
        let author = self.whoami.urn();
        let mut issue = self.get_raw(project, issue_id)?.unwrap();
        let changes = events::react(&mut issue, comment_id, &author, &[reaction])?;
        let _cob = self
            .store
            .update(
                &self.whoami,
                project,
                UpdateObjectSpec {
                    object_id: *issue_id,
                    typename: TYPENAME.clone(),
                    message: Some("Add comment".to_owned()),
                    changes,
                },
            )
            .unwrap();

        Ok(())
    }

    pub fn reply(
        &self,
        project: &Urn,
        issue_id: &IssueId,
        comment_id: CommentId,
        reply: &str,
    ) -> Result<(), Error> {
        let author = self.author();
        let mut issue = self.get_raw(project, issue_id)?.unwrap();
        let changes = events::reply(&mut issue, comment_id, &author, reply, Timestamp::now())?;

        let _cob = self
            .store
            .update(
                &self.whoami,
                project,
                UpdateObjectSpec {
                    object_id: *issue_id,
                    typename: TYPENAME.clone(),
                    message: Some("Reply".to_owned()),
                    changes,
                },
            )
            .unwrap();

        Ok(())
    }

    pub fn all(&self, project: &Urn) -> Result<Vec<(IssueId, Issue)>, Error> {
        let cobs = self.store.list(project, &TYPENAME)?;

        let mut issues = Vec::new();
        for cob in cobs {
            let issue: Result<Issue, _> = cob.history().try_into();
            issues.push((*cob.id(), issue.unwrap()));
        }
        issues.sort_by_key(|(_, i)| i.timestamp);

        Ok(issues)
    }

    pub fn count(&self, project: &Urn) -> Result<usize, Error> {
        let cobs = self.store.list(project, &TYPENAME)?;

        Ok(cobs.len())
    }

    pub fn get(&self, namespace: &Urn, id: &ObjectId) -> anyhow::Result<Option<Issue>> {
        self.store.get::<Issue>(namespace, id)
    }

    pub fn get_raw(&self, project: &Urn, id: &IssueId) -> Result<Option<Automerge>, Error> {
        let cob = self.store.retrieve(project, &TYPENAME, id)?;
        let cob = if let Some(cob) = cob {
            cob
        } else {
            return Ok(None);
        };

        let doc = cob.history().traverse(Vec::new(), |mut doc, entry| {
            match entry.contents() {
                EntryContents::Automerge(bytes) => {
                    doc.extend(bytes);
                }
            }
            ControlFlow::Continue(doc)
        });

        let doc = Automerge::load(&doc)?;

        Ok(Some(doc))
    }
}

mod cobs {
    use super::*;

    pub(super) fn create(
        history: EntryContents,
        project: &Urn,
        whoami: &LocalIdentity,
        store: &CollaborativeObjects,
    ) -> Result<IssueId, Error> {
        let cob = store.create(
            whoami,
            project,
            NewObjectSpec {
                typename: TYPENAME.clone(),
                message: Some("Create issue".to_owned()),
                history,
            },
        )?;

        Ok(*cob.id())
    }
}

mod events {
    use super::*;
    use automerge::{
        transaction::{CommitOptions, Transactable},
        ObjId,
    };

    pub fn create(
        author: &Author,
        title: &str,
        description: &str,
        timestamp: Timestamp,
        labels: &[Label],
    ) -> Result<EntryContents, AutomergeError> {
        let title = title.trim();
        // TODO: Return error.
        if title.is_empty() {
            panic!("Empty issue title");
        }

        // TODO: Set actor id of document?
        let mut doc = Automerge::new();
        let _issue = doc
            .transact_with::<_, _, AutomergeError, _, ()>(
                |_| CommitOptions::default().with_message("Create issue".to_owned()),
                |tx| {
                    let issue = tx.put_object(ObjId::Root, "issue", ObjType::Map)?;

                    tx.put(&issue, "title", title)?;
                    tx.put(&issue, "author", author.urn().to_string())?;
                    tx.put(&issue, "peer", author.peer.default_encoding())?;
                    tx.put(&issue, "state", State::Open)?;
                    tx.put(&issue, "timestamp", timestamp)?;
                    tx.put_object(&issue, "discussion", ObjType::List)?;

                    let labels_id = tx.put_object(&issue, "labels", ObjType::Map)?;
                    for label in labels {
                        tx.put(&labels_id, label.name().trim(), true)?;
                    }

                    // Nb. The top-level comment doesn't have a `replies` field.
                    let comment_id = tx.put_object(&issue, "comment", ObjType::Map)?;

                    tx.put(&comment_id, "body", description.trim())?;
                    tx.put(&comment_id, "author", author.urn().to_string())?;
                    tx.put(&comment_id, "peer", author.peer.default_encoding())?;
                    tx.put(&comment_id, "timestamp", timestamp)?;
                    tx.put_object(&comment_id, "reactions", ObjType::Map)?;

                    Ok(issue)
                },
            )
            .map_err(|failure| failure.error)?
            .result;

        Ok(EntryContents::Automerge(doc.save_incremental()))
    }

    pub fn comment(
        issue: &mut Automerge,
        author: &Author,
        body: &str,
        timestamp: Timestamp,
    ) -> Result<EntryContents, AutomergeError> {
        let _comment = issue
            .transact_with::<_, _, AutomergeError, _, ()>(
                |_| CommitOptions::default().with_message("Add comment".to_owned()),
                |tx| {
                    let (_obj, obj_id) = tx.get(ObjId::Root, "issue")?.unwrap();
                    let (_, discussion_id) = tx.get(&obj_id, "discussion")?.unwrap();

                    let length = tx.length(&discussion_id);
                    let comment = tx.insert_object(&discussion_id, length, ObjType::Map)?;

                    tx.put(&comment, "author", author.urn().to_string())?;
                    tx.put(&comment, "peer", author.peer.default_encoding())?;
                    tx.put(&comment, "body", body.trim())?;
                    tx.put(&comment, "timestamp", timestamp)?;
                    tx.put_object(&comment, "replies", ObjType::List)?;
                    tx.put_object(&comment, "reactions", ObjType::Map)?;

                    Ok(comment)
                },
            )
            .map_err(|failure| failure.error)?
            .result;

        let change = issue.get_last_local_change().unwrap().raw_bytes().to_vec();

        Ok(EntryContents::Automerge(change))
    }

    pub fn lifecycle(
        issue: &mut Automerge,
        _author: &Urn,
        state: State,
    ) -> Result<EntryContents, AutomergeError> {
        issue
            .transact_with::<_, _, AutomergeError, _, ()>(
                |_| CommitOptions::default().with_message(state.lifecycle_message()),
                |tx| {
                    let (_, obj_id) = tx.get(ObjId::Root, "issue")?.unwrap();
                    tx.put(&obj_id, "state", state)?;

                    // TODO: Record who changed the state of the issue.

                    Ok(())
                },
            )
            .map_err(|failure| failure.error)?;

        let change = issue.get_last_local_change().unwrap().raw_bytes().to_vec();

        Ok(EntryContents::Automerge(change))
    }

    pub fn label(
        issue: &mut Automerge,
        _author: &Urn,
        labels: &[Label],
    ) -> Result<EntryContents, AutomergeError> {
        issue
            .transact_with::<_, _, AutomergeError, _, ()>(
                |_| CommitOptions::default().with_message("Label issue".to_owned()),
                |tx| {
                    let (_, obj_id) = tx.get(ObjId::Root, "issue")?.unwrap();
                    let (_, labels_id) = tx.get(&obj_id, "labels")?.unwrap();

                    for label in labels {
                        tx.put(&labels_id, label.name().trim(), true)?;
                    }
                    Ok(())
                },
            )
            .map_err(|failure| failure.error)?;

        let change = issue.get_last_local_change().unwrap().raw_bytes().to_vec();

        Ok(EntryContents::Automerge(change))
    }

    pub fn reply(
        issue: &mut Automerge,
        comment_id: CommentId,
        author: &Author,
        body: &str,
        timestamp: Timestamp,
    ) -> Result<EntryContents, AutomergeError> {
        issue
            .transact_with::<_, _, AutomergeError, _, ()>(
                |_| CommitOptions::default().with_message("Reply".to_owned()),
                |tx| {
                    let (_, obj_id) = tx.get(ObjId::Root, "issue")?.unwrap();
                    let (_, discussion_id) = tx.get(&obj_id, "discussion")?.unwrap();
                    let (_, comment_id) = tx.get(&discussion_id, usize::from(comment_id))?.unwrap();
                    let (_, replies_id) = tx.get(&comment_id, "replies")?.unwrap();

                    let length = tx.length(&replies_id);
                    let reply = tx.insert_object(&replies_id, length, ObjType::Map)?;

                    // Nb. Replies don't themselves have replies.
                    tx.put(&reply, "author", author.urn().to_string())?;
                    tx.put(&reply, "peer", author.peer.default_encoding())?;
                    tx.put(&reply, "body", body.trim())?;
                    tx.put(&reply, "timestamp", timestamp)?;
                    tx.put_object(&reply, "reactions", ObjType::Map)?;

                    Ok(())
                },
            )
            .map_err(|failure| failure.error)?;

        let change = issue.get_last_local_change().unwrap().raw_bytes().to_vec();

        Ok(EntryContents::Automerge(change))
    }

    pub fn react(
        issue: &mut Automerge,
        comment_id: CommentId,
        author: &Urn,
        reactions: &[Reaction],
    ) -> Result<EntryContents, AutomergeError> {
        issue
            .transact_with::<_, _, AutomergeError, _, ()>(
                |_| CommitOptions::default().with_message("React".to_owned()),
                |tx| {
                    let (_, obj_id) = tx.get(ObjId::Root, "issue")?.unwrap();
                    let (_, discussion_id) = tx.get(&obj_id, "discussion")?.unwrap();
                    let (_, comment_id) = if comment_id == CommentId::root() {
                        tx.get(&obj_id, "comment")?.unwrap()
                    } else {
                        tx.get(&discussion_id, usize::from(comment_id) - 1)?
                            .unwrap()
                    };
                    let (_, reactions_id) = tx.get(&comment_id, "reactions")?.unwrap();

                    for reaction in reactions {
                        let key = reaction.emoji.to_string();
                        let reaction_id = if let Some((_, reaction_id)) =
                            tx.get(&reactions_id, key)?
                        {
                            reaction_id
                        } else {
                            tx.put_object(&reactions_id, reaction.emoji.to_string(), ObjType::Map)?
                        };
                        tx.put(&reaction_id, author.to_string(), true)?;
                    }

                    Ok(())
                },
            )
            .map_err(|failure| failure.error)?;

        let change = issue.get_last_local_change().unwrap().raw_bytes().to_vec();

        Ok(EntryContents::Automerge(change))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test;

    #[test]
    fn test_issue_create_and_get() {
        let (storage, profile, whoami, project) = test::setup::profile();
        let author = whoami.urn();
        let timestamp = Timestamp::now();
        let cobs = Store::new(whoami, profile.paths(), &storage);
        let issues = cobs.issues();
        let issue_id = issues
            .create(&project.urn(), "My first issue", "Blah blah blah.", &[])
            .unwrap();
        let issue = issues.get(&project.urn(), &issue_id).unwrap().unwrap();

        assert_eq!(issue.title(), "My first issue");
        assert_eq!(issue.author().urn(), &author);
        assert_eq!(issue.description(), "Blah blah blah.");
        assert_eq!(issue.comments().len(), 0);
        assert_eq!(issue.state(), State::Open);
        assert!(issue.timestamp() >= timestamp);
    }

    #[test]
    fn test_issue_create_and_change_state() {
        let (storage, profile, whoami, project) = test::setup::profile();
        let cobs = Store::new(whoami, profile.paths(), &storage);
        let issues = cobs.issues();
        let issue_id = issues
            .create(&project.urn(), "My first issue", "Blah blah blah.", &[])
            .unwrap();

        issues
            .lifecycle(
                &project.urn(),
                &issue_id,
                State::Closed {
                    reason: CloseReason::Other,
                },
            )
            .unwrap();

        let issue = issues.get(&project.urn(), &issue_id).unwrap().unwrap();
        assert_eq!(
            issue.state(),
            State::Closed {
                reason: CloseReason::Other
            }
        );

        issues
            .lifecycle(&project.urn(), &issue_id, State::Open)
            .unwrap();
        let issue = issues.get(&project.urn(), &issue_id).unwrap().unwrap();
        assert_eq!(issue.state(), State::Open);
    }

    #[test]
    fn test_issue_react() {
        let (storage, profile, whoami, project) = test::setup::profile();
        let cobs = Store::new(whoami, profile.paths(), &storage);
        let issues = cobs.issues();
        let project = project.urn();
        let issue_id = issues
            .create(&project, "My first issue", "Blah blah blah.", &[])
            .unwrap();

        let reaction = Reaction::new('🥳').unwrap();
        issues
            .react(&project, &issue_id, CommentId::root(), reaction)
            .unwrap();

        let issue = issues.get(&project, &issue_id).unwrap().unwrap();
        let count = issue.reactions()[&reaction];

        // TODO: Test multiple reactions from same author and different authors

        assert_eq!(count, 1);
    }

    #[test]
    fn test_issue_reply() {
        let (storage, profile, whoami, project) = test::setup::profile();
        let cobs = Store::new(whoami, profile.paths(), &storage);
        let issues = cobs.issues();
        let project = project.urn();
        let issue_id = issues
            .create(&project, "My first issue", "Blah blah blah.", &[])
            .unwrap();

        issues.comment(&project, &issue_id, "Ho ho ho.").unwrap();
        issues
            .reply(&project, &issue_id, CommentId::root(), "Hi hi hi.")
            .unwrap();
        issues
            .reply(&project, &issue_id, CommentId::root(), "Ha ha ha.")
            .unwrap();

        let issue = issues.get(&project, &issue_id).unwrap().unwrap();
        let reply1 = &issue.comments()[0].replies[0];
        let reply2 = &issue.comments()[0].replies[1];

        assert_eq!(reply1.body, "Hi hi hi.");
        assert_eq!(reply2.body, "Ha ha ha.");
    }

    #[test]
    fn test_issue_label() {
        let (storage, profile, whoami, project) = test::setup::profile();
        let cobs = Store::new(whoami, profile.paths(), &storage);
        let issues = cobs.issues();
        let project = project.urn();
        let issue_id = issues
            .create(&project, "My first issue", "Blah blah blah.", &[])
            .unwrap();

        let bug_label = Label::new("bug").unwrap();
        let wontfix_label = Label::new("wontfix").unwrap();

        issues
            .label(&project, &issue_id, &[bug_label.clone()])
            .unwrap();
        issues
            .label(&project, &issue_id, &[wontfix_label.clone()])
            .unwrap();

        let issue = issues.get(&project, &issue_id).unwrap().unwrap();
        let labels = issue.labels();

        assert!(labels.contains(&bug_label));
        assert!(labels.contains(&wontfix_label));
    }

    #[test]
    fn test_issue_comment() {
        let (storage, profile, whoami, project) = test::setup::profile();
        let now = Timestamp::now();
        let author = whoami.urn();
        let cobs = Store::new(whoami, profile.paths(), &storage);
        let issues = cobs.issues();
        let issue_id = issues
            .create(&project.urn(), "My first issue", "Blah blah blah.", &[])
            .unwrap();

        issues
            .comment(&project.urn(), &issue_id, "Ho ho ho.")
            .unwrap();

        issues
            .comment(&project.urn(), &issue_id, "Ha ha ha.")
            .unwrap();

        let issue = issues.get(&project.urn(), &issue_id).unwrap().unwrap();
        let c1 = &issue.comments()[0];
        let c2 = &issue.comments()[1];

        assert_eq!(&c1.body, "Ho ho ho.");
        assert_eq!(c1.author.urn(), &author);
        assert_eq!(&c2.body, "Ha ha ha.");
        assert_eq!(c2.author.urn(), &author);
        assert!(c1.timestamp >= now);
    }

    #[test]
    fn test_issue_resolve() {
        let (storage, profile, whoami, project) = test::setup::profile();
        let cobs = Store::new(whoami, profile.paths(), &storage);
        let issues = cobs.issues();
        let issue_id = issues
            .create(&project.urn(), "My first issue", "Blah blah blah.", &[])
            .unwrap();

        issues
            .comment(&project.urn(), &issue_id, "Ho ho ho.")
            .unwrap();

        let mut issue = issues.get(&project.urn(), &issue_id).unwrap().unwrap();
        issue.resolve(&storage).unwrap();

        let c1 = &issue.comments()[0];

        assert!(
            matches!(&issue.author().profile, Some(AuthorProfile { name, .. }) if name == "cloudhead")
        );
        assert!(
            matches!(&issue.comment.author.profile, Some(AuthorProfile { name, .. }) if name == "cloudhead")
        );
        assert!(
            matches!(&c1.author.profile, Some(AuthorProfile { name, .. }) if name == "cloudhead")
        );
    }

    #[test]
    fn test_issue_state_serde() {
        assert_eq!(
            serde_json::to_value(State::Open).unwrap(),
            serde_json::json!({ "status": "open" })
        );

        assert_eq!(
            serde_json::to_value(State::Closed {
                reason: CloseReason::Solved
            })
            .unwrap(),
            serde_json::json!({ "status": "closed", "reason": "solved" })
        );
    }

    #[test]
    fn test_issue_all() {
        let (storage, profile, whoami, project) = test::setup::profile();
        let author = Author::new(whoami.urn(), *storage.peer_id());
        let cobs = Store::new(whoami, profile.paths(), &storage);
        let issues = cobs.issues();

        cobs::create(
            events::create(
                &author,
                "My first issue",
                "Blah blah blah.",
                Timestamp::new(1),
                &[],
            )
            .unwrap(),
            &project.urn(),
            &cobs.whoami,
            issues.store,
        )
        .unwrap();

        cobs::create(
            events::create(
                &author,
                "My second issue",
                "Blah blah blah.",
                Timestamp::new(2),
                &[],
            )
            .unwrap(),
            &project.urn(),
            &cobs.whoami,
            issues.store,
        )
        .unwrap();

        cobs::create(
            events::create(
                &author,
                "My third issue",
                "Blah blah blah.",
                Timestamp::new(3),
                &[],
            )
            .unwrap(),
            &project.urn(),
            &cobs.whoami,
            issues.store,
        )
        .unwrap();

        let issues = issues.all(&project.urn()).unwrap();

        // Issues are sorted by timestamp.
        assert_eq!(issues[0].1.title(), "My first issue");
        assert_eq!(issues[1].1.title(), "My second issue");
        assert_eq!(issues[2].1.title(), "My third issue");
    }
}
