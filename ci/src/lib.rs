use std::convert::TryInto;
use std::ffi::OsString;
use std::fs::File;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};

use anyhow::Context;
use ssh2::Session;
use url::Host;

use rad_terminal::args::{Args, Help};
use rad_terminal::components as term;

pub const HELP: Help = Help {
    name: "ci",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
USAGE
    rad ci (--install | --uninstall) --seed <host> [--ssh-user <user>] [--root <root>] [--docker-compose <yml>] [--verbose]

OPTIONS
    --install           Runs docker images for CI. Default: Concourse
    --uninstall         Terminates running CI images. Default: Concourse
    --seed              Address of the seed node.
    --ssh-user          SSH user on target machine. Default: root
    --root              (Optional) Radicle root on server. Default: /app/radicle/radicle
    --docker-compose    (Optional) docker-compose.yml to deploy. Default: downloaded
    --verbose
"#,
};

const CONCOURSE_DOCKER_COMPOSE_URL: &str = "https://concourse-ci.org/docker-compose.yml";
const YML_NAME: &str = "concourse-docker-compose.yml";

#[derive(Default, Eq, PartialEq)]
pub struct Options {
    pub install: bool,
    pub uninstall: bool,
    pub seed: Option<Host>,
    pub ssh_user: String,
    pub root: PathBuf,
    pub docker_compose: Option<PathBuf>,
    pub verbose: bool,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut install = false;
        let mut uninstall = false;
        let mut seed = None;
        let mut ssh_user = whoami::username();
        let mut root = PathBuf::from("/app/radicle/root");
        let mut docker_compose = None;
        let mut verbose = false;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("seed") => {
                    let value = parser.value()?;
                    let value = value.to_string_lossy();
                    let value = value.as_ref();
                    let addr = Host::parse(value)?;

                    seed = Some(addr);
                }
                Long("install") | Short('i') => {
                    install = true;
                }
                Long("uninstall") | Short('u') => {
                    uninstall = true;
                }
                Long("ssh-user") => {
                    let value = parser.value()?;

                    ssh_user = String::from(value.to_string_lossy());
                }
                Long("root") => {
                    let value = parser.value()?;
                    root = value.try_into()?;
                }
                Long("docker-compose") => {
                    let value = parser.value()?;
                    let path: PathBuf = value.try_into()?;

                    docker_compose = Some(path);
                }
                Long("verbose") | Short('v') => {
                    verbose = true;
                }
                _ => {
                    return Err(anyhow::anyhow!(arg.unexpected()));
                }
            }
        }

        Ok((
            Options {
                seed,
                install,
                uninstall,
                ssh_user,
                root,
                docker_compose,
                verbose,
            },
            vec![],
        ))
    }
}

struct ServerPath {
    root: PathBuf,
    color: fn(String) -> String,
}

impl ServerPath {
    fn etc(&self) -> PathBuf {
        self.root.join("etc")
    }

    fn etc_color(&self) -> String {
        (self.color)(self.etc().to_str().unwrap().to_string())
    }

    fn docker_compose(&self) -> PathBuf {
        self.etc().join(YML_NAME)
    }

    fn docker_compose_color(&self) -> String {
        (self.color)(self.docker_compose().to_str().unwrap().to_string())
    }

    fn hook(&self) -> PathBuf {
        self.root.join("git").join("hooks").join("post-receive-ok")
    }

    fn hook_color(&self) -> String {
        (self.color)(self.hook().to_str().unwrap().to_string())
    }

    fn extract_env(&self) -> PathBuf {
        self.etc().join("extract-env")
    }

    fn extract_env_color(&self) -> String {
        (self.color)(self.extract_env().to_str().unwrap().to_string())
    }

    fn ci_env(&self) -> PathBuf {
        self.etc().join(".ci.env")
    }

    fn ci_env_color(&self) -> String {
        (self.color)(self.ci_env().to_str().unwrap().to_string())
    }
}

pub fn run(options: Options) -> anyhow::Result<()> {
    let seed = options.seed.context("Seed is invalid")?;
    // SSH into server
    let spinner = term::spinner(&format!(
        "SSH into remote {}@{}",
        term::format::bold(&options.ssh_user),
        term::format::dim(&seed)
    ));
    let tcp = TcpStream::connect(format!("{}:22", &seed))?;
    let mut sess = Session::new()?;
    sess.set_tcp_stream(tcp);
    sess.handshake()?;

    let mut agent = sess.agent()?;
    agent.connect()?;
    agent.list_identities()?;
    for identity in agent.identities()? {
        if agent.userauth(&options.ssh_user, &identity).is_ok() && sess.authenticated() {
            break;
        }
    }
    if !sess.authenticated() {
        anyhow::bail!("Couldn't authenticate against server, add your key to ssh-agent.");
    }
    spinner.finish();

    let verbosity = options.verbose;
    let execute_fn = |cmd, spinner| execute_cmd_with_spinner(&sess, cmd, spinner, verbosity);

    // Check requirements
    term::blank();
    term::info!("Checking requirements:");
    term::blank();

    let mut requirements = if options.install {
        vec![
            "wget",
            "sh",
            "cat",
            "grep",
            "cut",
            "mkdir",
            "docker",
            "docker-compose",
        ]
    } else {
        vec!["docker", "docker-compose"]
    };
    if options.install && options.docker_compose.is_some() {
        requirements.remove(0);
    }

    if requirements
        .iter()
        .map(|req| {
            let spinner = term::spinner(&format!(
                "Checking if {} exists",
                term::format::highlight(req)
            ));
            let (_, status) = execute_fn(format!("which {}", req), spinner).unwrap();
            status
        })
        .sum::<i32>()
        > 0
    {
        term::info!("Requirements are not installed.");
        std::process::exit(1);
    }
    term::blank();

    let server_path = ServerPath {
        root: options.root,
        color: term::format::tertiary,
    };

    // Apply changes
    if options.install {
        // Make sure directories exist
        let spinner = term::spinner(&format!(
            "Making sure directory {} exists",
            server_path.etc_color()
        ));
        let (_, status) = execute_fn(format!("mkdir -p {:?}", server_path.etc()), spinner)?;
        if status != 0 {
            std::process::exit(1);
        }
        term::blank();

        // Upload custom docker-compose.yml file when supplied
        if options.docker_compose.is_some() {
            let spinner = term::spinner(&format!(
                "Copying {} to {}",
                term::format::highlight("docker-compose.yml"),
                server_path.docker_compose_color()
            ));
            let path = options.docker_compose.unwrap();
            let mut file = File::open(path).context("Couldn't open --docker-compose file")?;
            let mut content = String::new();
            file.read_to_string(&mut content)
                .context("Couldn't read the contents of --docker-compose file")?;

            upload_content_to_file(
                &sess,
                content.as_bytes(),
                server_path.docker_compose().to_str().unwrap(),
                0o644,
            )?;
            spinner.finish();
        }

        // Copy post-receive-ok file over
        let spinner = term::spinner(&format!(
            "Copying {} hook to {}",
            term::format::highlight("post-receive-ok"),
            server_path.hook_color()
        ));
        let receive_hook_content = include_str!("../post-receive-ok");
        upload_content_to_file(
            &sess,
            receive_hook_content.as_bytes(),
            server_path
                .hook()
                .to_str()
                .context("Couldn't get server hook path as str")?,
            0o755,
        )?;
        spinner.finish();

        // Download the docker-compose.yml
        let spinner = term::spinner(&format!(
            "Downloading {} to {}",
            term::format::highlight("docker-compose.yml"),
            server_path.docker_compose_color()
        ));
        execute_fn(
            format!(
                "wget -nc -O {:?} {}",
                server_path.docker_compose(),
                CONCOURSE_DOCKER_COMPOSE_URL
            ),
            spinner,
        )?;

        // Create .ci.env
        let spinner = term::spinner(&format!(
            "Copying {} to {}",
            term::format::highlight("extract-env"),
            server_path.extract_env_color()
        ));
        let extract_env_content = include_str!("../extract-env");
        upload_content_to_file(
            &sess,
            extract_env_content.as_bytes(),
            server_path
                .extract_env()
                .to_str()
                .context("Couldn't get extract-env path as str")?,
            0o755,
        )?;
        spinner.finish();

        let spinner = term::spinner(&format!(
            "Creating {} in {}",
            term::format::highlight(".ci.env"),
            server_path.ci_env_color()
        ));
        let (_, status) = execute_fn(
            format!(
                "{:?} {:?} {:?} && cat {:?}",
                server_path
                    .extract_env()
                    .to_str()
                    .context("Couldn't get extract-env path as str")?,
                server_path.docker_compose(),
                server_path.ci_env(),
                server_path.ci_env()
            ),
            spinner,
        )?;
        if status != 0 {
            std::process::exit(1);
        }

        // Run `docker-compose up -d`
        term::blank();
        let spinner = term::spinner("Installing CI...");
        let (_, status) = execute_fn(
            format!("docker-compose -f {:?} up -d", server_path.docker_compose()),
            spinner,
        )?;
        if status == 0 {
            term::success!("CI installed!");
        }
    } else if options.uninstall {
        // Run `docker-compose down`
        let spinner = term::spinner("Uninstalling CI...");
        let (_, status) = execute_fn(
            format!("docker-compose -f {:?} down", server_path.docker_compose()),
            spinner,
        )?;
        if status == 0 {
            term::success!("CI uninstalled!");
        }

        // Delete docker-compose.yml file
        let mut channel = sess.channel_session().unwrap();
        execute_cmd(
            &mut channel,
            &format!("rm {:?}", server_path.docker_compose()),
        )?;

        // Delete post-receive-ok hook
        let mut channel = sess.channel_session().unwrap();
        execute_cmd(&mut channel, &format!("rm {:?}", server_path.hook()))?;
    }

    Ok(())
}

fn execute_cmd(channel: &mut ssh2::Channel, cmd: &str) -> anyhow::Result<(String, i32)> {
    let mut res = String::new();
    channel.exec(cmd)?;
    channel.read_to_string(&mut res)?;
    channel.stderr().read_to_string(&mut res)?;
    channel.wait_close()?;
    Ok((res, channel.exit_status()?))
}

fn execute_cmd_with_spinner(
    sess: &ssh2::Session,
    cmd: String,
    spinner: term::Spinner,
    verbose: bool,
) -> anyhow::Result<(String, i32)> {
    let mut channel = sess.channel_session().unwrap();
    let (res, status) = execute_cmd(&mut channel, &cmd)?;
    if status == 0 {
        spinner.finish();
    } else {
        spinner.failed();
    }
    if verbose {
        term::blob(&res);
    }
    Ok((res, status))
}

fn upload_content_to_file(
    sess: &ssh2::Session,
    content: &[u8],
    file_path: &str,
    permissions: i32,
) -> anyhow::Result<()> {
    let mut remote_file = sess.scp_send(
        Path::new(file_path),
        permissions,
        content.len().try_into().unwrap(),
        None,
    )?;

    remote_file.write_all(content)?;
    remote_file.send_eof()?;
    remote_file.wait_eof()?;
    remote_file.close()?;
    remote_file.wait_close()?;

    Ok(())
}
