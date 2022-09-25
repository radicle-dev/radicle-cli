use std::fmt;

use librad::profile::Profile;

use radicle_common::profile;

use crate as term;

/// Identity formatter that takes a profile and displays it as
/// `peer_id` (`username`) depending on the configuration.
pub struct Identity<'a> {
    profile: &'a Profile,
    /// If true, `peer_id` is printed in its compact format e.g. `hynddpk…uf4qwge`
    short: bool,
    /// If true, `peer_id` and `username` are printed using the terminal's
    /// styled formatters.
    styled: bool,
}

impl<'a> Identity<'a> {
    pub fn new(profile: &'a Profile) -> Self {
        Self {
            profile,
            short: false,
            styled: false,
        }
    }

    pub fn short(mut self) -> Self {
        self.short = true;
        self
    }

    pub fn styled(mut self) -> Self {
        self.styled = true;
        self
    }
}

impl<'a> fmt::Display for Identity<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let read_only = profile::read_only(self.profile).map_err(|_| fmt::Error)?;
        let username = profile::name(Some(self.profile)).map_err(|_| fmt::Error)?;
        let username = format!("({})", username);
        let peer_id = match self.short {
            true => radicle_common::fmt::peer(read_only.peer_id()),
            false => read_only.peer_id().to_string(),
        };

        if self.styled {
            write!(
                f,
                "{} {}",
                term::format::highlight(peer_id.to_string()),
                term::format::dim(username)
            )
        } else {
            write!(f, "{} {}", peer_id, username)
        }
    }
}
