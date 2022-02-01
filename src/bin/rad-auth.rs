use anyhow::Context as _;

use rad_auth::{Options, HELP};
use rad_common::{git, keys, person, profile};
use rad_terminal::components as term;

fn main() {
    term::run_command::<Options, _>(HELP, "Authentication", run);
}

fn run(options: Options) -> anyhow::Result<()> {
    let sock = keys::ssh_auth_sock();

    let profiles = match rad_profile::list(None) {
        Ok(profiles) if !options.init => Some(profiles),
        _ => None,
    };

    if let Some(profiles) = profiles {
        let profile = profile::default()?;

        term::info!(
            "Your active profile is {}",
            term::format::highlight(&profile.id().to_string()),
        );

        let selection = if profiles.len() > 1 {
            if let Some(p) = term::format::profile_select(&profiles, &profile) {
                p
            } else {
                return Ok(());
            }
        } else {
            &profile
        };

        if !keys::is_ready(selection, sock.clone())? {
            term::warning("Adding your radicle key to ssh-agent");

            // TODO: We should show the spinner on the passphrase prompt,
            // otherwise it seems like the passphrase is valid even if it isn't.
            let pass = term::pwhash(term::secret_input());
            let spinner = term::spinner("Unlocking...");

            keys::add(selection, pass, sock.clone()).context("invalid passphrase supplied")?;
            spinner.finish();

            term::success!("Radicle key added to ssh-agent");
        } else {
            term::success!("Signing key already in ssh-agent");
        }

        if selection.id() != profile.id() {
            let id = selection.id();
            profile::set(id)?;

            term::success!("Profile {} activated", id);
        }
        let (signer, _) = keys::storage(&profile, sock)?;

        git::configure_monorepo(profile.paths().git_dir(), &signer.peer_id())?;
        term::success!("Signing key configured in git");
    } else {
        term::headline("Initializing your 🌱 profile and identity");

        let username: String = term::text_input("Username", None)?;
        let pass = term::pwhash(term::secret_input_with_confirmation());

        let mut spinner = term::spinner("Creating your 🌱 Ed25519 keypair...");
        let (profile, peer_id) = rad_profile::create(None, pass.clone())?;
        let monorepo = profile.paths().git_dir();

        git::configure_monorepo(monorepo, &peer_id)?;

        spinner.finish();
        spinner = term::spinner("Adding to ssh-agent...");

        let profile_id = keys::add(&profile, pass, sock.clone())?;
        let (signer, storage) = keys::storage(&profile, sock)?;

        spinner.finish();

        let person = person::create(&profile, &username, signer, &storage)?;
        person::set_local(&storage, &person);

        term::success!(
            "Profile {} created.",
            term::format::highlight(&profile_id.to_string())
        );

        term::blank();
        term::info!(
            "Your radicle Peer ID is {}. This identifies your device.",
            term::format::highlight(&peer_id.to_string())
        );
        term::info!(
            "Your personal 🌱 URN is {}. This identifies you across devices.",
            term::format::highlight(&person.urn().to_string())
        );

        term::blank();
        term::tip("To create a radicle project, run `rad init` from a git repository.");
    }
    Ok(())
}
