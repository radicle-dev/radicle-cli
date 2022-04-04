//! Proposal-related functions and types.
pub use git2::{Note, Repository};
pub use nonempty::NonEmpty;
pub use serde::{Deserialize, Serialize};

/// Commit hash
type Revision = librad::git_ext::Oid;

/// Content types supported by text fields in proposal data.
#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ContentType {
    Plain,
    Markdown,
}

/// Representation of textual user input in proposal data.
#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Text {
    pub content: String,
    pub mime: ContentType,
}

/// Data structure as specified in RFC (link below). A "proposal" is
/// conceptually similar to a "topic branch": a series of one or more
/// commits made on top of some commit within the ancestry graph of the
/// project's main branch.
/// RFC: https://lists.sr.ht/~radicle-link/dev/patches/25724
#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Metadata {
    // The title should contain a very short and concise summary of
    // this proposal.
    pub title: String,
    // Description or "cover letter" of this proposal. The content is typed
    // and can be one of the types defined by the `ContentType`.
    pub description: Option<Text>,
    // List of revisions (commits) included in this proposal. The field `head`
    // of this `NonEmpty` object is the most recent revision.
    pub revisions: NonEmpty<Revision>,
}

/// Creates an empty proposal referencing a zeroed-out OID.
impl Default for Metadata {
    fn default() -> Self {
        Metadata {
            title: "".to_owned(),
            description: Some(Text {
                content: "".to_owned(),
                mime: ContentType::Plain,
            }),
            revisions: NonEmpty {
                head: Revision::from(git2::Oid::zero()),
                tail: vec![],
            },
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use assay::assay;

    const PROPOSAL_VALID: &str = r#"{
  "title": "A title",
  "description": {
    "content": "A short description",
    "mime": "Plain"
  },
  "revisions": [
    "0000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000"
  ]
}"#;

    #[assay]
    fn fields_can_be_initialized() {
        let metadata = Metadata {
            title: "Title".to_owned(),
            description: Some(Text {
                content: "Description".to_owned(),
                mime: ContentType::Plain,
            }),
            revisions: NonEmpty {
                head: Revision::from(git2::Oid::zero()),
                tail: vec![],
            },
        };

        assert_eq!(metadata.title, "Title".to_owned());
        assert_eq!(
            metadata.description.as_ref().unwrap().content,
            "Description".to_owned()
        );
        assert_eq!(
            metadata.description.as_ref().unwrap().mime,
            ContentType::Plain
        );
        assert_eq!(metadata.revisions.head, Revision::from(git2::Oid::zero()));
    }

    #[assay]
    fn can_be_serialized() {
        let metadata = Metadata {
            title: "A title".to_owned(),
            description: Some(Text {
                content: "A short description".to_owned(),
                mime: ContentType::Plain,
            }),
            revisions: NonEmpty {
                head: Revision::from(git2::Oid::zero()),
                tail: vec![Revision::from(git2::Oid::zero())],
            },
        };
        assert_eq!(serde_json::to_string_pretty(&metadata)?, PROPOSAL_VALID);
    }

    #[assay]
    fn can_be_deserialized() {
        let fixture = Metadata {
            title: "A title".to_owned(),
            description: Some(Text {
                content: "A short description".to_owned(),
                mime: ContentType::Plain,
            }),
            revisions: NonEmpty {
                head: Revision::from(git2::Oid::zero()),
                tail: vec![Revision::from(git2::Oid::zero())],
            },
        };
        let result: Metadata = serde_json::from_str(PROPOSAL_VALID)?;
        assert_eq!(result, fixture);
    }

    #[assay]
    fn empty_revisions_cannot_be_deserialized() {
        let proposal = serde_json::json!({
            "title": "A title",
            "description": {
                "content": "A short description",
                "mime": "Plain"
            },
            "revisions": []
        });
        assert!(serde_json::from_str::<Metadata>(&proposal.to_string()).is_err())
    }

    #[assay]
    fn wrong_mime_cannot_be_deserialized() {
        let proposal = serde_json::json!({
            "title": "A title",
            "description": {
                "content": "A short description",
                "mime": "Foo"
            },
            "revisions": [
                "0000000000000000000000000000000000000000",
                "0000000000000000000000000000000000000000"
            ]
        });
        assert!(serde_json::from_str::<Metadata>(&proposal.to_string()).is_err())
    }
}
