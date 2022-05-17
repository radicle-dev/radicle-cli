#![allow(clippy::large_enum_variant)]
use std::borrow::Borrow;
use std::collections::HashMap;
use std::convert::{Infallible, TryFrom};

use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use automerge::{Automerge, AutomergeError, ScalarValue, Value};
use serde::{Deserialize, Serialize};

use librad::git::storage::ReadOnly;
use librad::git::Urn;

use crate::project;

#[derive(thiserror::Error, Debug)]
pub enum ResolveError {
    #[error("identity {urn} was not found")]
    NotFound { urn: Urn },
    #[error(transparent)]
    Identities(#[from] librad::git::identities::Error),
}

/// A discussion thread.
pub type Discussion = Vec<Comment<Replies>>;

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

/// Issue author.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum Author {
    Urn { urn: Urn },
    Resolved(project::PeerIdentity),
}

impl Author {
    pub fn urn(&self) -> &Urn {
        match self {
            Self::Urn { ref urn } => urn,
            Self::Resolved(project::PeerIdentity { urn, .. }) => urn,
        }
    }

    pub fn resolve<S: AsRef<ReadOnly>>(&mut self, storage: &S) -> Result<(), ResolveError> {
        match self {
            Self::Urn { urn } => {
                let id = project::PeerIdentity::get(urn, storage)?
                    .ok_or_else(|| ResolveError::NotFound { urn: urn.clone() })?;
                *self = Self::Resolved(id);
            }
            Self::Resolved(_) => {}
        }
        Ok(())
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
    pub author: Author,
    pub body: String,
    pub reactions: HashMap<Reaction, usize>,
    pub replies: R,
    pub timestamp: Timestamp,
}

impl Comment<()> {
    pub fn resolve<S: AsRef<ReadOnly>>(&mut self, storage: &S) -> Result<(), ResolveError> {
        self.author.resolve(storage)
    }
}

impl Comment<Replies> {
    pub fn resolve<S: AsRef<ReadOnly>>(&mut self, storage: &S) -> Result<(), ResolveError> {
        self.author.resolve(storage)?;
        for reply in &mut self.replies {
            reply.resolve(storage)?;
        }
        Ok(())
    }
}

pub fn author(val: Value) -> Result<Author, AutomergeError> {
    let urn = val.into_string().unwrap();
    let urn = Urn::from_str(&urn).unwrap();

    Ok(Author::Urn { urn })
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

pub mod lookup {
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
