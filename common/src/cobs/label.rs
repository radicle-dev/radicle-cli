#![allow(clippy::large_enum_variant)]

use std::convert::TryFrom;
use std::ops::ControlFlow;
use std::str::FromStr;

use automerge::{Automerge, AutomergeError, ObjType};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

use librad::collaborative_objects::{
    CollaborativeObjects, EntryContents, History, NewObjectSpec, ObjectId, TypeName,
};
use librad::git::identities::local::LocalIdentity;
use librad::git::Storage;
use librad::git::Urn;
use librad::paths::Paths;

use crate::cobs::shared::*;

lazy_static! {
    pub static ref TYPENAME: TypeName = FromStr::from_str("xyz.radicle.label").unwrap();
}

/// Identifier for a label.
pub type LabelId = ObjectId;

/// Describes a label.
#[derive(Debug, PartialEq, Eq, Hash, Clone, Serialize, Deserialize)]
pub struct Label {
    pub name: String,
    pub description: String,
    pub color: Color,
}

impl TryFrom<&History> for Label {
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
        let label = Label::try_from(doc)?;

        Ok(label)
    }
}

impl TryFrom<Automerge> for Label {
    type Error = AutomergeError;

    fn try_from(doc: Automerge) -> Result<Self, Self::Error> {
        let (_, obj_id) = doc.get(automerge::ObjId::Root, "label")?.unwrap();
        let (name, _) = doc.get(&obj_id, "name")?.unwrap();
        let (description, _) = doc.get(&obj_id, "description")?.unwrap();
        let (color, _) = doc.get(&obj_id, "color")?.unwrap();

        let name = name.into_string().unwrap();
        let description = description.into_string().unwrap();
        let color = color.into_string().unwrap();
        let color = Color::from_str(&color).unwrap();

        Ok(Self {
            name,
            description,
            color,
        })
    }
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

pub struct Labels<'a> {
    store: CollaborativeObjects<'a>,
    whoami: LocalIdentity,
}

impl<'a> Labels<'a> {
    pub fn new(whoami: LocalIdentity, paths: &Paths, storage: &'a Storage) -> Result<Self, Error> {
        let store = storage.collaborative_objects(Some(paths.cob_cache_dir().to_path_buf()));

        Ok(Self { store, whoami })
    }

    pub fn create(
        &self,
        project: &Urn,
        name: &str,
        description: &str,
        color: &Color,
    ) -> Result<LabelId, Error> {
        let author = self.whoami.urn();
        let _timestamp = Timestamp::now();
        let history = events::create(&author, name, description, color)?;

        cobs::create(history, project, &self.whoami, &self.store)
    }

    pub fn get(&self, project: &Urn, id: &LabelId) -> Result<Option<Label>, Error> {
        let cob = self
            .store
            .retrieve(project, &TYPENAME, id)
            .map_err(|e| Error::Retrieve(e.to_string()))?;

        if let Some(cob) = cob {
            let label = Label::try_from(cob.history()).unwrap();
            Ok(Some(label))
        } else {
            Ok(None)
        }
    }
}

mod cobs {
    use super::*;

    pub(super) fn create(
        history: EntryContents,
        project: &Urn,
        whoami: &LocalIdentity,
        store: &CollaborativeObjects,
    ) -> Result<LabelId, Error> {
        let cob = store
            .create(
                whoami,
                project,
                NewObjectSpec {
                    typename: TYPENAME.clone(),
                    message: Some("Create label".to_owned()),
                    history,
                },
            )
            .map_err(|e| Error::Create(e.to_string()))?;

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
        _author: &Urn,
        name: &str,
        description: &str,
        color: &Color,
    ) -> Result<EntryContents, AutomergeError> {
        let name = name.trim();
        // TODO: Return error.
        if name.is_empty() {
            panic!("Empty label name");
        }
        let mut doc = Automerge::new();

        doc.transact_with::<_, _, AutomergeError, _, ()>(
            |_| CommitOptions::default().with_message("Create label".to_owned()),
            |tx| {
                let label = tx.put_object(ObjId::Root, "label", ObjType::Map)?;

                tx.put(&label, "name", name)?;
                tx.put(&label, "description", description)?;
                tx.put(&label, "color", color.to_string())?;

                Ok(label)
            },
        )
        .map_err(|failure| failure.error)?;

        Ok(EntryContents::Automerge(doc.save_incremental()))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test;

    #[test]
    fn test_label_create_and_get() {
        let (storage, profile, whoami, project) = test::setup::profile();
        let labels = Labels::new(whoami, profile.paths(), &storage).unwrap();
        let label_id = labels
            .create(
                &project.urn(),
                "bug",
                "Something that doesn't work",
                &Color::from_str("#ff0000").unwrap(),
            )
            .unwrap();
        let label = labels.get(&project.urn(), &label_id).unwrap().unwrap();

        assert_eq!(label.name, "bug");
        assert_eq!(label.description, "Something that doesn't work");
        assert_eq!(label.color.to_string(), "#ff0000");
    }
}
