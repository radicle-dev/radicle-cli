use std::io::{Error, ErrorKind};
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::Result;

use librad::crypto::peer::PeerId;

use rad_terminal::compoments as term;

pub fn push_delegate_id(
    repo: &Path,
    seed: &str,
    self_id: &str,
    peer_id: PeerId,
) -> Result<(), anyhow::Error> {
    let output = Command::new("git")
        .stdout(Stdio::null())
        .current_dir(repo)
        .arg("push")
        .arg("-q")
        .arg("--signed")
        .arg(format!("{}/{}", seed, self_id))
        .arg(format!(
            "refs/namespaces/{}/refs/rad/id:refs/remotes/{}/rad/id",
            self_id, peer_id
        ))
        .output()
        .expect("Git failed to start.");

    match output.status.success() {
        true => Ok(()),
        false => {
            term::error("Could not push delegate id.");
            term::format::error_detail(&format!("{}", String::from_utf8_lossy(&output.stderr)));
            Err(anyhow::Error::new(Error::new(
                ErrorKind::Other,
                String::from_utf8_lossy(&output.stderr),
            )))
        }
    }
}

pub fn push_project_id(
    repo: &Path,
    seed: &str,
    project_id: &str,
    peer_id: PeerId,
) -> Result<(), anyhow::Error> {
    let output = Command::new("git")
        .stdout(Stdio::null())
        .current_dir(repo)
        .arg("push")
        .arg("-q")
        .arg("--signed")
        .arg("--atomic")
        .arg(format!("{}/{}", seed, project_id))
        .arg(format!(
            "refs/namespaces/{}/refs/rad/id:refs/remotes/{}/rad/id",
            project_id, peer_id
        ))
        .output()
        .expect("Git failed to start.");

    match output.status.success() {
        true => Ok(()),
        false => {
            term::error("Could not push project id.");
            term::format::error_detail(&format!("{}", String::from_utf8_lossy(&output.stderr)));
            Err(anyhow::Error::new(Error::new(
                ErrorKind::Other,
                String::from_utf8_lossy(&output.stderr),
            )))
        }
    }
}

pub fn push_refs(
    repo: &Path,
    seed: &str,
    project_id: &str,
    peer_id: PeerId,
) -> Result<(), anyhow::Error> {
    let output = Command::new("git")
        .stdout(Stdio::null())
        .current_dir(repo)
        .arg("push")
        .arg("-q")
        .arg("--signed")
        .arg("--atomic")
        .arg(format!("{}/{}", seed, project_id))
        .arg(format!(
            "refs/namespaces/{}/refs/rad/ids/*:refs/remotes/{}/rad/ids/*",
            project_id, peer_id
        ))
        .arg(format!(
            "refs/namespaces/{}/refs/rad/signed_refs:refs/remotes/{}/rad/signed_refs",
            project_id, peer_id
        ))
        .arg(format!(
            "+refs/namespaces/{}/refs/heads/*:refs/remotes/{}/heads/*",
            project_id, peer_id
        ))
        .output()
        .expect("Git failed to start.");

    match output.status.success() {
        true => Ok(()),
        false => {
            term::error("Could not push other refs.");
            term::format::error_detail(&format!("{}", String::from_utf8_lossy(&output.stderr)));
            Err(anyhow::Error::new(Error::new(
                ErrorKind::Other,
                String::from_utf8_lossy(&output.stderr),
            )))
        }
    }
}
