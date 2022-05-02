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

#[derive(Debug, thiserror::Error)]
pub enum WalletError {
    #[error(transparent)]
    Call(#[from] CallError),
    #[error("failed to sign the tx")]
    TransactionSignature,
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
    ) -> Result<Signature, WalletError> {
        let msg = unsafe { std::str::from_utf8_unchecked(msg.as_ref()) };
        self.client
            .personal_sign(&[msg, &self.address_string()])
            .await
            .map_err(WalletError::from)
    }

    pub async fn sign_transaction(&self, msg: &TypedTransaction) -> Result<Signature, WalletError> {
        let to = if let Some(NameOrAddress::Address(address)) = msg.to() {
            Some(*address)
        } else {
            None
        };
        let tx = Transaction {
            from: *msg.from().unwrap(),
            to,
            gas_limit: msg.gas().cloned(),
            gas_price: msg.gas_price(),
            value: *msg.value().unwrap_or(&U256::from(0)),
            data: msg.data().unwrap().to_vec(),
            nonce: None,
        };

        let raw = self.client.sign_transaction(tx).await?.to_vec();
        let mut v_r_s = None;
        for offset in 0..7 {
            let mut head = raw.len() - 67 + offset;
            v_r_s = extract_v_r_s(&raw[head..]);
            if v_r_s.is_some() {
                break;
            }

            if offset == 0 {
                continue;
            }
            head = raw.len() - 67 - offset;
            v_r_s = extract_v_r_s(&raw[head..]);
            if v_r_s.is_some() {
                break;
            }
        }

        let (v, r, s) = v_r_s.ok_or(WalletError::TransactionSignature)?;
        Ok(Signature {
            v,
            r: U256::from(r),
            s: U256::from(s),
        })
    }
}

fn extract_v_r_s(tx: &[u8]) -> Option<(u64, &[u8], &[u8])> {
    let mut head = 0_usize;
    let v: u64 = tx[head].into();

    head += 1;
    if tx[head] <= 0x80 {
        return None;
    }
    let len_r = (tx[head] - 0x80) as usize;
    if head + len_r >= tx.len() {
        return None;
    }
    let r = &tx[head + 1..head + 1 + len_r];

    head += 1 + len_r;
    if tx[head] <= 0x80 {
        return None;
    }
    let len_s = (tx[head] - 0x80) as usize;
    if head + len_s >= tx.len() {
        return None;
    }
    let s = &tx[head + 1..head + 1 + len_s];

    if 1 + r.len() + s.len() + 2 != tx.len() {
        return None;
    }

    Some((v, r, s))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_regular_sig() {
        let tx = [
            0x1c, 0xa0, 0x88, 0xff, 0x6c, 0xf0, 0xfe, 0xfd, 0x94, 0xdb, 0x46, 0x11, 0x11, 0x49,
            0xae, 0x4b, 0xfc, 0x17, 0x9e, 0x9b, 0x94, 0x72, 0x1f, 0xff, 0xd8, 0x21, 0xd3, 0x8d,
            0x16, 0x46, 0x4b, 0x3f, 0x71, 0xd0, 0xa0, 0x45, 0xe0, 0xaf, 0xf8, 0x00, 0x96, 0x1c,
            0xfc, 0xe8, 0x05, 0xda, 0xef, 0x70, 0x16, 0xb9, 0xb6, 0x75, 0xc1, 0x37, 0xa6, 0xa4,
            0x1a, 0x54, 0x8f, 0x7b, 0x60, 0xa3, 0x48, 0x4c, 0x06, 0xa3, 0x3a,
        ];

        let v_r_s = extract_v_r_s(&tx);
        assert!(v_r_s.is_some());
        let (v, r, s) = v_r_s.unwrap();

        assert_eq!(v, 0x1c);
        assert_eq!(r, &tx[tx.len() - 65..tx.len() - 33]);
        assert_eq!(s, &tx[tx.len() - 32..]);
    }

    #[test]
    fn test_variable_sig() {
        let tx = [
            0x2c, 0xa0, 0x09, 0x0c, 0x0a, 0x25, 0xaf, 0x16, 0x3b, 0x51, 0x86, 0xd5, 0x6f, 0x61,
            0xd2, 0xd1, 0xe7, 0xcf, 0xf1, 0x05, 0xb8, 0x9e, 0x24, 0xed, 0x48, 0x26, 0x7c, 0x43,
            0xa0, 0x22, 0x27, 0xd9, 0xf7, 0x14, 0x9f, 0x9b, 0xcc, 0xf7, 0x3a, 0xef, 0xa7, 0x7d,
            0x2c, 0xcb, 0x0b, 0x81, 0x59, 0x15, 0x04, 0xde, 0xcc, 0x07, 0xc1, 0x26, 0x92, 0xf9,
            0x0f, 0xfe, 0x47, 0xd0, 0xf0, 0xbd, 0xea, 0x99, 0xa6, 0x8d,
        ];

        let v_r_s = extract_v_r_s(&tx);
        assert!(v_r_s.is_some());
        let (v, r, s) = v_r_s.unwrap();

        assert_eq!(v, 0x2c);
        assert_eq!(r, &tx[tx.len() - 64..tx.len() - 32]);
        assert_eq!(s, &tx[tx.len() - 31..]);
    }

    #[test]
    fn test_malformed_sig() {
        let tx = [
            0x2c, 0xa0, 0x09, 0x0c, 0x0a, 0x25, 0xaf, 0x16, 0x3b, 0x51, 0x86, 0xd5, 0x6f, 0x61,
            0xd2, 0xd1, 0xe7, 0xcf, 0xf1, 0x05, 0xb8, 0x9e, 0x24, 0xed, 0x48, 0x26, 0x7c, 0x43,
            0xa0, 0x22, 0x27, 0xd9, 0xf7, 0x14, 0x81, 0x9b, 0xcc, 0xf7, 0x3a, 0xef, 0xa7, 0x7d,
            0x2c, 0xcb, 0x0b, 0x81, 0x59, 0x15, 0x04, 0xde, 0xcc, 0x07, 0xc1, 0x26, 0x92, 0xf9,
            0x0f, 0xfe, 0x47, 0xd0, 0xf0, 0xbd, 0xea, 0x99, 0xa6, 0x8d,
        ];

        let v_r_s = extract_v_r_s(&tx);
        assert!(v_r_s.is_none());
    }
}
