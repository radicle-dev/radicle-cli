use radicle_tools::cli;

use std::thread::sleep;
use std::time::Duration;

use librad::{
    canonical::Cstring,
    identities::payload::{self},
};
use rad_clib::{keys::ssh::SshAuthSock};

use cli::{keys, profile, project, tui};

fn main() -> anyhow::Result<()> {
    match run() {
        Ok(()) => Ok(()),
        Err(_) => std::process::exit(0),
    }
}

fn run() -> anyhow::Result<()> {
    tui::headline("Initializing local 🌱 project");
    
    let _repo = project::repository()?;
    let profile = profile::default()?;
    let storage = keys::storage(&profile, SshAuthSock::default())?;
    let signer = keys::signer(&profile, SshAuthSock::default())?;

    let name = tui::text_input("Name", None);
    let description = tui::text_input("Description", Some("".to_string()));
    let branch = tui::text_input("Default branch", Some("master".to_string()));

    let spinner = tui::spinner("Creating project...");
    sleep(Duration::from_secs(3));

    let payload = payload::Project {
        name: Cstring::from(name),
        description: Some(Cstring::from(description)),
        default_branch: Some(Cstring::from(branch)),
    };

    let _profile = project::create(&storage, signer, &profile, payload)?;
    spinner.finish();

    tui::success("Project initialized.");
    Ok(())
}
