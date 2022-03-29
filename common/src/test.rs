use std::{env, error};

use librad::crypto::keystore::pinentry::SecUtf8;
use librad::profile::LNK_HOME;

use super::{keys, profile, test};
use rad_terminal::components as term;

pub type BoxedError = Box<dyn error::Error>;

pub const USER_PASS: &str = "password";

pub mod setup {
    use super::*;
    pub fn lnk_home() -> Result<(), BoxedError> {
        env::set_var(LNK_HOME, env::current_dir()?.join("lnk_home"));
        Ok(())
    }
}

pub mod teardown {
    use super::*;
    pub fn profiles() -> Result<(), BoxedError> {
        for profile in profile::list()? {
            let pass = term::pwhash(SecUtf8::from(test::USER_PASS));
            keys::remove(&profile, pass, keys::ssh_auth_sock())?;
        }
        Ok(())
    }
}
