use std::process::Command;
use std::{env, error};

use anyhow::Result;
use assay::assay;

use librad::crypto::keystore::pinentry::SecUtf8;
use librad::profile::LNK_HOME;

use rad_common::{keys, profile};
use rad_terminal::components as term;

const PASSWORD: &str = "password";

type BoxedError = Box<dyn error::Error>;

fn setup() -> Result<(), BoxedError> {
    env::set_var(LNK_HOME, env::current_dir()?.join("lnk_home"));
    Ok(())
}

fn teardown() -> Result<(), BoxedError> {
    for profile in profile::list()? {
        let pass = term::pwhash(SecUtf8::from(PASSWORD));
        keys::remove(&profile, pass, keys::ssh_auth_sock())?;
    }
    Ok(())
}

mod auth {
    use super::*;

    const USERNAME_MISSING: &str = "missing argument for option '--username'";
    const PASSWORD_MISSING: &str = "missing argument for option '--password'";
    const INIT_MISSING: &str = "invalid option '--password'";

    #[assay(
        setup = setup()?,
        teardown = teardown()?,
      )]
    fn can_be_initialized() {
        let status = Command::new("rad-auth")
            .args(["--init", "--username", "user1", "--password", PASSWORD])
            .status();
        assert!(status?.success());
    }

    #[assay]
    fn username_missing() {
        let output = Command::new("rad-auth")
            .args(["--init", "--username"])
            .output()?;
        let result = String::from_utf8_lossy(&output.stderr);

        assert!(result.contains(USERNAME_MISSING), "{}", result);
    }

    #[assay]
    fn password_missing() {
        let output = Command::new("rad-auth")
            .args(["--init", "--password"])
            .output()?;
        let result = String::from_utf8_lossy(&output.stderr);

        assert!(result.contains(PASSWORD_MISSING), "{}", result);
    }

    #[assay]
    fn init_missing() {
        let output = Command::new("rad-auth").args(["--password"]).output()?;
        let result = String::from_utf8_lossy(&output.stderr);

        assert!(result.contains(INIT_MISSING), "{}", result);
    }
}
