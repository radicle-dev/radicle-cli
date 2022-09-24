use librad::profile::Profile;

use radicle_common::fmt;
use radicle_common::profile;

pub fn print(profile: &Profile) -> String {
    match profile::read_only(profile) {
        Ok(storage) => {
            let username = profile::name(Some(profile)).unwrap_or_default();
            format!(
                "{} {}",
                fmt::peer(storage.peer_id()),
                format!("({})", username)
            )
        }
        Err(_) => String::new(),
    }
}
