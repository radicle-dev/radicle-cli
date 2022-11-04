//! Person-related functionality.
use anyhow::{Context, Error, Result};
/*
use librad::{canonical::Cstring, git::identities::local::LocalIdentity};

use librad::git::identities::Person;
use librad::git::storage::Storage;

use librad::crypto::BoxedSigner;
use librad::identities::payload;
use librad::identities::payload::HasNamespace;
pub use librad::git::identities::person::verify;
*/
use radicle::Profile;

lazy_static::lazy_static! {
    static ref ENS_NAMESPACE: url::Url = "https://radicle.xyz/ethereum/ens/v1"
        .parse()
        .expect("static URL malformed");
}

/// ENS payload.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct Ens {
    pub name: String,
}
/*
impl HasNamespace for Ens {
    fn namespace() -> &'static url::Url {
        &ENS_NAMESPACE
    }
}
*/

/*
// TODO(dave)
/// Set an ENS payload for the local identity.
/// Returns the updated person.
pub fn set_ens_payload(ens: Ens, storage: &Storage) -> Result<Person> {
    let id = local::default(storage)?;
    let payload = id.payload();
    let mut exts = payload
        .exts()
        .map(|(namespace, val)| (namespace.clone(), val.clone()))
        .map(|(namespace, val)| payload::Ext { namespace, val })
        .collect::<Vec<_>>();

    let namespace = Ens::namespace().clone();
    let val = serde_json::to_value(ens)?;
    let delegations = id.delegations().iter().cloned();

    exts.push(payload::Ext { namespace, val });

    let new = person::update(
        storage,
        &id.urn(),
        Some(id.urn()),
        None,
        exts,
        Some(delegations),
    )?;

    Ok(new)
}
*/
