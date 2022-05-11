use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::convert::{Infallible, TryFrom, TryInto};
use std::ops::ControlFlow;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use automerge::{Automerge, AutomergeError, ObjType, ScalarValue, Value};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

use librad::collaborative_objects::{
    CollaborativeObjects, EntryContents, History, NewObjectSpec, ObjectId, TypeName,
    UpdateObjectSpec,
};
use librad::git::identities::local::LocalIdentity;
use librad::git::Storage;
use librad::git::Urn;
use librad::paths::Paths;

lazy_static! {
    pub static ref TYPENAME: TypeName = FromStr::from_str("xyz.radicle.issue").unwrap();
    pub static ref SCHEMA: serde_json::Value =
        serde_json::from_slice(include_bytes!("issue.json")).unwrap();
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Create error: {0}")]
    Create(String),

    #[error("List error: {0}")]
    List(String),

    #[error("Retrieve error: {0}")]
    Retrieve(String),

    #[error(transparent)]
    Automerge(#[from] AutomergeError),
}

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone, Serialize, Deserialize)]
pub struct Reaction {
    pub emoji: char,
}

impl Reaction {
    pub fn new(emoji: char) -> Result<Self, Infallible> {
        Ok(Self { emoji })
    }
}

impl FromStr for Reaction {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut chars = s.chars();
        let first = chars.next().ok_or(String::new())?;

        // Reactions should not consist of more than a single emoji.
        if chars.next().is_some() {
            return Err(String::new());
        }
        Ok(Reaction::new(first).unwrap())
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Label(String);

impl Label {
    pub fn new(name: impl Into<String>) -> Result<Self, Infallible> {
        Ok(Self(name.into()))
    }

    pub fn name(&self) -> &str {
        self.0.as_str()
    }
}

impl From<Label> for String {
    fn from(Label(name): Label) -> Self {
        name
    }
}

/// Local id of a comment in an issue.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Copy, Clone)]
pub struct CommentId {
    /// Represents the index of the comment in the thread,
    /// with `0` being the top-level comment.
    ix: usize,
}

impl CommentId {
    /// Root comment.
    pub const fn root() -> Self {
        Self { ix: 0 }
    }
}

impl From<usize> for CommentId {
    fn from(ix: usize) -> Self {
        Self { ix }
    }
}

impl From<CommentId> for usize {
    fn from(id: CommentId) -> Self {
        id.ix
    }
}

/// Comment replies.
pub type Replies = Vec<Comment>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment<R = ()> {
    pub author: Urn,
    pub body: String,
    pub reactions: HashMap<Reaction, usize>,
    pub replies: R,
    pub timestamp: Timestamp,
}

pub fn author(val: Value) -> Result<Urn, AutomergeError> {
    let author = val.into_string().unwrap();
    let author = Urn::from_str(&author).unwrap();

    Ok(author)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Open,
    Closed,
}

impl From<State> for ScalarValue {
    fn from(state: State) -> Self {
        match state {
            State::Open => ScalarValue::from("open"),
            State::Closed => ScalarValue::from("closed"),
        }
    }
}

impl<'a> TryFrom<Value<'a>> for State {
    type Error = &'static str;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        let state = value.to_str().ok_or("value isn't a string")?;

        match state {
            "open" => Ok(Self::Open),
            "closed" => Ok(Self::Closed),
            _ => Err("invalid state name"),
        }
    }
}

/// A discussion thread.
pub type Discussion = Vec<Comment<Replies>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub author: Urn,
    pub title: String,
    pub state: State,
    pub comment: Comment,
    pub discussion: Discussion,
    pub labels: HashSet<Label>,
    pub timestamp: Timestamp,
}

impl Issue {
    pub fn author(&self) -> &Urn {
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
}

impl TryFrom<&History> for Issue {
    type Error = anyhow::Error;

    fn try_from(history: &History) -> Result<Self, Self::Error> {
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

impl TryFrom<Automerge> for Issue {
    type Error = AutomergeError;

    fn try_from(doc: Automerge) -> Result<Self, Self::Error> {
        let (_obj, obj_id) = doc.get(automerge::ObjId::Root, "issue")?.unwrap();
        let (title, _) = doc.get(&obj_id, "title")?.unwrap();
        let (_, comment_id) = doc.get(&obj_id, "comment")?.unwrap();
        let (discussion, discussion_id) = doc.get(&obj_id, "discussion")?.unwrap();
        let (author, _) = doc.get(&obj_id, "author")?.unwrap();
        let (state, _) = doc.get(&obj_id, "state")?.unwrap();
        let (timestamp, _) = doc.get(&obj_id, "timestamp")?.unwrap();
        let (labels, labels_id) = doc.get(&obj_id, "labels")?.unwrap();

        assert_eq!(discussion.to_objtype(), Some(ObjType::List));
        assert_eq!(labels.to_objtype(), Some(ObjType::Map));

        // Top-level comment.
        let comment = lookup::comment(&doc, &comment_id)?;

        // Discussion thread.
        let mut discussion: Discussion = Vec::new();
        for i in 0..doc.length(&discussion_id) {
            let (_val, comment_id) = doc.get(&discussion_id, i as usize)?.unwrap();
            let comment = lookup::thread(&doc, &comment_id)?;

            discussion.push(comment);
        }

        // Labels.
        let mut labels = HashSet::new();
        for key in doc.keys(&labels_id) {
            let label = Label::new(key).unwrap();

            labels.insert(label);
        }

        let author = self::author(author)?;
        let state = State::try_from(state).unwrap();
        let timestamp = Timestamp::try_from(timestamp).unwrap();

        Ok(Self {
            title: title.into_string().unwrap(),
            state,
            author,
            comment,
            discussion,
            labels,
            timestamp,
        })
    }
}

#[derive(Debug, Copy, Clone, PartialOrd, PartialEq, Ord, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Timestamp {
    seconds: u64,
}

impl Timestamp {
    pub fn new(seconds: u64) -> Self {
        Self { seconds }
    }

    pub fn now() -> Self {
        let duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

        Self {
            seconds: duration.as_secs(),
        }
    }

    pub fn as_secs(&self) -> u64 {
        self.seconds
    }
}

impl From<Timestamp> for ScalarValue {
    fn from(ts: Timestamp) -> Self {
        ScalarValue::Timestamp(ts.seconds as i64)
    }
}

impl<'a> TryFrom<Value<'a>> for Timestamp {
    type Error = String;

    fn try_from(val: Value) -> Result<Self, Self::Error> {
        if let Value::Scalar(scalar) = val {
            if let ScalarValue::Timestamp(ts) = scalar.borrow() {
                return Ok(Self {
                    seconds: *ts as u64,
                });
            }
        }
        Err(String::from("value is not a timestamp"))
    }
}

pub struct Issues<'a> {
    store: CollaborativeObjects<'a>,
    whoami: LocalIdentity,
}

impl<'a> Issues<'a> {
    pub fn new(whoami: LocalIdentity, paths: &Paths, storage: &'a Storage) -> Result<Self, Error> {
        let store = storage.collaborative_objects(Some(paths.cob_cache_dir().to_path_buf()));

        Ok(Self { store, whoami })
    }

    pub fn create(&self, project: &Urn, title: &str, description: &str) -> Result<ObjectId, Error> {
        let author = self.whoami.urn();
        let timestamp = Timestamp::now();
        let history = events::create(&author, title, description, timestamp)?;

        cobs::create(history, project, &self.whoami, &self.store)
    }

    pub fn comment(
        &self,
        project: &Urn,
        issue_id: &ObjectId,
        body: &str,
    ) -> Result<ObjectId, Error> {
        let author = self.whoami.urn();
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

    pub fn close(&self, project: &Urn, issue_id: &ObjectId) -> Result<(), Error> {
        let author = self.whoami.urn();
        let mut issue = self.get_raw(project, issue_id)?.unwrap();
        let changes = events::lifecycle(&mut issue, &author, State::Closed)?;
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

    pub fn label(&self, project: &Urn, issue_id: &ObjectId, labels: &[Label]) -> Result<(), Error> {
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
        issue_id: &ObjectId,
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
        issue_id: &ObjectId,
        comment_id: CommentId,
        reply: &str,
    ) -> Result<(), Error> {
        let author = self.whoami.urn();
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

    pub fn all(&self, project: &Urn) -> Result<Vec<(ObjectId, Issue)>, Error> {
        let cobs = self
            .store
            .list(project, &TYPENAME)
            .map_err(|e| Error::List(e.to_string()))?;

        let mut issues = Vec::new();
        for cob in cobs {
            let issue: Result<Issue, _> = cob.history().try_into();
            issues.push((*cob.id(), issue.unwrap()));
        }
        issues.sort_by_key(|(_, i)| i.timestamp);

        Ok(issues)
    }

    pub fn get(&self, project: &Urn, id: &ObjectId) -> Result<Option<Issue>, Error> {
        let cob = self
            .store
            .retrieve(project, &TYPENAME, id)
            .map_err(|e| Error::Retrieve(e.to_string()))?;

        if let Some(cob) = cob {
            let issue = Issue::try_from(cob.history()).unwrap();
            Ok(Some(issue))
        } else {
            Ok(None)
        }
    }

    pub fn get_raw(&self, project: &Urn, id: &ObjectId) -> Result<Option<Automerge>, Error> {
        let cob = self
            .store
            .retrieve(project, &TYPENAME, id)
            .map_err(|e| Error::Retrieve(e.to_string()))?;

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
    ) -> Result<ObjectId, Error> {
        let cob = store
            .create(
                whoami,
                project,
                NewObjectSpec {
                    schema_json: SCHEMA.clone(),
                    typename: TYPENAME.clone(),
                    message: Some("Create issue".to_owned()),
                    history,
                },
            )
            .map_err(|e| Error::Create(e.to_string()))?;

        Ok(*cob.id())
    }
}

mod lookup {
    use std::convert::TryFrom;
    use std::str::FromStr;

    use super::{Automerge, AutomergeError, Comment, HashMap, Reaction, Replies, Timestamp};

    pub fn comment(
        doc: &Automerge,
        obj_id: &automerge::ObjId,
    ) -> Result<Comment<()>, AutomergeError> {
        let (author, _) = doc.get(&obj_id, "author")?.unwrap();
        let (body, _) = doc.get(&obj_id, "body")?.unwrap();
        let (timestamp, _) = doc.get(&obj_id, "timestamp")?.unwrap();
        let (_, reactions_id) = doc.get(&obj_id, "reactions")?.unwrap();

        let author = super::author(author)?;
        let body = body.into_string().unwrap();
        let timestamp = Timestamp::try_from(timestamp).unwrap();

        let mut reactions: HashMap<_, usize> = HashMap::new();
        for reaction in doc.keys(&reactions_id) {
            let key = Reaction::from_str(&reaction).unwrap();
            let count = reactions.entry(key).or_default();

            *count += 1;
        }

        Ok(Comment {
            author,
            body,
            reactions,
            replies: (),
            timestamp,
        })
    }

    pub fn thread(
        doc: &Automerge,
        obj_id: &automerge::ObjId,
    ) -> Result<Comment<Replies>, AutomergeError> {
        let comment = self::comment(doc, obj_id)?;

        let mut replies = Vec::new();
        if let Some((_, replies_id)) = doc.get(&obj_id, "replies")? {
            for i in 0..doc.length(&replies_id) {
                let (_, reply_id) = doc.get(&replies_id, i as usize)?.unwrap();
                let reply = self::comment(doc, &reply_id)?;

                replies.push(reply);
            }
        }

        Ok(Comment {
            author: comment.author,
            body: comment.body,
            reactions: comment.reactions,
            replies,
            timestamp: comment.timestamp,
        })
    }
}

mod events {
    use super::*;
    use automerge::{
        transaction::{CommitOptions, Transactable},
        ObjId,
    };

    pub fn create(
        author: &Urn,
        title: &str,
        description: &str,
        timestamp: Timestamp,
    ) -> Result<EntryContents, AutomergeError> {
        // TODO: Set actor id of document?
        let mut doc = Automerge::new();
        let _issue = doc
            .transact_with::<_, _, AutomergeError, _, ()>(
                |_| CommitOptions::default().with_message("Create issue".to_owned()),
                |tx| {
                    let issue = tx.put_object(ObjId::Root, "issue", ObjType::Map)?;

                    tx.put(&issue, "title", title)?;
                    tx.put(&issue, "author", author.to_string())?;
                    tx.put(&issue, "state", State::Open)?;
                    tx.put(&issue, "timestamp", timestamp)?;
                    tx.put_object(&issue, "labels", ObjType::Map)?;
                    tx.put_object(&issue, "discussion", ObjType::List)?;

                    let comment = tx.put_object(&issue, "comment", ObjType::Map)?;

                    // Nb. The top-level comment doesn't have a `replies` field.
                    tx.put(&comment, "body", description)?;
                    tx.put(&comment, "author", author.to_string())?;
                    tx.put(&comment, "timestamp", timestamp)?;
                    tx.put_object(&comment, "reactions", ObjType::Map)?;

                    Ok(issue)
                },
            )
            .map_err(|failure| failure.error)?
            .result;

        Ok(EntryContents::Automerge(doc.save_incremental()))
    }

    pub fn comment(
        issue: &mut Automerge,
        author: &Urn,
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

                    tx.put(&comment, "author", author.to_string())?;
                    tx.put(&comment, "body", body)?;
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
                |_| CommitOptions::default().with_message("Close issue".to_owned()),
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
                        tx.put(&labels_id, label.name(), true)?;
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
        author: &Urn,
        body: &str,
        timestamp: Timestamp,
    ) -> Result<EntryContents, AutomergeError> {
        issue
            .transact_with::<_, _, AutomergeError, _, ()>(
                |_| CommitOptions::default().with_message("Reply".to_owned()),
                |tx| {
                    let CommentId { ix } = comment_id;
                    let (_, obj_id) = tx.get(ObjId::Root, "issue")?.unwrap();
                    let (_, discussion_id) = tx.get(&obj_id, "discussion")?.unwrap();
                    let (_, comment_id) = tx.get(&discussion_id, ix)?.unwrap();
                    let (_, replies_id) = tx.get(&comment_id, "replies")?.unwrap();

                    let length = tx.length(&replies_id);
                    let reply = tx.insert_object(&replies_id, length, ObjType::Map)?;

                    // Nb. Replies don't themselves have replies.
                    tx.put(&reply, "author", author.to_string())?;
                    tx.put(&reply, "body", body)?;
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
                        tx.get(&discussion_id, comment_id.ix - 1)?.unwrap()
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
                        tx.put(&reaction_id, author.encode_id(), true)?;
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
    use std::env;
    use std::path::Path;

    use librad::crypto::keystore::crypto::{Pwhash, KDF_PARAMS_TEST};
    use librad::crypto::keystore::pinentry::SecUtf8;
    use librad::git::identities::Project;

    use librad::profile::{Profile, LNK_HOME};

    use super::*;
    use crate::{keys, person, project, test};

    fn setup() -> (Storage, Profile, LocalIdentity, Project) {
        let tempdir = env::temp_dir().join("rad").join("home");
        let home = env::var(LNK_HOME)
            .map(|s| Path::new(&s).to_path_buf())
            .unwrap_or_else(|_| tempdir.to_path_buf());

        env::set_var(LNK_HOME, home);

        let name = "cloudhead";
        let pass = Pwhash::new(SecUtf8::from(test::USER_PASS), *KDF_PARAMS_TEST);
        let (profile, _peer_id) = lnk_profile::create(None, pass.clone()).unwrap();
        let signer = test::signer(&profile, pass).unwrap();
        let storage = keys::storage(&profile, signer.clone()).unwrap();
        let person = person::create(&profile, name, signer, &storage).unwrap();

        person::set_local(&storage, &person).unwrap();

        let whoami = person::local(&storage).unwrap();
        let payload = project::payload(
            "nakamoto".to_owned(),
            "Bitcoin light-client".to_owned(),
            "master".to_owned(),
        );
        let project = project::create(payload, &storage).unwrap();

        (storage, profile, whoami, project)
    }

    #[test]
    fn test_issue_create_and_get() {
        let (storage, profile, whoami, project) = setup();
        let author = whoami.urn();
        let issues = Issues::new(whoami, profile.paths(), &storage).unwrap();
        let issue_id = issues
            .create(&project.urn(), "My first issue", "Blah blah blah.")
            .unwrap();
        let issue = issues.get(&project.urn(), &issue_id).unwrap().unwrap();
        let timestamp = Timestamp::now();

        assert_eq!(issue.title(), "My first issue");
        assert_eq!(issue.author(), &author);
        assert_eq!(issue.description(), "Blah blah blah.");
        assert_eq!(issue.comments().len(), 0);
        assert_eq!(issue.state(), State::Open);
        assert!(issue.timestamp() >= timestamp);
    }

    #[test]
    fn test_issue_create_and_change_state() {
        let (storage, profile, whoami, project) = setup();
        let issues = Issues::new(whoami, profile.paths(), &storage).unwrap();
        let issue_id = issues
            .create(&project.urn(), "My first issue", "Blah blah blah.")
            .unwrap();

        issues.close(&project.urn(), &issue_id).unwrap();

        let issue = issues.get(&project.urn(), &issue_id).unwrap().unwrap();
        assert_eq!(issue.state(), State::Closed);
    }

    #[test]
    fn test_issue_react() {
        let (storage, profile, whoami, project) = setup();
        let issues = Issues::new(whoami, profile.paths(), &storage).unwrap();
        let project = project.urn();
        let issue_id = issues
            .create(&project, "My first issue", "Blah blah blah.")
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
        let (storage, profile, whoami, project) = setup();
        let issues = Issues::new(whoami, profile.paths(), &storage).unwrap();
        let project = project.urn();
        let issue_id = issues
            .create(&project, "My first issue", "Blah blah blah.")
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
        let (storage, profile, whoami, project) = setup();
        let issues = Issues::new(whoami, profile.paths(), &storage).unwrap();
        let project = project.urn();
        let issue_id = issues
            .create(&project, "My first issue", "Blah blah blah.")
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
        let (storage, profile, whoami, project) = setup();
        let now = Timestamp::now();
        let author = whoami.urn();
        let issues = Issues::new(whoami, profile.paths(), &storage).unwrap();
        let issue_id = issues
            .create(&project.urn(), "My first issue", "Blah blah blah.")
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
        assert_eq!(&c1.author, &author);
        assert_eq!(&c2.body, "Ha ha ha.");
        assert_eq!(&c2.author, &author);
        assert!(c1.timestamp >= now);
    }

    #[test]
    fn test_issue_all() {
        let (storage, profile, whoami, project) = setup();
        let author = whoami.urn();
        let issues = Issues::new(whoami.clone(), profile.paths(), &storage).unwrap();

        cobs::create(
            events::create(
                &author,
                "My first issue",
                "Blah blah blah.",
                Timestamp::new(1),
            )
            .unwrap(),
            &project.urn(),
            &whoami,
            &issues.store,
        )
        .unwrap();

        cobs::create(
            events::create(
                &author,
                "My second issue",
                "Blah blah blah.",
                Timestamp::new(2),
            )
            .unwrap(),
            &project.urn(),
            &whoami,
            &issues.store,
        )
        .unwrap();

        cobs::create(
            events::create(
                &author,
                "My third issue",
                "Blah blah blah.",
                Timestamp::new(3),
            )
            .unwrap(),
            &project.urn(),
            &whoami,
            &issues.store,
        )
        .unwrap();

        let issues = issues.all(&project.urn()).unwrap();

        // Issues are sorted by timestamp.
        assert_eq!(issues[0].1.title(), "My first issue");
        assert_eq!(issues[1].1.title(), "My second issue");
        assert_eq!(issues[2].1.title(), "My third issue");
    }
}
