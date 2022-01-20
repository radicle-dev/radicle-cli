use librad::git::Urn;
use librad::profile::Profile;

use rad_clib::keys::ssh::SshAuthSock;
use rad_common::{git, keys, profile, project, seed};
use rad_terminal::compoments as term;

fn main() -> anyhow::Result<()> {
    match run() {
        Ok(()) => Ok(()),
        Err(err) => {
            term::format::error("Publishing failed", &err);
            term::blank();

            std::process::exit(1);
        }
    }
}

fn run() -> anyhow::Result<()> {
    let seed = "http://localhost:8778".to_string();
    let profile = Profile::load()?;
    let (_, storage) = keys::storage(&profile, SshAuthSock::default())?;

    term::info("Reading local git config...");

    let repo = project::repository()?;
    let remote = project::remote(&repo)?;
    let project_id = Urn::encode_id(&remote.url.urn);
    let git_version = git::version()?;

    term::info(&format!(
        "Publishing 🌱 project {}",
        term::format::highlight(&remote.url.urn.to_string())
    ));
    term::info(&format!("Publishing to {}", term::format::highlight(&seed)));
    term::info(&format!("Git version {}", git_version));

    if git_version < git::VERSION_REQUIRED {
        anyhow::bail!(
            "a minimum git version of {} is required, please update your installation",
            git::VERSION_REQUIRED
        );
    }

    let peer_id = profile::peer_id(&storage)?;
    let urn = profile::user(&storage)?;
    let monorepo = profile.paths().git_dir();
    let self_id = Urn::encode_id(&urn);

    let mut spinner = term::spinner(&format!("Syncing delegate identity {}...", &self_id));
    match seed::push_delegate_id(monorepo, &seed, &self_id, peer_id) {
        Ok(_) => spinner.finish(),
        Err(err) => {
            spinner.failed();
            return Err(err);
        }
    }

    spinner = term::spinner("Syncing project id...");
    match seed::push_project_id(monorepo, &seed, &project_id, peer_id) {
        Ok(_) => spinner.finish(),
        Err(err) => {
            spinner.failed();
            return Err(err);
        }
    }

    spinner = term::spinner("Syncing rad/*, signed refs and heads...");
    match seed::push_refs(monorepo, &seed, &project_id, peer_id) {
        Ok(_) => spinner.finish(),
        Err(err) => {
            spinner.failed();
            return Err(err);
        }
    }
    term::success("Project published.");

    Ok(())
}
