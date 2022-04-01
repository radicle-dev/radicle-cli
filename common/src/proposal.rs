use nonempty::NonEmpty;

pub use git2::{Note, Oid, Repository};
pub use serde::{Deserialize, Serialize};

/// Commit hash
type Revision = Oid;

/// Content types supported by proposals.
pub enum ContentType {
    Plain,
    Markdown,
}

/// Typed text content used by proposals.
pub struct Text {
    pub content: String,
    pub mime: ContentType,
}

/// Data structure as specified in RFC. A "proposal" is conceptually similar
/// to a "topic branch": a series of one or more commits made on top of some
/// commit within the ancestry graph of the project's main branch.
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

#[cfg(test)]
mod test {
    use super::*;
    use assay::assay;

    #[assay]
    fn fields_can_be_initialized() {
        let metadata = Metadata {
            title: "Title".to_owned(),
            description: Some(Text {
                content: "Description".to_owned(),
                mime: ContentType::Plain,
            }),
            revisions: NonEmpty {
                head: Oid::zero(),
                tail: vec![Oid::zero()],
            },
        };

        assert_eq!(metadata.title, "Title".to_owned());
        assert_eq!(metadata.revisions.head, Oid::zero());
        assert_eq!(
            metadata.description.unwrap().content,
            "Description".to_owned()
        );
        assert_eq!(metadata.description.unwrap().mime, ContentType::Plain);
    }
}
