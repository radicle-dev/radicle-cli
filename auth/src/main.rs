use rad_clib::keys::ssh::SshAuthSock;

use rad_common::{keys, person, profile};
use rad_terminal::compoments as term;

mod args;

fn main() -> anyhow::Result<()> {
    match run() {
        Ok(()) => Ok(()),
        Err(_) => {
            println!();
            std::process::exit(1);
        }
    }
}

fn run() -> anyhow::Result<()> {
    let args = args::parse()?;

    let sock = SshAuthSock::default();

    term::headline("Initializing your 🌱 profile and identity");

    let profiles = rad_profile::list(None)?;
    if !profiles.is_empty() && !args.add {
        term::warning("Found existing profile(s):");
        let profile = profile::default()?;
        term::format::profile_list(&profiles, &profile);
        term::info("If you want to create a new profile, please use --add.");
    } else {
        let username = term::text_input("Username", None);
        let pass = term::pwhash(term::secret_input());

        let mut spinner = term::spinner("Creating your profile...");
        let (profile, _) = rad_profile::create(None, pass.clone())?;
        spinner.finish();
        spinner = term::spinner("Adding your key to ssh-agent...");

        let _id = keys::add(&profile, pass, sock.clone())?;
        let storage = keys::storage(&profile, sock)?;

        spinner.finish();
        spinner = term::spinner("Creating identity...");

        let person = person::create(&profile, &username)?;
        person::set_local(&storage, &person);
        spinner.finish();
        term::success("Profile and identity created.");
    }
    Ok(())
}
