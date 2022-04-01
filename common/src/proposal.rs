use nonempty::NonEmpty;

pub use serde::{Deserialize, Serialize};
pub use git2::{Note, Oid, Repository};

/// Commit hash
type Revision = Oid;


/// Data structure as specified in RFC. A "proposal" is conceptually similar
/// to a "topic branch": a series of one or more commits made on top of some
/// commit within the ancestry graph of the project's main branch.
pub struct Metadata {
    // The title should contain a very short and concise summary of
    // this proposal.
    pub title: String,
    // List of revisions (commits) included in this proposal. The field `head` 
    // of this `NonEmpty` object is the most recent revision.
    pub revisions: NonEmpty<Revision>,
    // Description or "cover letter" of this proposal.
    pub description: Option<String>,
}

#[cfg(test)]
mod test {
    use assay::assay;
    use super::*;

    #[assay]
    fn fields_can_be_initialized() {
        let metadata = Metadata {
            patch: Patch {
                title: "Title".to_owned(),
                revisions: NonEmpty {
                    head: Oid::zero(),
                    tail: vec![Oid::zero()],
                },
            },
            description: Some("Description".to_owned()),
        };

        assert_eq!(metadata.patch.title, "Title".to_owned());
        assert_eq!(metadata.patch.revisions.head, Oid::zero());
        assert_eq!(metadata.description, Some("Description".to_owned()));
    }
}