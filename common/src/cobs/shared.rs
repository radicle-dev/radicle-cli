#![allow(clippy::large_enum_variant)]
use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::convert::{Infallible, TryFrom};
use std::fmt;
use std::hash::Hash;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::Arc;
use std::time;
use std::time::{SystemTime, UNIX_EPOCH};

use automerge::transaction::Transactable;
use automerge::{Automerge, AutomergeError, ObjType, ScalarValue, Value};
use chrono::TimeZone;
use serde::{Deserialize, Serialize};

use librad::collaborative_objects::{CollaborativeObjects, ObjectId, TypeName};
use librad::git::storage::ReadOnly;
use librad::git::Urn;
use librad::PeerId;
use radicle_git_ext as git;

use crate::person;
use crate::project;

#[derive(thiserror::Error, Debug)]
pub enum ValueError {
    #[error("invalid type")]
    InvalidType,
    #[error("invalid value: `{0}`")]
    InvalidValue(String),
    #[error("value error: {0}")]
    Other(Arc<dyn std::error::Error + Send + Sync>),
}

#[derive(thiserror::Error, Debug)]
pub enum ResolveError {
    #[error("identity {urn} was not found")]
    NotFound { urn: Urn },
    #[error(transparent)]
    Identities(#[from] librad::git::identities::Error),
}

/// A generic COB identifier.
#[derive(Debug, Clone)]
pub enum CobIdentifier {
    /// Regular, full object id.
    Full(ObjectId),
    /// A prefix of a full id.
    Prefix(String),
}

impl FromStr for CobIdentifier {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(id) = ObjectId::from_str(s) {
            Ok(CobIdentifier::Full(id))
        } else {
            // TODO: Do some validation here.
            Ok(CobIdentifier::Prefix(s.to_owned()))
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("Create error: {0}")]
    Create(String),

    #[error("Update error: {0}")]
    Update(String),

    #[error("List error: {0}")]
    List(String),

    #[error("Retrieve error: {0}")]
    Retrieve(String),

    #[error(transparent)]
    Automerge(#[from] AutomergeError),
}

pub trait Store<'a> {
    fn type_name() -> TypeName;

    fn store(&self) -> &CollaborativeObjects<'a>;

    fn find(
        &self,
        project: &Urn,
        predicate: impl Fn(&ObjectId) -> bool,
    ) -> Result<Vec<ObjectId>, StoreError> {
        let cobs = self
            .store()
            .list(project, &Self::type_name())
            .map_err(|e| StoreError::List(e.to_string()))?;

        Ok(cobs
            .into_iter()
            .map(|c| *c.id())
            .filter(|id| predicate(id))
            .collect())
    }

    fn resolve_id(&self, project: &Urn, identifier: CobIdentifier) -> anyhow::Result<ObjectId> {
        match identifier {
            CobIdentifier::Full(id) => Ok(id),
            CobIdentifier::Prefix(prefix) => {
                let matches = self.find(project, |p| p.to_string().starts_with(&prefix))?;

                match matches.as_slice() {
                    [id] => Ok(*id),
                    [_id, ..] => {
                        anyhow::bail!(
                            "object id `{}` is ambiguous; please use the fully qualified id",
                            prefix
                        );
                    }
                    [] => {
                        anyhow::bail!("object `{}` not found in local storage", prefix);
                    }
                }
            }
        }
    }
}

/// A discussion thread.
pub type Discussion = Vec<Comment<Replies>>;

#[derive(thiserror::Error, Debug)]
pub enum ReactionError {
    #[error("invalid reaction")]
    InvalidReaction,
}

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Reaction {
    pub emoji: char,
}

impl Reaction {
    pub fn new(emoji: char) -> Result<Self, ReactionError> {
        if emoji.is_whitespace() || emoji.is_ascii() || emoji.is_alphanumeric() {
            return Err(ReactionError::InvalidReaction);
        }
        Ok(Self { emoji })
    }
}

impl FromStr for Reaction {
    type Err = ReactionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut chars = s.chars();
        let first = chars.next().ok_or(ReactionError::InvalidReaction)?;

        // Reactions should not consist of more than a single emoji.
        if chars.next().is_some() {
            return Err(ReactionError::InvalidReaction);
        }
        Reaction::new(first)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum LabelError {
    #[error("invalid label name: `{0}`")]
    InvalidName(String),
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Label(String);

impl Label {
    pub fn new(name: impl Into<String>) -> Result<Self, LabelError> {
        let name = name.into();

        if name.chars().any(|c| c.is_whitespace()) || name.is_empty() {
            return Err(LabelError::InvalidName(name));
        }
        Ok(Self(name))
    }

    pub fn name(&self) -> &str {
        self.0.as_str()
    }
}

impl FromStr for Label {
    type Err = LabelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl From<Label> for String {
    fn from(Label(name): Label) -> Self {
        name
    }
}

/// RGB color.
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Color(u32);

#[derive(thiserror::Error, Debug)]
pub enum ColorConversionError {
    #[error("invalid format: expect '#rrggbb'")]
    InvalidFormat,
    #[error(transparent)]
    ParseInt(#[from] std::num::ParseIntError),
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:06x}", self.0)
    }
}

impl FromStr for Color {
    type Err = ColorConversionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let hex = s.replace('#', "").to_lowercase();

        if hex.chars().count() != 6 {
            return Err(ColorConversionError::InvalidFormat);
        }

        match u32::from_str_radix(&hex, 16) {
            Ok(n) => Ok(Color(n)),
            Err(e) => Err(e.into()),
        }
    }
}

impl Serialize for Color {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        let s = self.to_string();
        serializer.serialize_str(&s)
    }
}

impl<'a> Deserialize<'a> for Color {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'a>,
    {
        let color = String::deserialize(deserializer)?;
        Self::from_str(&color).map_err(serde::de::Error::custom)
    }
}

/// A user identity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum Identity {
    /// Only the URN is known.
    Unresolved { urn: Urn },
    /// The full user identity is resolved.
    Resolved {
        urn: Urn,
        name: String,
        ens: Option<person::Ens>,
    },
}

impl Identity {
    pub fn urn(&self) -> &Urn {
        match self {
            Self::Unresolved { ref urn } => urn,
            Self::Resolved { urn, .. } => urn,
        }
    }

    pub fn name(&self) -> String {
        match self {
            Self::Unresolved { urn } => urn.encode_id(),
            Self::Resolved { name, .. } => name.clone(),
        }
    }

    pub fn resolve<S: AsRef<ReadOnly>>(&mut self, storage: &S) -> Result<&Identity, ResolveError> {
        match self {
            Self::Unresolved { urn } => {
                let identity = project::PeerIdentity::get(urn, storage)?
                    .ok_or_else(|| ResolveError::NotFound { urn: urn.clone() })?;
                *self = Self::Resolved {
                    urn: identity.urn,
                    name: identity.name,
                    ens: identity.ens,
                };
            }
            Self::Resolved { .. } => {}
        }
        Ok(self)
    }
}

impl From<Urn> for Identity {
    fn from(urn: Urn) -> Self {
        Self::Unresolved { urn }
    }
}

/// Author.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Author {
    pub peer: PeerId,
    pub identity: Identity,
}

impl Author {
    pub fn new(urn: Urn, peer: PeerId) -> Self {
        Self {
            peer,
            identity: Identity::Unresolved { urn },
        }
    }

    pub fn name(&self) -> String {
        self.identity.name()
    }

    pub fn urn(&self) -> &Urn {
        self.identity.urn()
    }

    pub fn resolve<S: AsRef<ReadOnly>>(&mut self, storage: &S) -> Result<&Author, ResolveError> {
        self.identity.resolve(storage)?;

        Ok(self)
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

impl<R: Default> Comment<R> {
    pub fn new(author: Author, body: String, timestamp: Timestamp) -> Self {
        Self {
            author,
            body,
            reactions: HashMap::default(),
            replies: R::default(),
            timestamp,
        }
    }
}

impl Comment<()> {
    pub fn resolve<S: AsRef<ReadOnly>>(&mut self, storage: &S) -> Result<&Author, ResolveError> {
        self.author.resolve(storage)
    }

    pub(super) fn put(
        &self,
        tx: &mut automerge::transaction::Transaction,
        id: &automerge::ObjId,
    ) -> Result<(), AutomergeError> {
        let comment_id = tx.put_object(&id, "comment", ObjType::Map)?;

        assert!(
            self.reactions.is_empty(),
            "Cannot put comment with non-empty reactions"
        );

        tx.put(&comment_id, "body", self.body.trim())?;
        tx.put(&comment_id, "author", self.author.urn().to_string())?;
        tx.put(&comment_id, "peer", self.author.peer.default_encoding())?;
        tx.put(&comment_id, "timestamp", self.timestamp)?;
        tx.put_object(&comment_id, "reactions", ObjType::Map)?;

        Ok(())
    }
}

impl Comment<Replies> {
    pub(super) fn put(
        &self,
        tx: &mut automerge::transaction::Transaction,
        id: &automerge::ObjId,
    ) -> Result<(), AutomergeError> {
        let comment_id = tx.put_object(&id, "comment", ObjType::Map)?;

        assert!(
            self.reactions.is_empty(),
            "Cannot put comment with non-empty reactions"
        );
        assert!(
            self.replies.is_empty(),
            "Cannot put comment with non-empty replies"
        );

        tx.put(&comment_id, "body", self.body.trim())?;
        tx.put(&comment_id, "author", self.author.urn().to_string())?;
        tx.put(&comment_id, "peer", self.author.peer.default_encoding())?;
        tx.put(&comment_id, "timestamp", self.timestamp)?;
        tx.put_object(&comment_id, "reactions", ObjType::Map)?;
        tx.put_object(&comment_id, "replies", ObjType::List)?;

        Ok(())
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

    pub fn to_rfc2822(&self) -> String {
        chrono::Utc.timestamp(self.as_secs() as i64, 0).to_rfc2822()
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let fmt = timeago::Formatter::new();
        let now = Timestamp::now();
        let duration = time::Duration::from_secs(now.seconds - self.seconds);

        write!(f, "{}", fmt.convert(duration))
    }
}

impl From<Timestamp> for ScalarValue {
    fn from(ts: Timestamp) -> Self {
        ScalarValue::Timestamp(ts.seconds as i64)
    }
}

impl<'a> FromValue<'a> for Timestamp {
    fn from_value(val: Value<'a>) -> Result<Self, ValueError> {
        if let Value::Scalar(scalar) = &val {
            if let ScalarValue::Timestamp(ts) = scalar.borrow() {
                return Ok(Self {
                    seconds: *ts as u64,
                });
            }
        }
        Err(ValueError::InvalidValue(val.to_string()))
    }
}

/// Implemented by types that can be converted from a [`Value`].
pub trait FromValue<'a>: Sized {
    fn from_value(val: Value<'a>) -> Result<Self, ValueError>;
}

impl<'a> FromValue<'a> for PeerId {
    fn from_value(val: Value<'a>) -> Result<PeerId, ValueError> {
        let peer = String::from_value(val)?;
        let peer = PeerId::from_str(&peer).map_err(|e| ValueError::Other(Arc::new(e)))?;

        Ok(peer)
    }
}

impl<'a> FromValue<'a> for uuid::Uuid {
    fn from_value(val: Value<'a>) -> Result<uuid::Uuid, ValueError> {
        let uuid = String::from_value(val)?;
        let uuid = uuid::Uuid::from_str(&uuid).map_err(|e| ValueError::Other(Arc::new(e)))?;

        Ok(uuid)
    }
}

impl<'a> FromValue<'a> for Urn {
    fn from_value(val: Value<'a>) -> Result<Urn, ValueError> {
        let urn = String::from_value(val)?;
        let urn = Urn::from_str(&urn).map_err(|e| ValueError::Other(Arc::new(e)))?;

        Ok(urn)
    }
}

impl<'a> FromValue<'a> for git::Oid {
    fn from_value(val: Value<'a>) -> Result<git::Oid, ValueError> {
        let oid = String::from_value(val)?;
        let oid = git::Oid::from_str(&oid).map_err(|e| ValueError::Other(Arc::new(e)))?;

        Ok(oid)
    }
}

impl<'a> FromValue<'a> for git::OneLevel {
    fn from_value(val: Value<'a>) -> Result<git::OneLevel, ValueError> {
        let one = String::from_value(val)?;
        let reflike = git::RefLike::try_from(one).map_err(|e| ValueError::Other(Arc::new(e)))?;
        let one = git::OneLevel::try_from(reflike).map_err(|e| ValueError::Other(Arc::new(e)))?;

        Ok(one)
    }
}

impl<'a> FromValue<'a> for String {
    fn from_value(val: Value) -> Result<String, ValueError> {
        val.into_string().map_err(|_| ValueError::InvalidType)
    }
}

/// Automerge document decoder.
///
/// Wraps a document, providing convenience functions. Derefs to the underlying doc.
#[derive(Copy, Clone)]
pub struct Document<'a> {
    doc: &'a Automerge,
}

impl<'a> Document<'a> {
    pub fn new(doc: &'a Automerge) -> Self {
        Self { doc }
    }

    pub fn get<O: AsRef<automerge::ObjId>, P: Into<automerge::Prop>>(
        &self,
        id: O,
        prop: P,
    ) -> Result<(automerge::Value<'a>, automerge::ObjId), DocumentError> {
        let prop = prop.into();

        self.doc
            .get(id.as_ref(), prop.clone())?
            .ok_or(DocumentError::PropertyNotFound(prop.to_string()))
    }

    pub fn val<O: AsRef<automerge::ObjId>, P: Into<automerge::Prop>, V: FromValue<'a>>(
        &self,
        id: O,
        prop: P,
    ) -> Result<V, DocumentError> {
        let prop = prop.into();
        let (val, _) = Document::get(self, id, prop)?;

        V::from_value(val).map_err(DocumentError::from)
    }

    pub fn lookup<V, O: AsRef<automerge::ObjId>, P: Into<automerge::Prop>>(
        &self,
        id: O,
        prop: P,
        lookup: fn(Document, &automerge::ObjId) -> Result<V, DocumentError>,
    ) -> Result<V, DocumentError> {
        let (_, obj_id) = self.get(&id, prop)?;
        lookup(*self, &obj_id)
    }

    pub fn list<V, O: AsRef<automerge::ObjId>, P: Into<automerge::Prop>>(
        &self,
        id: O,
        prop: P,
        item: fn(Document, &automerge::ObjId) -> Result<V, DocumentError>,
    ) -> Result<Vec<V>, DocumentError> {
        let prop = prop.into();
        let id = id.as_ref();
        let (list, list_id) = self
            .doc
            .get(id, prop.clone())?
            .ok_or_else(|| DocumentError::PropertyNotFound(prop.to_string()))?;

        assert_eq!(list.to_objtype(), Some(ObjType::List));

        let mut objs: Vec<V> = Vec::new();
        for i in 0..self.length(&list_id) {
            let (_, item_id) = self
                .doc
                .get(&list_id, i as usize)?
                .ok_or_else(|| DocumentError::PropertyNotFound(prop.to_string()))?;
            let item = item(*self, &item_id)?;

            objs.push(item);
        }
        Ok(objs)
    }

    pub fn map<
        V: Default,
        K: Hash + Eq + FromStr,
        O: AsRef<automerge::ObjId>,
        P: Into<automerge::Prop>,
    >(
        &self,
        id: O,
        prop: P,
        mut value: impl FnMut(&mut V),
    ) -> Result<HashMap<K, V>, DocumentError> {
        let prop = prop.into();
        let id = id.as_ref();

        let (obj, obj_id) = self
            .doc
            .get(id, prop.clone())?
            .ok_or_else(|| DocumentError::PropertyNotFound(prop.to_string()))?;

        assert_eq!(obj.to_objtype(), Some(ObjType::Map));

        let mut map = HashMap::new();
        for key in self.doc.keys(&obj_id) {
            let key = K::from_str(&key).map_err(|_| DocumentError::Property)?;
            let val = map.entry(key).or_default();

            value(val);
        }
        Ok(map)
    }

    pub fn keys<K: Hash + Eq + FromStr, O: AsRef<automerge::ObjId>, P: Into<automerge::Prop>>(
        &self,
        id: O,
        prop: P,
    ) -> Result<HashSet<K>, DocumentError> {
        let prop = prop.into();
        let id = id.as_ref();

        let (obj, obj_id) = self
            .doc
            .get(id, prop.clone())?
            .ok_or_else(|| DocumentError::PropertyNotFound(prop.to_string()))?;

        assert_eq!(obj.to_objtype(), Some(ObjType::Map));

        let mut keys = HashSet::new();
        for key in self.doc.keys(&obj_id) {
            let key = K::from_str(&key).map_err(|_| DocumentError::Property)?;

            keys.insert(key);
        }
        Ok(keys)
    }
}

impl<'a> Deref for Document<'a> {
    type Target = Automerge;

    fn deref(&self) -> &Self::Target {
        self.doc
    }
}

/// Error decoding a document.
#[derive(thiserror::Error, Debug)]
pub enum DocumentError {
    #[error(transparent)]
    Automerge(#[from] AutomergeError),
    #[error("property '{0}' not found in object")]
    PropertyNotFound(String),
    #[error("error decoding property")]
    Property,
    #[error("error decoding value: {0}")]
    Value(#[from] ValueError),
    #[error("list cannot be empty")]
    EmptyList,
}

pub mod lookup {
    use super::{Author, Comment, HashMap, Reaction, Replies};
    use super::{Document, DocumentError};

    pub fn comment(doc: Document, obj_id: &automerge::ObjId) -> Result<Comment<()>, DocumentError> {
        let peer = doc.val(&obj_id, "peer")?;
        let author = doc
            .val(&obj_id, "author")
            .map(|urn| Author::new(urn, peer))?;
        let body = doc.val(&obj_id, "body")?;
        let timestamp = doc.val(&obj_id, "timestamp")?;
        let reactions: HashMap<Reaction, usize> = doc.map(&obj_id, "reactions", |v| *v += 1)?;

        Ok(Comment {
            author,
            body,
            reactions,
            replies: (),
            timestamp,
        })
    }

    pub fn thread(
        doc: Document,
        obj_id: &automerge::ObjId,
    ) -> Result<Comment<Replies>, DocumentError> {
        let comment = self::comment(doc, obj_id)?;
        let replies = doc.list(&obj_id, "replies", self::comment)?;

        Ok(Comment {
            author: comment.author,
            body: comment.body,
            reactions: comment.reactions,
            replies,
            timestamp: comment.timestamp,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_color() {
        let c = Color::from_str("#ffccaa").unwrap();
        assert_eq!(c.to_string(), "#ffccaa".to_owned());
        assert_eq!(serde_json::to_string(&c).unwrap(), "\"#ffccaa\"".to_owned());
        assert_eq!(serde_json::from_str::<'_, Color>("\"#ffccaa\"").unwrap(), c);

        let c = Color::from_str("#0000aa").unwrap();
        assert_eq!(c.to_string(), "#0000aa".to_owned());

        let c = Color::from_str("#aa0000").unwrap();
        assert_eq!(c.to_string(), "#aa0000".to_owned());

        let c = Color::from_str("#00aa00").unwrap();
        assert_eq!(c.to_string(), "#00aa00".to_owned());

        Color::from_str("#aa00").unwrap_err();
        Color::from_str("#abc").unwrap_err();
    }
}
