use core::{result, str::FromStr};
use std::io;

use ethers::prelude::*;
use ethers::utils::to_checksum;

#[derive(thiserror::Error, Debug)]
pub enum Error<E: std::fmt::Debug = ()> {
    #[error("http request failed: {0}")]
    Ureq(ureq::Error),
    #[error("i/o error: {0}")]
    Io(#[from] io::Error),
    #[error("signature error: {0}")]
    SignatureError(#[from] SignatureError),
    #[error("API error ({0}): {1}")]
    RemoteError(u16, String),
    #[error("invalid data received from API")]
    InvalidData,
    #[error("signature error: {0:?}")]
    Signature(E),
}

impl Error {
    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::RemoteError(status, _) if *status == 404)
    }
}

impl From<ureq::Error> for Error {
    fn from(e: ureq::Error) -> Self {
        match e {
            ureq::Error::Status(status, r) => {
                Error::RemoteError(status, r.into_string().unwrap_or_default())
            }
            e => Error::Ureq(e),
        }
    }
}

pub type Result<T = (), E = Error> = result::Result<T, E>;

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum Operation {
    Call,
    DelegateCall,
}

pub struct Client<'a> {
    agent: ureq::Agent,
    transactions_api: &'a str,
}

pub struct Safe<'a> {
    client: &'a Client<'a>,
    safe_address: Address,

    pub owners: Vec<Address>,
    pub threshold: u64,
    pub nonce: U256,
}

#[derive(Debug)]
// All fields are public, so you may construct it directly or update the nonce,
// prices or refund address as desired.
pub struct SafeTx {
    pub safe_address: Address,
    pub to: Address,
    pub value: U256,
    pub data: Bytes,
    pub operation: Operation,
    pub nonce: U256,
}

#[derive(Debug)]
pub struct SignedSafeTx {
    inner: SafeTx,
    sender: Address,
    safe_tx_hash: TxHash,
    signature: Signature,
}

impl Client<'_> {
    pub fn new(transactions_api: &str) -> Client {
        Client {
            agent: ureq::Agent::new(),
            transactions_api,
        }
    }

    pub fn get_safe(&self, safe_address: Address) -> Result<Safe, Error> {
        #[derive(serde::Deserialize)]
        struct SafeResponse {
            nonce: u64,
            threshold: u64,
            owners: Vec<String>,
        }

        let SafeResponse {
            nonce,
            threshold,
            owners,
        } = self
            .agent
            .get(&format!(
                "{}/v1/safes/{}/",
                self.transactions_api,
                to_checksum(&safe_address, None),
            ))
            .call()?
            .into_json()?;

        let owners = owners
            .iter()
            .map(|o| Address::from_str(o))
            .collect::<Result<_, _>>()
            .map_err(|_| Error::InvalidData)?;
        let nonce = U256::from(nonce);

        Ok(Safe {
            client: self,
            safe_address,
            nonce,
            threshold,
            owners,
        })
    }
}

impl Safe<'_> {
    pub fn create_transaction(
        &self,
        to: Address,
        value: U256,
        data: Bytes,
        operation: Operation,
    ) -> SafeTx {
        SafeTx {
            safe_address: self.safe_address,
            to,
            value,
            data,
            nonce: self.nonce,
            operation,
        }
    }

    pub fn propose(&self, signed_safe_tx: SignedSafeTx) -> Result {
        let SafeTx {
            to,
            value,
            data,
            nonce,
            operation,
            ..
        } = signed_safe_tx.inner;

        self.client
            .agent
            .post(&format!(
                "{}/v1/safes/{}/multisig-transactions/",
                self.client.transactions_api,
                to_checksum(&self.safe_address, None),
            ))
            .send_json(ureq::json!({
                "to": to_checksum(&to, None),
                "value": value.to_string(),
                "data": data,
                "operation": operation as u8,
                "gasToken": None::<()>,
                "safeTxGas": "0",
                "baseGas": "0",
                "gasPrice": "0",
                "refundReceiver": Address::zero(),
                "nonce": nonce.to_string(),
                "contractTransactionHash": signed_safe_tx.safe_tx_hash,
                "sender": to_checksum(&signed_safe_tx.sender, None),
                "signature": format!("0x{}", signed_safe_tx.signature),
                "origin": "An unauthenticated origin string?",
            }))
            // 200 OK, is not okay. We expect 201 Created.
            .and_then(|r| match r.status() {
                201 => Ok(()),
                s => Err(ureq::Error::Status(s, r)),
            })
            .map_err(From::from)
    }

    pub fn confirm(&self, signed_safe_tx: SignedSafeTxHash) -> Result {
        self.client
            .agent
            .post(&format!(
                "{}/v1/multisig-transactions/{:?}/confirmations/",
                self.client.transactions_api, signed_safe_tx.safe_tx_hash,
            ))
            .send_json(ureq::json!({
                "signature": format!("0x{}", signed_safe_tx.signature),
            }))
            // 200 OK, is not okay. We expect 201 Created.
            .and_then(|r| match r.status() {
                201 => Ok(()),
                s => Err(ureq::Error::Status(s, r)),
            })
            .map_err(From::from)
    }
}

impl SafeTx {
    // We consume self. If signing fails, we have bigger issues than recreating the transaction.
    pub async fn sign<S>(self, signer: &S) -> Result<SignedSafeTx, S::Error>
    where
        S: Signer,
    {
        use ethers::abi::Tokenizable;
        use ethers::utils::keccak256;
        use tiny_keccak::{Hasher, Keccak};
        macro_rules! tokenize {
            ($($tok:expr),* $(,)*) => {
                &[$($tok.into_token()),*][..]
            }
        }

        let safe_tx_hash = TxHash::from({
            let mut hasher = Keccak::v256();
            let mut output = [0u8; 32];
            hasher.update(&[0x19, 0x01]);
            // domain separator
            hasher.update(&keccak256(&ethers::abi::encode(tokenize![
                // DOMAIN_SEPARATOR_TYPEHASH
                U256::from_str(
                    "0x47e79534a245952e8b16893a336b85a3d9ea9fa8c573f3d803afb92a79469218"
                )
                .unwrap(),
                U256::from(signer.chain_id()),
                self.safe_address,
            ])));
            // safeTxHash
            hasher.update(&keccak256(&ethers::abi::encode(tokenize![
                // SAFE_TX_TYPEHASH
                U256::from_str(
                    "0xbb8310d486368db6bd6f849402fdd73ad53d316b5a4b2644ad6efe0f941286d8"
                )
                .unwrap(),
                self.to,
                self.value,
                keccak256(&self.data),
                self.operation as u8,
                U256::zero(),    // safe_tx_gas
                U256::zero(),    // base_gas
                U256::zero(),    // gas_price
                Address::zero(), // gas_token
                Address::zero(), // refund_receiver
                self.nonce,
            ])));
            hasher.finalize(&mut output);
            output
        });

        let SignedSafeTxHash { signature, .. } = sign_tx_hash(signer, safe_tx_hash).await?;

        Ok(SignedSafeTx {
            inner: self,
            sender: signer.address(),
            safe_tx_hash,
            signature,
        })
    }
}

pub struct SignedSafeTxHash {
    safe_tx_hash: TxHash,
    signature: Signature,
}

pub async fn sign_tx_hash<S>(signer: &S, safe_tx_hash: TxHash) -> Result<SignedSafeTxHash, S::Error>
where
    S: Signer,
{
    let mut signature = signer.sign_message(safe_tx_hash).await?;

    // signature.recover expects signature.v ∈ {27, 28}
    if signature.v < 2 {
        signature.v += 27;
    }
    assert!(
        [27, 28].contains(&signature.v),
        "BUG: `Signer::sign_message` produced an invalid `v` field."
    );
    assert_eq!(
        Some(signer.address()),
        signature.recover(&safe_tx_hash[..]).ok(),
        "BUG: `Signature::recover` failed to recover our signing \
            address... did `Signer::sign_message` fail to add the prefix?"
    );
    signature.v += 4;

    Ok(SignedSafeTxHash {
        safe_tx_hash,
        signature,
    })
}

#[test]
#[ignore]
fn check() {
    futures_executor::block_on(async {
        use rand_chacha::rand_core::SeedableRng;

        // Super secure RNG for tests
        let mut fake_rng =
            rand_chacha::ChaCha20Rng::from_seed(*b"0123456789abcdef0123456789abcdef");
        let wallet = LocalWallet::new(&mut fake_rng).with_chain_id(4u64);

        // ... Propose a new transaction ...

        let value = U256::from(0u64);
        let data = match "anchor" {
            "unanchor" => "cbd566010000000000000000000000000000000000000000000000000000000000000000",
            "anchor" => "68f1fbf80000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000000",
            _ => todo!(),
        };

        let client = Client::new("https://safe-transaction.rinkeby.gnosis.io/api");
        let safe = client
            .get_safe(
                Address::from_str("0xb535CEd5f003e00CfF2424892D4885b139019F1d")
                    .expect("Invalid rinkeby address"),
            )
            .expect("Safe exists");
        let org_rinkeby = Address::from_str("0xaFb752f961CEF7FdfB9d2925120D23Aa9B4ed7Ae")
            .expect("Invalid org address");
        let safe_tx = safe.create_transaction(
            org_rinkeby,
            value,
            Bytes::from(hex::decode(data).expect("Invalid data")),
            Operation::Call,
        );

        // Optionally update any of safe_tx' fields. The nonce is dynamically
        // set respective to the safe state.
        let signed_safe_tx = safe_tx.sign(&wallet).await.expect("Invalid signature");
        safe.propose(signed_safe_tx)
            .expect("Failed to propose transaction");

        // ... Add confirmation to existing transaction ...

        let safe_tx_hash =
            TxHash::from_str("0xec812245f4c8908551914922bb1b093b3aa8c02e284c5e8c54fb8170673716cd")
                .expect("Invalid TxHash");

        match safe.confirm(
            sign_tx_hash(&wallet, safe_tx_hash)
                .await
                .expect("Invalid signature"),
        ) {
            Ok(_) => (),
            // Should this be considered `Ok` by `Safe::confirm`?
            Err(Error::RemoteError(400, body)) if body.contains("was already executed") => (),
            Err(e) => panic!("Failed to confirm transaction: {:?}", e),
        }
    });
}
