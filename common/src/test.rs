use std::process::Command;
use std::{env, error};

use librad::crypto::keystore::crypto;
use librad::crypto::keystore::pinentry::SecUtf8;
use librad::profile::LNK_HOME;

use super::{keys, profile, test};

pub type BoxedError = Box<dyn error::Error>;

pub const USER_PASS: &str = "password";

pub const SSH_AUTH_SOCK: &str = "SSH_AUTH_SOCK";
pub const SSH_AGENT_PID: &str = "SSH_AGENT_PID";

pub const SOCK_FILE: &str = "ssh-agent.sock";

pub mod setup {
    use super::*;

    #[derive(PartialEq)]
    pub enum Env {
        Home,
        SshAgent,
    }

    pub fn with(environment: &[Env]) -> Result<(), BoxedError> {
        if environment.contains(&Env::Home) {
            env::set_var(LNK_HOME, env::current_dir()?.join("lnk_home"));
        }
        if environment.contains(&Env::SshAgent) {
            let (ssh_auth_sock, ssh_agent_id) = ssh_agent::start()?;
            let pid = ssh_agent_id.ok_or("ssh-agent was started, but pid could not be read.")?;
            env::set_var(SSH_AGENT_PID, pid);
            env::set_var(SSH_AUTH_SOCK, ssh_auth_sock);
        }
        Ok(())
    }
}

pub mod teardown {
    use super::*;
    pub fn all() -> Result<(), BoxedError> {
        #[cfg(test)]
        let params = *crypto::KDF_PARAMS_TEST;
        #[cfg(not(test))]
        let params = crypto::KdfParams::recommended();

        if let Ok(profiles) = profile::list() {
            for profile in profiles {
                let pass = crypto::Pwhash::new(SecUtf8::from(test::USER_PASS), params);
                keys::remove(&profile, pass, keys::ssh_auth_sock())?;
            }
        }

        ssh_agent::stop()?;

        Ok(())
    }
}

mod ssh_agent {
    use super::*;
    use std::path::PathBuf;
    /// Spawns a ssh-agent and extracts its pid from stdout using `SSH_AGENT_PID=`.
    pub fn start() -> Result<(PathBuf, Option<String>), BoxedError> {
        let ssh_auth_sock = env::current_dir()?.join(SOCK_FILE);
        let path = &format!("{}", ssh_auth_sock.to_string_lossy());
        let id_prefix = &format!("{}=", SSH_AGENT_PID);

        let output = Command::new("ssh-agent")
            .args(["-a", path])
            .output()
            .expect("Could not start ssh-agent");

        let stdout = String::from_utf8(output.stdout).unwrap_or("".to_owned());
        let lines: Vec<_> = stdout.split(';').collect();
        let configs: Vec<_> = lines.iter().map(|line| line.trim()).collect();

        let pid_configs: Vec<_> = configs
            .iter()
            .filter(|entry| entry.starts_with(id_prefix))
            .collect();

        let pid = pid_configs
            .last()
            .and_then(|p| p.strip_prefix(id_prefix))
            .map(|p| p.to_owned());

        Ok((ssh_auth_sock, pid))
    }

    pub fn stop() -> Result<(), BoxedError> {
        let ssh_auth_sock = env::current_dir()?.join(SOCK_FILE);
        if ssh_auth_sock.exists() {
            let mut kill = Command::new("kill")
                .args(["-9", &env::var(SSH_AGENT_PID)?])
                .spawn()?;
            kill.wait()?;
        }

        Ok(())
    }
}
