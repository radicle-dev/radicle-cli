use rad_common::{keys, person, profile, project};
use rad_show::{Options, HELP};
use rad_terminal::args;
use rad_terminal::components as term;

fn main() {
    args::run_command::<Options, _>(HELP, "Show", run);
}

fn run(mut options: Options) -> anyhow::Result<()> {
    let profile = profile::default()?;
    let sock = keys::ssh_auth_sock();
    let (_, storage) = keys::storage(&profile, sock)?;

    if options == Options::default() {
        options.show_proj_id = true;
        options.show_peer_id = true;
        options.show_self = true;
        options.show_profile_id = true;
        options.show_ssh_key = true;
    }

    if options.show_proj_id {
        let repo = project::repository()?;
        let remote = project::remote(&repo)?;
        let urn = remote.url.urn;

        term::info!("project: {}", term::format::highlight(urn));
    }
    if options.show_peer_id {
        term::info!("peer: {}", term::format::highlight(storage.peer_id()));
    }
    if options.show_self {
        let id = person::local(&storage)?;
        term::info!("self: {}", term::format::highlight(id.urn()));
    }
    if options.show_profile_id {
        term::info!("profile: {}", term::format::highlight(profile.id()));
    }
    if options.show_ssh_key {
        let peer_id = storage.peer_id();
        let ssh = keys::to_ssh_fingerprint(peer_id)?;
        term::info!("ssh: {}", term::format::highlight(ssh));
    }
    Ok(())
}
