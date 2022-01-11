// Copyright © 2021 The Radicle Link Contributors
//
// This file is part of radicle-link, distributed under the GPLv3 with Radicle
// Linking Exception. For full terms see the included LICENSE file.

// extern crate rad_cli;
use radicle_tools::cli;

use std::thread::sleep;
use std::time::Duration;

use librad::{
    canonical::Cstring,
    identities::payload::{self},
};
use rad_clib::{keys::ssh::SshAuthSock, storage::ssh};

use cli::{proc::some_or_exit, profile, project, tui};

fn main() -> anyhow::Result<()> {
    tui::headline("Initializing local 🌱 project");
    
    some_or_exit(project::current());

    let profile = some_or_exit(profile::default());
    let (signer, storage) = ssh::storage(&profile, SshAuthSock::default())?;

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

    some_or_exit(project::create(&storage, signer, &profile, payload));
    spinner.finish();

    tui::success("Project initialized.");
    Ok(())
}
