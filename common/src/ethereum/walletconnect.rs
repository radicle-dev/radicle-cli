use std::error::Error;
use walletconnect::client::{CallError, SessionError};
use walletconnect::{qr, Client, Metadata, Transaction};

use ethers::types::transaction::eip2718::TypedTransaction;
use ethers::types::{Address, NameOrAddress, Signature, U256};

#[derive(Debug)]
pub struct WalletConnect {
    client: Client,
    chain_id: u64,
    address: Address,
}

impl WalletConnect {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let client = Client::new(
            "radicle-cli",
            Metadata {
                description: "Interact with Radicle".into(),
                url: "https://radicle.network".parse()?,
                icons: vec!["https://app.radicle.network/logo.png".parse()?],
                name: "Radicle CLI".into(),
            },
        )?;

        Ok(WalletConnect {
            client,
            chain_id: 0,
            address: Address::zero(),
        })
    }

    pub async fn show_qr(mut self) -> Result<Self, SessionError> {
        let (accounts, chain_id) = self
            .client
            .ensure_session(|uri| {
                println!("{}", uri.as_url());
                qr::print(uri);
            })
            .await?;

        self.chain_id = chain_id;
        self.address.assign_from_slice(accounts[0].as_bytes());

        Ok(self)
    }

    pub fn chain_id(&self) -> u64 {
        self.chain_id
    }

    pub fn address(&self) -> Address {
        self.address
    }

    fn address_string(&self) -> String {
        format!("{}", self.address)
    }

    pub async fn sign_message<S: Send + Sync + AsRef<[u8]>>(
        &self,
        msg: S,
    ) -> Result<Signature, CallError> {
        let msg = unsafe { std::str::from_utf8_unchecked(msg.as_ref()) };
        self.client
            .personal_sign(&[msg, &self.address_string()])
            .await
    }

    pub async fn sign_transaction(&self, msg: &TypedTransaction) -> Result<Signature, CallError> {
        let to = if let Some(NameOrAddress::Address(address)) = msg.to() {
            Some(*address)
        } else {
            None
        };
        let tx = Transaction {
            from: *msg.from().unwrap(),
            to,
            gas_limit: None,
            gas_price: msg.gas_price(),
            value: *msg.value().unwrap_or(&U256::from(0)),
            data: msg.data().unwrap().to_vec(),
            nonce: msg.nonce().copied(),
        };

        let raw = self.client.sign_transaction(tx).await?.to_vec();
        assert_eq!(raw[raw.len() - 66], 160);
        assert_eq!(raw[raw.len() - 33], 160);

        // Transform `v` according to:
        // https://github.com/ethereum/EIPs/blob/master/EIPS/eip-155.md#specification
        let mut v = raw[raw.len() - 67] as u64;
        if v == 27 || v == 28 {
            v += 2 * self.chain_id() + 8;
        }

        Ok(Signature {
            v,
            r: U256::from(&raw[raw.len() - 65..raw.len() - 33]),
            s: U256::from(&raw[raw.len() - 32..]),
        })
    }
}
