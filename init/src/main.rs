use std::thread::sleep;
use std::time::Duration;

use librad::canonical::Cstring;
use librad::identities::payload::{self};

use rad_clib::keys::ssh::SshAuthSock;

use rad_common::{keys, profile, project};
use rad_terminal::compoments as term;

fn main() -> anyhow::Result<()> {
    match run() {
        Ok(()) => Ok(()),
        Err(_) => std::process::exit(1),
    }
}

fn run() -> anyhow::Result<()> {
    term::headline("Initializing local 🌱 project");

    let _repo = project::repository()?;
    let profile = profile::default()?;
    let storage = keys::storage(&profile, SshAuthSock::default())?;
    let signer = keys::signer(&profile, SshAuthSock::default())?;

    let name = term::text_input("Name", None);
    let description = term::text_input("Description", Some("".to_string()));
    let branch = term::text_input("Default branch", Some("master".to_string()));

    let spinner = term::spinner("Creating project...");
    sleep(Duration::from_secs(3));

    let payload = payload::Project {
        name: Cstring::from(name),
        description: Some(Cstring::from(description)),
        default_branch: Some(Cstring::from(branch)),
    };

    let _profile = project::create(&storage, signer, &profile, payload)?;
    spinner.finish();

    term::success("Project initialized.");
    Ok(())
}
