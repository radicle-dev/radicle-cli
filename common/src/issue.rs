//! Manage issues.
#![allow(dead_code)]
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::convert::TryFrom;
use std::time::{SystemTime, UNIX_EPOCH};

use librad::git::types::{Many, Namespace, Reference};
use librad::git::{identities::local::LocalIdentity, Storage, Urn};

use radicle_git_ext::RefLike;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// Unix timestamp (seconds since Epoch).
pub type Timestamp = u64;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone, Serialize, Deserialize)]
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
    pub fn namespace(project: &Urn, id: Id) -> Reference<Many> {
        let namespace = Namespace::from(project);
        let refname = RefLike::try_from(format!("issues/{id}")).unwrap();

        Reference::rads(namespace, None).with_name(&refname)
    }

    pub fn id_ref(project: &Urn, id: Id) -> Reference<Many> {
        let namespace = Self::namespace(project, id);
        let head = RefLike::try_from(format!("{}/id", namespace.name)).unwrap();

        namespace.with_name(head)
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Reaction {
    emoji: char,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Comment {
    pub body: String,
    pub author: Urn,
    pub timestamp: Timestamp,
    pub parents: BTreeSet<Id>,
    pub reply_to: Option<Id>,
    pub reactions: BTreeMap<Reaction, usize>,
}

impl Comment {
    pub fn new(
        body: impl Into<String>,
        author: Urn,
        timestamp: Timestamp,
        parents: impl IntoIterator<Item = Id>,
    ) -> Self {
        Self {
            body: body.into(),
            author,
            timestamp,
            parents: parents.into_iter().collect(),
            reply_to: None,
            reactions: BTreeMap::new(),
        }
    }

    pub fn id(&self) -> Id {
        let json = serde_json::to_vec(self).unwrap();
        let oid = git2::Oid::hash_object(git2::ObjectType::Blob, &json).unwrap();

        Id::from(oid)
    }

    pub fn head_ref(project: &Urn, issue: Id) -> Reference<Many> {
        let namespace = Issue::namespace(project, issue);
        let name = RefLike::try_from(format!("{}/comments/head", namespace.name)).unwrap();

        namespace.with_name(name)
    }

    pub fn id_ref(project: &Urn, issue: Id, id: Id) -> Reference<Many> {
        let namespace = Issue::namespace(project, issue);
        let name = RefLike::try_from(format!("{}/comments/{}", namespace.name, id)).unwrap();

        namespace.with_name(name)
    }
}

/// Unsorted comments graph.
pub type Comments = HashMap<Id, Comment>;

/// Sorted comments list.
pub type SortedComments = Vec<(Id, Comment)>;

/// Projects map.
pub struct Project {
    pub issues: HashMap<Id, (Issue, Comments)>,
}

impl Project {
    pub fn get_issue(&self, id: Id) -> (&Issue, SortedComments) {
        let (issue, comments) = self.issues.get(&id).unwrap();
        let sorter = TopologicalSort::new(&comments);
        let list = sorter
            .sort()
            .into_iter()
            .map(|id| (id, comments.get(&id).unwrap().clone()))
            .collect();

        (issue, list)
    }
}

/// Issues backend in Git.
pub struct Backend {
    pub projects: HashMap<Urn, Project>,

    whoami: LocalIdentity,
    repo: git2::Repository,
}

impl Backend {
    pub fn new(whoami: LocalIdentity, storage: &Storage) -> Result<Self, git2::Error> {
        let repo = git2::Repository::open_bare(storage.path())?;
        let projects = HashMap::new();

        Ok(Self {
            whoami,
            repo,
            projects,
        })
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
        &mut self,
        project_urn: &Urn,
        title: impl Into<String>,
        description: impl Into<String>,
        labels: impl IntoIterator<Item = Label>,
        assignees: impl IntoIterator<Item = Urn>,
        related: impl IntoIterator<Item = Urn>,
    ) -> Result<(Id, &Issue, git2::Reference), git2::Error> {
        let project = self.projects.get_mut(project_urn).unwrap();
        let issue = Issue {
            project: project_urn.clone(),
            title: title.into(),
            description: description.into(),
            author: self.whoami.urn(),
            timestamp: self::timestamp(),
            labels: labels.into_iter().collect(),
            assignees: assignees.into_iter().collect(),
            related: related.into_iter().collect(),
        };

        let (id, commit) = blob(&issue, &self.repo, &self.whoami, "issue")?;
        let head = Issue::id_ref(project_urn, id);
        let reference =
            self.repo
                .reference(head.to_string().as_str(), commit, false, "Create new issue")?;
        let (issue, _) = project
            .issues
            .entry(id)
            .or_insert((issue, Comments::default()));

        Ok((id, issue, reference))
    }

    /// Create a comment under an issue.
    ///
    ///     /git/refs/namespaces/<proj>/refs/rad/issues/<id>/comments/<id> -> <commit>
    ///
    /// When a new comment is added, the `comments/head` ref is updated to point to it.
    ///
    pub fn comment(
        &mut self,
        project_urn: &Urn,
        issue_id: Id,
        body: impl Into<String>,
    ) -> Result<(Id, &Comment, git2::Reference), git2::Error> {
        let project = self.projects.get_mut(project_urn).unwrap();
        let (_, comments) = project.issues.get_mut(&issue_id).unwrap();
        let comment = Comment {
            author: self.whoami.urn(),
            body: body.into(),
            timestamp: self::timestamp(),
            reply_to: None,
            parents: BTreeSet::new(),
            reactions: BTreeMap::new(),
        };
        let (id, commit) = blob(&comment, &self.repo, &self.whoami, "comment")?;
        let id_ref = Comment::id_ref(project_urn, issue_id, id).to_string();
        let reference =
            self.repo
                .reference(id_ref.as_str(), commit, false, "Create new comment")?;

        let head_ref = Comment::head_ref(project_urn, issue_id);
        self.repo.reference_symbolic(
            head_ref.to_string().as_str(),
            id_ref.as_str(),
            true,
            "Set comment head",
        )?;
        let comment = comments.entry(id).or_insert(comment);

        Ok((id, comment, reference))
    }

    ////////////////////////////////////////////////////////////////////////////

    fn get_comment(&self, project: &Urn, issue: Id, id: Id) -> Result<Comment, git2::Error> {
        let refname = Comment::id_ref(project, issue, id).to_string();
        let value = self.get_blob(&refname)?;

        Ok(value)
    }

    fn get_issue(&self, project: &Urn, id: Id) -> Result<Issue, git2::Error> {
        let head = Issue::id_ref(project, id).to_string();
        let value = self.get_blob(&head)?;

        Ok(value)
    }

    /// Get the head comment of all remotes as well as the local one.
    fn get_comment_heads(&self, project: &Urn, issue: Id) -> Result<HashSet<Id>, git2::Error> {
        let head_ref = Comment::head_ref(project, issue).to_string();
        let reference = self.repo.find_reference(&head_ref)?;
        let target = reference.symbolic_target().unwrap();
        let id = target.split('/').next_back().unwrap();
        let oid = git2::Oid::from_str(id).unwrap();

        let mut ids = HashSet::new();
        ids.insert(Id::from(oid));

        Ok(ids)
    }

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
}

struct TopologicalSort<'a> {
    comments: &'a HashMap<Id, Comment>,
    output: Vec<Id>,
    context: Vec<(BTreeSet<Id>, Timestamp)>,
    sorted: HashSet<Id>,
    stack: HashSet<Id>,
    unsorted: Vec<Id>,
}

impl<'a> TopologicalSort<'a> {
    pub fn new(comments: &'a HashMap<Id, Comment>) -> Self {
        Self {
            comments,
            output: Vec::new(),
            context: Vec::new(),
            stack: HashSet::new(),
            sorted: HashSet::new(),
            unsorted: comments.keys().copied().collect(),
        }
    }

    pub fn sort(mut self) -> Vec<Id> {
        while self.sorted.len() < self.comments.len() {
            let id = self.unsorted.pop().unwrap();
            let comment = self.comments.get(&id).unwrap();

            self.visit(id, comment);
        }
        self.output
    }

    fn visit(&mut self, id: Id, comment: &Comment) {
        if self.sorted.contains(&id) {
            return;
        }
        if self.stack.contains(&id) {
            panic!("Graph cycle detected");
        }
        self.stack.insert(id);

        for id in &comment.parents {
            let parent = self.comments.get(id).unwrap();
            self.visit(*id, parent);
        }
        self.stack.remove(&id);
        self.sorted.insert(id);

        let mut iter = self.context.iter().enumerate().rev();
        let ix = loop {
            if let Some((i, (parents, timestamp))) = iter.next() {
                if &comment.parents != parents || &comment.timestamp >= timestamp {
                    break i + 1;
                }
            } else {
                break 0;
            }
        };
        self.output.insert(ix, id);
        self.context
            .insert(ix, (comment.parents.clone(), comment.timestamp));
    }
}

fn blob<T: Serialize>(
    value: &T,
    repo: &git2::Repository,
    whoami: &LocalIdentity,
    log: &str,
) -> Result<(Id, git2::Oid), git2::Error> {
    let json = serde_json::to_vec(value).unwrap();
    let blob = repo.blob(&json)?;
    let tree = {
        let mut builder = repo.treebuilder(None)?;
        builder.insert(blob.to_string(), blob, 0o100644)?;
        repo.find_tree(builder.write()?)?
    };
    let id = Id::from(blob);
    let name = whoami.subject().name.to_string();
    let committer = git2::Signature::now(name.as_str(), &format!("{name}@radicle.local"))?;
    let commit = repo.commit(
        None,
        &committer,
        &committer,
        &format!("{log} (create)"),
        &tree,
        &[],
    )?;

    Ok((id, commit))
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
        let head = Issue::id_ref(&urn, issue);

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
        let head = Comment::id_ref(&urn, issue, comment);

        assert_eq!(
            head.to_string(),
            "refs/namespaces/hnrkbjg7r54q48sqsaho1n4qfxhi4nbmdh51y/refs/rad/issues/53d4c844582a67fe355845fb65fe89800c887c37/comments/998192f1c7ea0036abbc39da48af71843258ec2c"
        );
    }

    #[assay(
        teardown = test::teardown::profiles()?,
    )]
    fn smoke_test() {
        let (mut backend, project) = self::setup();
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

        let heads = backend.get_comment_heads(&project.urn(), i).unwrap();
        assert_eq!(heads.len(), 1);
        assert!(heads.contains(&c2));

        let c1 = backend.get_comment(&project.urn(), i, c1).unwrap();
        assert_eq!(&c1.body, "That's too bad.");

        let c2 = backend.get_comment(&project.urn(), i, c2).unwrap();
        assert_eq!(&c2.body, "I really think that's too bad.");
    }

    #[test]
    fn test_topological_sort() {
        let timestamp = 0;

        let a = Urn::try_from_id("hnrkbjg7r54q48sqsaho1n4qfxhi4nbmdh51y").unwrap();
        let b = Urn::try_from_id("hnrkq5sti58yw41szighu565oprgt5eduhmhy").unwrap();

        // A1 06f8ba9604e13385ea4ab7de0aefd9807de3707b
        // B2 0e7debaa08a26f5eeb099c32fbb9f776ec1899f0
        // A3 3e8d17ded2f18691f8c710c122a2d693c4c490f9
        // B3 00f930e3dea4d24cf0aa9bb5d0a9b19ad3ffcf67
        // B4 01de06ff27ad3462ed69bd0d730f5875c4b09ff4
        // A5 7a8f9c324bf98fb0405cf8b923141f2a0a306f9b

        let a1 = Comment::new("A1", a.clone(), timestamp, []);
        let b2 = Comment::new("B2", b.clone(), timestamp, [a1.id()]);
        let a3 = Comment::new("A3", a.clone(), timestamp + 1, [b2.id()]);
        let b3 = Comment::new("B3", b.clone(), timestamp + 2, [b2.id()]);
        let b4 = Comment::new("B4", b, timestamp, [a3.id(), b3.id()]);
        let a5 = Comment::new("A5", a, timestamp, [b4.id()]);

        let expected = vec![a1, b2, a3, b3, b4, a5];
        let comments: HashMap<Id, Comment> =
            expected.clone().into_iter().map(|c| (c.id(), c)).collect();

        let sorter = TopologicalSort::new(&comments);
        let sorted = sorter.sort();

        assert_eq!(sorted, expected.iter().map(|c| c.id()).collect::<Vec<_>>());
    }
}
