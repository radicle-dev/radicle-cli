use std::path::PathBuf;

use anyhow::Context as _;

use rad_checkout::{Options, HELP};
use rad_common::{identities, keys, profile};
use rad_terminal::components as term;

fn main() {
    term::run_command::<Options, _>(HELP, "Project checkout", run);
}

fn run(options: Options) -> anyhow::Result<()> {
    let profile = profile::default()?;
    let sock = keys::ssh_auth_sock();
    let (signer, storage) = keys::storage(&profile, sock)?;
    let project = identities::project::get(&storage, &options.urn)?
        .context("project could not be found in local storage")?;
    let name = project.subject().name.to_string();
    let path = PathBuf::from(name.clone());

    if path.exists() {
        anyhow::bail!("the local path {:?} already exists", path.as_path());
    }

    term::headline(&format!(
        "Initializing local checkout for 🌱 {} ({})",
        term::format::highlight(&options.urn),
        name,
    ));

    let spinner = term::spinner("Performing checkout...");
    if let Err(err) = identities::project::checkout(
        &storage,
        profile.paths().clone(),
        signer,
        &options.urn,
        None,
        path,
    ) {
        spinner.failed();
        return Err(err.into());
    }
    spinner.finish();

    term::success!(
        "Project checkout successful under ./{}",
        term::format::highlight(name)
    );

    Ok(())
}
