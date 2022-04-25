//! Manage issues.
use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::time::{SystemTime, UNIX_EPOCH};

use librad::git::types::{Many, Namespace, Reference};
use librad::git::{identities::local::LocalIdentity, Storage, Urn};

use radicle_git_ext::RefLike;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// Unix timestamp (seconds since Epoch).
pub type Timestamp = u64;

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone, Serialize, Deserialize)]
pub struct Id {
    #[serde(with = "string")]
    oid: git2::Oid,
}

impl From<git2::Oid> for Id {
    fn from(oid: git2::Oid) -> Self {
        Self { oid }
    }
}

impl std::fmt::Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.oid)
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Label {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Issue {
    pub project: Urn,
    pub title: String,
    pub description: String,
    pub author: Urn,
    pub timestamp: Timestamp,
    pub labels: HashSet<Label>,
    pub assignees: HashSet<Urn>,
    pub related: HashSet<Urn>,
}

impl Issue {
    pub fn new(
        project: Urn,
        title: String,
        description: String,
        author: Urn,
        timestamp: Timestamp,
    ) -> Self {
        Self {
            project,
            title,
            description,
            author,
            timestamp,
            labels: HashSet::new(),
            assignees: HashSet::new(),
            related: HashSet::new(),
        }
    }

    pub fn namespace(project: &Urn, id: Id) -> Reference<Many> {
        let namespace = Namespace::from(project);
        let refname = RefLike::try_from(format!("issues/{id}")).unwrap();

        Reference::rads(namespace, None).with_name(&refname)
    }

    pub fn head(project: &Urn, id: Id) -> Reference<Many> {
        let namespace = Self::namespace(project, id);
        let head = RefLike::try_from(format!("{}/head", namespace.name)).unwrap();

        namespace.with_name(head)
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reaction {
    emoji: char,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Comment {
    pub body: String,
    pub author: Urn,
    pub timestamp: Timestamp,
    pub parents: HashSet<Id>,
    pub reply_to: Option<Id>,
    pub reactions: HashMap<Reaction, usize>,
}

impl Comment {
    pub fn head(project: &Urn, issue: Id, id: Id) -> Reference<Many> {
        let namespace = Issue::namespace(project, issue);
        let head = RefLike::try_from(format!("{}/comments/{}", namespace.name, id)).unwrap();

        namespace.with_name(head)
    }
}

/// Issues backend in Git.
pub struct Backend {
    whoami: LocalIdentity,
    repo: git2::Repository,
}

impl Backend {
    pub fn new(whoami: LocalIdentity, storage: &Storage) -> Result<Self, git2::Error> {
        let repo = git2::Repository::open_bare(storage.path())?;
        Ok(Self { whoami, repo })
    }

    /// Create an issue.
    ///
    /// Creeates a ref that points to a commit under which is a blob with a JSON
    /// serialization of the issue.
    ///
    ///     /git/refs/namespaces/<proj>/refs/rad/issues/<id>/head -> <commit>
    ///
    /// The issue *id* is the oid of the initial JSON blob. The ref points to the
    /// last commit.
    ///
    pub fn issue(
        &self,
        project: &Urn,
        title: impl Into<String>,
        description: impl Into<String>,
        labels: impl IntoIterator<Item = Label>,
        assignees: impl IntoIterator<Item = Urn>,
        related: impl IntoIterator<Item = Urn>,
    ) -> Result<(Id, Issue, git2::Reference), git2::Error> {
        let issue = Issue {
            project: project.clone(),
            title: title.into(),
            description: description.into(),
            author: self.whoami.urn(),
            timestamp: self::timestamp(),
            labels: labels.into_iter().collect(),
            assignees: assignees.into_iter().collect(),
            related: related.into_iter().collect(),
        };

        let (id, commit) = self.blob(&issue, "issue")?;
        let head = Issue::head(project, id);
        let reference =
            self.repo
                .reference(head.to_string().as_str(), commit, false, "Create new issue")?;

        Ok((id, issue, reference))
    }

    pub fn get_issue(&self, project: &Urn, id: Id) -> Result<Issue, git2::Error> {
        let head = Issue::head(project, id).to_string();
        let value = self.get_blob(&head)?;

        Ok(value)
    }

    pub fn comment(
        &self,
        project: &Urn,
        issue: Id,
        body: impl Into<String>,
    ) -> Result<(Id, Comment, git2::Reference), git2::Error> {
        let comment = Comment {
            author: self.whoami.urn(),
            body: body.into(),
            timestamp: self::timestamp(),
            reply_to: None,
            parents: HashSet::new(),
            reactions: HashMap::new(),
        };
        let (id, commit) = self.blob(&comment, "comment")?;
        let head = Comment::head(project, issue, id);
        let reference = self.repo.reference(
            head.to_string().as_str(),
            commit,
            false,
            "Create new comment",
        )?;

        Ok((id, comment, reference))
    }

    pub fn get_comment(&self, project: &Urn, issue: Id, id: Id) -> Result<Comment, git2::Error> {
        let refname = Comment::head(project, issue, id).to_string();
        let value = self.get_blob(&refname)?;

        Ok(value)
    }

    ////////////////////////////////////////////////////////////////////////////

    fn get_blob<T: DeserializeOwned>(&self, refname: &str) -> Result<T, git2::Error> {
        let head = self.repo.find_reference(refname)?;
        let commit = head.peel_to_commit()?;
        let tree = commit.tree()?;
        let entry = tree.get(0).unwrap();
        let obj = entry.to_object(&self.repo)?;
        let blob = obj.into_blob().unwrap();
        let content = blob.content();
        let json = std::str::from_utf8(content).unwrap();
        let value = serde_json::from_str(json).unwrap();

        Ok(value)
    }

    fn blob<T: Serialize>(&self, value: &T, log: &str) -> Result<(Id, git2::Oid), git2::Error> {
        let json = serde_json::to_vec(value).unwrap();
        let blob = self.repo.blob(&json)?;
        let tree = {
            let mut builder = self.repo.treebuilder(None)?;
            builder.insert(blob.to_string(), blob, 0o100644)?;
            self.repo.find_tree(builder.write()?)?
        };
        let id = Id::from(blob);
        let name = self.whoami.subject().name.to_string();
        let committer = git2::Signature::now(name.as_str(), &format!("{name}@radicle.local"))?;
        let commit = self.repo.commit(
            None,
            &committer,
            &committer,
            &format!("{log} (create)"),
            &tree,
            &[],
        )?;

        Ok((id, commit))
    }
}

pub fn timestamp() -> Timestamp {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

mod string {
    use std::fmt::Display;

    use serde::{Deserializer, Serializer};

    pub fn serialize<T, S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        T: Display,
        S: Serializer,
    {
        serializer.collect_str(value)
    }

    pub fn deserialize<'a, D, T, E>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'a>,
        T: std::str::FromStr<Err = E>,
        E: std::fmt::Display,
    {
        let buf: &str = serde::de::Deserialize::deserialize(deserializer)?;
        T::from_str(buf).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod test {
    use std::env;
    use std::path::Path;

    use assay::assay;

    use librad::crypto::keystore::crypto::{Pwhash, KDF_PARAMS_TEST};
    use librad::crypto::keystore::pinentry::SecUtf8;
    use librad::git::identities::Project;

    use librad::profile::LNK_HOME;

    use super::*;
    use crate::{keys, person, project, test};

    fn setup() -> (Backend, Project) {
        let tempdir = env::temp_dir().join("rad").join("home");
        let home = env::var(LNK_HOME)
            .map(|s| Path::new(&s).to_path_buf())
            .unwrap_or_else(|_| tempdir.to_path_buf());

        env::set_var(LNK_HOME, home);

        let sock = keys::ssh_auth_sock();
        let name = "cloudhead";
        let pass = Pwhash::new(SecUtf8::from(test::USER_PASS), *KDF_PARAMS_TEST);
        let (profile, _peer_id) = lnk_profile::create(None, pass.clone()).unwrap();

        keys::add(&profile, pass, sock.clone()).unwrap();

        let (signer, storage) = keys::storage(&profile, sock).unwrap();
        let person = person::create(&profile, name, signer, &storage).unwrap();

        person::set_local(&storage, &person);

        let whoami = person::local(&storage).unwrap();
        let backend = Backend::new(whoami, &storage).unwrap();
        let payload = project::payload(
            "nakamoto".to_owned(),
            "Bitcoin light-client".to_owned(),
            "master".to_owned(),
        );
        (backend, project::create(payload, &storage).unwrap())
    }

    #[test]
    fn test_issue_head() {
        let urn = Urn::try_from_id("hnrkbjg7r54q48sqsaho1n4qfxhi4nbmdh51y").unwrap();
        let oid = git2::Oid::from_str("53d4c844582a67fe355845fb65fe89800c887c37").unwrap();
        let issue = Id::from(oid);
        let head = Issue::head(&urn, issue);

        assert_eq!(
            head.to_string(),
            "refs/namespaces/hnrkbjg7r54q48sqsaho1n4qfxhi4nbmdh51y/refs/rad/issues/53d4c844582a67fe355845fb65fe89800c887c37/head"
        );
    }

    #[test]
    fn test_comment_head() {
        let urn = Urn::try_from_id("hnrkbjg7r54q48sqsaho1n4qfxhi4nbmdh51y").unwrap();
        let oid1 = git2::Oid::from_str("53d4c844582a67fe355845fb65fe89800c887c37").unwrap();
        let oid2 = git2::Oid::from_str("998192f1c7ea0036abbc39da48af71843258ec2c").unwrap();
        let issue = Id::from(oid1);
        let comment = Id::from(oid2);
        let head = Comment::head(&urn, issue, comment);

        assert_eq!(
            head.to_string(),
            "refs/namespaces/hnrkbjg7r54q48sqsaho1n4qfxhi4nbmdh51y/refs/rad/issues/53d4c844582a67fe355845fb65fe89800c887c37/comments/998192f1c7ea0036abbc39da48af71843258ec2c"
        );
    }

    #[assay(
        teardown = test::teardown::profiles()?,
    )]
    fn smoke_test() {
        let (backend, project) = self::setup();
        let (i, _, _) = backend
            .issue(
                &project.urn(),
                "Nothing works",
                "Long explanation about how nothing works.",
                [],
                [],
                [],
            )
            .unwrap();

        let (c1, _, _) = backend
            .comment(&project.urn(), i, "That's too bad.")
            .unwrap();
        let (c2, _, _) = backend
            .comment(&project.urn(), i, "I really think that's too bad.")
            .unwrap();

        let issue = backend.get_issue(&project.urn(), i).unwrap();
        assert_eq!(&issue.title, "Nothing works");
        assert_eq!(&issue.project, &project.urn());

        let c1 = backend.get_comment(&project.urn(), i, c1).unwrap();
        assert_eq!(&c1.body, "That's too bad.");

        let c2 = backend.get_comment(&project.urn(), i, c2).unwrap();
        assert_eq!(&c2.body, "I really think that's too bad.");
    }
}
