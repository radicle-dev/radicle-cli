use std::convert::TryFrom;
use std::ops::ControlFlow;
use std::str::FromStr;

use automerge::{Automerge, AutomergeError, ObjType, ScalarValue, Value};
use lazy_static::lazy_static;
use nonempty::NonEmpty;

use librad::collaborative_objects::{
    CollaborativeObjects, EntryContents, NewObjectSpec, ObjectId, TypeName, UpdateObjectSpec,
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

    #[error("Retrieve error: {0}")]
    Retrieve(String),

    #[error(transparent)]
    Automerge(#[from] AutomergeError),
}

#[derive(Debug)]
pub struct Comment {
    pub author: Urn,
    pub body: String,
}

pub fn author(val: Value) -> Result<Urn, AutomergeError> {
    let author = val.into_string().unwrap();
    let author = Urn::from_str(&author).unwrap();

    Ok(author)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug)]
pub struct Issue {
    pub author: Urn,
    pub title: String,
    pub state: State,
    pub comments: NonEmpty<Comment>,
    pub automerge: Automerge,
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
        &self.comments.head.body
    }

    pub fn comments(&self) -> &[Comment] {
        &self.comments.tail
    }
}

impl TryFrom<Automerge> for Issue {
    type Error = AutomergeError;

    fn try_from(doc: Automerge) -> Result<Self, Self::Error> {
        let (_obj, obj_id) = doc.get(automerge::ObjId::Root, "issue")?.unwrap();
        let (title, _) = doc.get(&obj_id, "title")?.unwrap();
        let (comments, comments_id) = doc.get(&obj_id, "comments")?.unwrap();
        let (author, _) = doc.get(&obj_id, "author")?.unwrap();
        let (state, _) = doc.get(&obj_id, "state")?.unwrap();

        assert_eq!(comments.to_objtype(), Some(ObjType::List));

        let mut comments = Vec::new();
        for i in 0..doc.length(&comments_id) {
            let (_val, comment_id) = doc.get(&comments_id, i as usize)?.unwrap();
            let (author, _) = doc.get(&comment_id, "author")?.unwrap();
            let (body, _) = doc.get(&comment_id, "body")?.unwrap();

            let author = self::author(author)?;
            let body = body.into_string().unwrap();

            comments.push(Comment { author, body });
        }
        let author = self::author(author)?;
        let comments = NonEmpty::from_vec(comments).unwrap();
        let state = State::try_from(state).unwrap();

        Ok(Self {
            title: title.into_string().unwrap(),
            state,
            author,
            comments,
            automerge: doc,
        })
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
        let history = events::create(&author, title, description)?;
        let cob = self
            .store
            .create(
                &self.whoami,
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

    pub fn comment(
        &self,
        project: &Urn,
        issue_id: &ObjectId,
        body: &str,
    ) -> Result<ObjectId, Error> {
        let author = self.whoami.urn();
        let mut issue = self.get_raw(project, issue_id)?.unwrap();
        let changes = events::comment(&mut issue, &author, body)?;
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

    pub fn get(&self, project: &Urn, id: &ObjectId) -> Result<Option<Issue>, Error> {
        let cob = self
            .store
            .retrieve(project, &TYPENAME, id)
            .map_err(|e| Error::Retrieve(e.to_string()))?;

        let cob = if let Some(cob) = cob {
            cob
        } else {
            return Ok(None);
        };

        let doc = cob.history().traverse(Automerge::new(), |mut doc, entry| {
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

        Ok(Some(issue))
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

                    let comments = tx.put_object(&issue, "comments", ObjType::List)?;
                    let comment = tx.insert_object(&comments, 0, ObjType::Map)?;

                    tx.put(&comment, "body", description)?;
                    tx.put(&comment, "author", author.to_string())?;

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
    ) -> Result<EntryContents, AutomergeError> {
        let _comment = issue
            .transact_with::<_, _, AutomergeError, _, ()>(
                |_| CommitOptions::default().with_message("Add comment".to_owned()),
                |tx| {
                    let (_obj, obj_id) = tx.get(ObjId::Root, "issue")?.unwrap();
                    let (_, comments) = tx.get(&obj_id, "comments")?.unwrap();

                    let length = tx.length(&comments);
                    let comment = tx.insert_object(&comments, length, ObjType::Map)?;

                    tx.put(&comment, "author", author.to_string())?;
                    tx.put(&comment, "body", body)?;

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

        person::set_local(&storage, &person);

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

        assert_eq!(issue.title(), "My first issue");
        assert_eq!(issue.author(), &author);
        assert_eq!(issue.description(), "Blah blah blah.");
        assert_eq!(issue.comments().len(), 0);
        assert_eq!(issue.state(), State::Open);
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
    fn test_issue_comment() {
        let (storage, profile, whoami, project) = setup();
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
    }
}
