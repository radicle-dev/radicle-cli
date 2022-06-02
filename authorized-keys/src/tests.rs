use std::convert::TryFrom;
use std::path::PathBuf;

use crate::{error::Error, rad_keys::RadKeys, KeyType, RadKeyring};

#[radicle_common::tokio::test]
async fn test_keyring() -> Result<(), Error> {
    let rk = RadKeys::try_from(PathBuf::from("./test/keys"))?;

    println!("Key Dir: {:?}", rk.keys_dir);

    let keys = rk.keyring(KeyType::OpenPgp).await?;

    println!("Found Keys: {:?}", keys);

    Ok(())
}
