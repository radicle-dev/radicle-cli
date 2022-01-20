use std::path::Path;

use anyhow::Result;
use librad::crypto::peer::PeerId;

use crate::git;

pub fn push_delegate_id(
    repo: &Path,
    seed: &str,
    self_id: &str,
    peer_id: PeerId,
) -> Result<String, anyhow::Error> {
    git::git(
        repo,
        [
            "push",
            "-q",
            "--signed",
            &format!("{}/{}", seed, self_id),
            &format!(
                "refs/namespaces/{}/refs/rad/id:refs/remotes/{}/rad/id",
                self_id, peer_id
            ),
        ],
    )
}

pub fn push_project_id(
    repo: &Path,
    seed: &str,
    project_id: &str,
    peer_id: PeerId,
) -> Result<String, anyhow::Error> {
    git::git(
        repo,
        [
            "push",
            "-q",
            "--signed",
            "--atomic",
            &format!("{}/{}", seed, project_id),
            &format!(
                "refs/namespaces/{}/refs/rad/id:refs/remotes/{}/rad/id",
                project_id, peer_id
            ),
        ],
    )
}

pub fn push_refs(
    repo: &Path,
    seed: &str,
    project_id: &str,
    peer_id: PeerId,
) -> Result<String, anyhow::Error> {
    git::git(
        repo,
        [
            "push",
            "-q",
            "--signed",
            "--atomic",
            &format!("{}/{}", seed, project_id),
            &format!(
                "refs/namespaces/{}/refs/rad/ids/*:refs/remotes/{}/rad/ids/*",
                project_id, peer_id
            ),
            &format!(
                "refs/namespaces/{}/refs/rad/signed_refs:refs/remotes/{}/rad/signed_refs",
                project_id, peer_id
            ),
            &format!(
                "+refs/namespaces/{}/refs/heads/*:refs/remotes/{}/heads/*",
                project_id, peer_id
            ),
        ],
    )
}
