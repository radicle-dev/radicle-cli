use std::collections::HashSet;
use std::convert::TryFrom;
use std::ops::ControlFlow;
use std::str::FromStr;

use automerge::{Automerge, AutomergeError, ObjType};
use serde::{Deserialize, Serialize};

use librad::collaborative_objects::{
    CollaborativeObjects, EntryContents, History, NewObjectSpec, ObjectId, TypeName,
    UpdateObjectSpec,
};
use librad::git::identities::local::LocalIdentity;
use librad::git::Storage;
use librad::git::Urn;
use librad::paths::Paths;
use librad::PeerId;

use crate::cobs::shared::*;

lazy_static::lazy_static! {
    pub static ref TYPENAME: TypeName = FromStr::from_str("xyz.radicle.user").unwrap();
    pub static ref SCHEMA: serde_json::Value =
        serde_json::from_slice(include_bytes!("user.json")).unwrap();
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum Event {}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    #[serde(flatten)]
    pub event: Event,
    pub timestamp: Timestamp,
}

#[derive(Debug, Clone, Serialize)]
pub struct User {
    pub urn: Urn,
    pub projects: HashSet<Urn>,
    pub activity: Vec<Activity>,
    pub timestamp: Timestamp,
}

impl TryFrom<Document<'_>> for User {
    type Error = DocumentError;

    fn try_from(doc: Document) -> Result<Self, Self::Error> {
        let (_obj, obj_id) = doc.get(automerge::ObjId::Root, "user")?;
        let urn = doc.val(&obj_id, "urn")?;
        let timestamp = doc.val(&obj_id, "timestamp")?;
        let projects = doc.keys(&obj_id, "projects")?;
        let activity = vec![];

        Ok(Self {
            urn,
            projects,
            activity,
            timestamp,
        })
    }
}

impl TryFrom<&History> for User {
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
        let user = User::try_from(Document::new(&doc))?;

        Ok(user)
    }
}

pub struct Users<'a> {
    pub whoami: LocalIdentity,
    pub peer_id: PeerId,

    store: CollaborativeObjects<'a>,
}

impl<'a> Store<'a> for Users<'a> {
    fn type_name() -> TypeName {
        TYPENAME.clone()
    }

    fn store(&self) -> &CollaborativeObjects<'a> {
        &self.store
    }
}

impl<'a> Users<'a> {
    pub fn new(whoami: LocalIdentity, paths: &Paths, storage: &'a Storage) -> Result<Self, Error> {
        let store = storage.collaborative_objects(Some(paths.cob_cache_dir().to_path_buf()));
        let peer_id = *storage.peer_id();

        Ok(Self {
            store,
            whoami,
            peer_id,
        })
    }

    pub fn create(&self) -> Result<(), Error> {
        let timestamp = Timestamp::now();
        let urn = self.whoami.urn();
        let history = events::create(&urn, timestamp)?;

        cobs::create(history, &urn, &self.whoami, &self.store)
    }

    pub fn local(&self) -> Result<Option<User>, Error> {
        let cobs = self
            .store
            .list(&self.whoami.urn(), &TYPENAME)
            .map_err(|e| Error::Retrieve(e.to_string()))?;

        if let Some(cob) = cobs.first() {
            let user = User::try_from(cob.history()).unwrap();
            Ok(Some(user))
        } else {
            Ok(None)
        }
    }

    pub fn local_raw(&self, project: &Urn) -> Result<Option<(ObjectId, Automerge)>, Error> {
        let cob = self
            .store
            .list(project, &TYPENAME)
            .map_err(|e| Error::Retrieve(e.to_string()))?;

        let cob = if let Some(cob) = cob.first() {
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

        Ok(Some((*cob.id(), doc)))
    }

    pub fn add_project(&mut self, project: Urn) -> Result<(), Error> {
        let urn = self.whoami.urn();
        let (user_id, mut user) = self.local_raw(&urn)?.unwrap();
        let changes = events::project(&mut user, &project)?;
        let _cob = self
            .store
            .update(
                &self.whoami,
                &urn,
                UpdateObjectSpec {
                    object_id: user_id,
                    typename: TYPENAME.clone(),
                    message: Some("Add project".to_owned()),
                    changes,
                },
            )
            .map_err(|e| Error::Update(e.to_string()));

        Ok(())
    }
}

mod cobs {
    use super::*;

    pub(super) fn create(
        history: EntryContents,
        person: &Urn,
        whoami: &LocalIdentity,
        store: &CollaborativeObjects,
    ) -> Result<(), Error> {
        let _cob = store
            .create(
                whoami,
                person,
                NewObjectSpec {
                    schema_json: SCHEMA.clone(),
                    typename: TYPENAME.clone(),
                    message: Some("Create user".to_owned()),
                    history,
                },
            )
            .map_err(|e| Error::Create(e.to_string()))?;

        Ok(())
    }
}

mod events {
    use super::*;
    use automerge::{
        transaction::{CommitOptions, Transactable},
        ObjId,
    };

    pub fn create(urn: &Urn, timestamp: Timestamp) -> Result<EntryContents, AutomergeError> {
        let mut doc = Automerge::new();

        doc.transact_with::<_, _, AutomergeError, _, ()>(
            |_| CommitOptions::default().with_message("Create user".to_owned()),
            |tx| {
                let user = tx.put_object(ObjId::Root, "user", ObjType::Map)?;

                tx.put(&user, "urn", urn.to_string())?;
                tx.put(&user, "timestamp", timestamp)?;
                tx.put_object(&user, "projects", ObjType::Map)?;
                tx.put_object(&user, "activity", ObjType::List)?;

                Ok(user)
            },
        )
        .map_err(|failure| failure.error)?;

        Ok(EntryContents::Automerge(doc.save_incremental()))
    }

    pub fn project(user: &mut Automerge, project: &Urn) -> Result<EntryContents, AutomergeError> {
        user.transact_with::<_, _, AutomergeError, _, ()>(
            |_| CommitOptions::default().with_message("Add project".to_owned()),
            |tx| {
                let (_obj, obj_id) = tx.get(ObjId::Root, "user")?.unwrap();
                let (_, projects_id) = tx.get(&obj_id, "projects")?.unwrap();

                tx.put(&projects_id, project.to_string(), true)?;

                Ok(())
            },
        )
        .map_err(|failure| failure.error)?;

        let change = user.get_last_local_change().unwrap().raw_bytes().to_vec();

        Ok(EntryContents::Automerge(change))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test;

    #[test]
    fn test_create() {
        let (storage, profile, whoami, _project) = test::setup::profile();
        let users = Users::new(whoami.clone(), profile.paths(), &storage).unwrap();

        users.create().unwrap();

        let user = users.local().unwrap().unwrap();
        assert_eq!(user.urn, whoami.urn());
    }

    #[test]
    fn test_projects() {
        let (storage, profile, whoami, _project) = test::setup::profile();
        let mut users = Users::new(whoami, profile.paths(), &storage).unwrap();
        let project1 = Urn::from_str("rad:git:hnrkbjokbt439jk3p1dsi67u3mca85yiy7fiy").unwrap();
        let project2 = Urn::from_str("rad:git:hnrkbtw9t1of4ykjy6er4qqwxtc54k9943eto").unwrap();

        users.create().unwrap();
        users.add_project(project1.clone()).unwrap();
        users.add_project(project2.clone()).unwrap();
        users.add_project(project2.clone()).unwrap(); // This should have no effect.

        let user = users.local().unwrap().unwrap();
        assert!(user.projects.contains(&project1));
        assert!(user.projects.contains(&project2));
        assert_eq!(user.projects.len(), 2);
    }
}
