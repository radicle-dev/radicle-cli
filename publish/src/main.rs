// TODO: Warn if git version is < 2.34
use librad::git::Urn;
use librad::profile::RadHome;

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
    let home = RadHome::default();

    let profile = profile::default()?;
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
    term::info(&format!("Git version {}", git_version));

    if git_version < git::VERSION_REQUIRED {
        anyhow::bail!(
            "a minimum git version of {} is required, please update your installation",
            git::VERSION_REQUIRED
        );
    }

    let peer_id = profile::peer_id(&storage)?;
    let urn = profile::user(&storage)?;
    let monorepo = profile::repo(&home, &profile)?;
    let self_id = Urn::encode_id(&urn);

    term::info("Using config:");
    term::format::seed_config(&seed, &profile, &urn);

    term::info(&format!("Syncing project {:?}", project_id));

    let mut spinner = term::spinner("Pushing delegate id...");
    seed::push_delegate_id(&monorepo, &seed, &self_id, peer_id)?;

    spinner.finish();
    spinner = term::spinner("Pushing project id...");
    seed::push_project_id(&monorepo, &seed, &project_id, peer_id)?;

    spinner.finish();
    spinner = term::spinner("Pushing rad/*, signed refs and heads...");
    seed::push_refs(&monorepo, &seed, &project_id, peer_id)?;

    spinner.finish();
    term::success("Projects published.");
    Ok(())
}
