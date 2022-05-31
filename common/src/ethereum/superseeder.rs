use std::str::FromStr;
use std::sync::Arc;

use ethers::prelude::builders::ContractCall;
use ethers::prelude::{signer::SignerMiddlewareError, Http, Lazy, Middleware, ProviderError};
use ethers::types::{Address, U256};
use ethers::{
    abi::Abi,
    contract::{AbiError, Contract, ContractError},
    providers::Provider,
};

use crate::ethereum;

const ABI: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/abis/Superseeder.json"
));

pub static SUPERSEEDER_ADDRESS: Lazy<Address> =
    Lazy::new(|| Address::from_str("0xa5b017164b97ef1adadc6d2ff84031c84edd8e78").unwrap());

pub struct Superseeder<M> {
    contract: Contract<M>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error<M: Middleware> {
    #[error(transparent)]
    Provider(#[from] ProviderError),
    #[error(transparent)]
    Contract(#[from] ContractError<M>),
    #[error(transparent)]
    Abi(#[from] ethers::abi::Error),
    #[error(transparent)]
    ContractAbi(#[from] AbiError),
    #[error(transparent)]
    SignerMiddleware(#[from] SignerMiddlewareError<Provider<Http>, ethereum::Wallet>),
}

impl<M> Superseeder<M>
where
    M: Middleware,
    Error<M>: From<<M as Middleware>::Error>,
{
    pub fn new(client: impl Into<Arc<M>>) -> Self {
        let abi: Abi = serde_json::from_str(ABI).expect("The ABI is valid");
        let contract = Contract::new(*SUPERSEEDER_ADDRESS, abi, client);

        Self { contract }
    }

    pub fn send(
        &self,
        erc20: Address,
        receivers: Vec<Address>,
        amounts: Vec<U256>,
    ) -> Result<ContractCall<M, ()>, AbiError> {
        use ethers::core::abi::Token;

        self.contract.method(
            "seed",
            (
                erc20,
                Token::Array(receivers.into_iter().map(Token::Address).collect()),
                Token::Array(amounts.into_iter().map(Token::Uint).collect()),
            ),
        )
    }
}
