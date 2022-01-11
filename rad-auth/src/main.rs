// Copyright © 2021 The Radicle Link Contributors
//
// This file is part of radicle-link, distributed under the GPLv3 with Radicle
// Linking Exception. For full terms see the included LICENSE file.

// extern crate rad_cli;
use radicle_tools::cli;

use structopt::StructOpt;

use rad_clib::keys::ssh::SshAuthSock;

use cli::{keys, person, proc::some_or_exit, profile, tui};

#[derive(Debug, StructOpt)]
pub struct Args {
    #[structopt(short, long)]
    pub add: bool,
}

fn main() -> anyhow::Result<()> {
    let Args { add } = Args::from_args();
    let sock = SshAuthSock::default();

    tui::headline("Initializing your 🌱 profile and identity");

    let profiles = rad_profile::list(None)?;
    if profiles.len() > 0 && !add {
        tui::warning("Found existing profile(s):");
        let profile = some_or_exit(profile::default());
        tui::format::profile_list(&profiles, &profile);
        tui::info("If you want to create a new profile, please use --add.");
    } else {
        let username = tui::text_input("Username", None); 
        let pass = tui::pwhash(tui::secret_input());

        let mut spinner = tui::spinner("Creating your profile...");
        let (profile, _) = rad_profile::create(None, pass.clone())?;
        
        spinner.finish();
        spinner = tui::spinner("Adding your key to ssh-agent...");
        match keys::add(&profile, pass, sock.clone()) {
            Some(_) => {
                let storage = some_or_exit(keys::storage(&profile, sock));

                spinner.finish();
                spinner = tui::spinner("Creating identity...");
                match person::create(&profile, &username) {
                    Some(person) => {
                        person::set_local(&storage, &person);
                        spinner.finish();
                        tui::success("Profile and identity created.");
                    }
                    None => {}
                }
            }
            None => {}
        }
    }
    println!();
    Ok(())
}
