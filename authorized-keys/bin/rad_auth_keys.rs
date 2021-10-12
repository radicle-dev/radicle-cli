use radicle_authorized_keys::{error::Error, rad_keys::RadKeys, KeyRingSource, Options};
use structopt::StructOpt;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let options = Options::from_args();

    // Default to `RadKeys` keyring source if `--source, -s` is not provided;
    let source = options.source.clone().unwrap_or(KeyRingSource::RadKeys);

    match source {
        KeyRingSource::RadKeys => {
            RadKeys::apply_options(options)
                .await
                .expect("failed to apply changes to authorized keys");
        }
        _ => return Err(Error::UnsupportedKeyRingSource),
    }

    Ok(())
}
