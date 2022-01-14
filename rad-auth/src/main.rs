use rad_clib::keys::ssh::SshAuthSock;
use rad_profile;
use libcli::{keys, person, profile, tui};

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

    tui::headline("Initializing your 🌱 profile and identity");

    let profiles = rad_profile::list(None)?;
    if profiles.len() > 0 && !args.add {
        tui::warning("Found existing profile(s):");
        let profile = profile::default()?;
        tui::format::profile_list(&profiles, &profile);
        tui::info("If you want to create a new profile, please use --add.");
    } else {
        let username = tui::text_input("Username", None);
        let pass = tui::pwhash(tui::secret_input());

        let mut spinner = tui::spinner("Creating your profile...");
        let (profile, _) = rad_profile::create(None, pass.clone())?;
        spinner.finish();
        spinner = tui::spinner("Adding your key to ssh-agent...");

        let _id = keys::add(&profile, pass, sock.clone())?;
        let storage = keys::storage(&profile, sock)?;

        spinner.finish();
        spinner = tui::spinner("Creating identity...");

        let person = person::create(&profile, &username)?;
        person::set_local(&storage, &person);
        spinner.finish();
        tui::success("Profile and identity created.");
    }
    Ok(())
}


