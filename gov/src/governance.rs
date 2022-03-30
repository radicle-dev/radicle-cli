use std::sync::Arc;

use ethers::contract::ContractError;
use ethers::prelude::{signer::SignerMiddlewareError, Http, Middleware, ProviderError};
use ethers::types::{Address, U256};
use ethers::{
    abi::Abi,
    contract::{AbiError, Contract},
    prelude::builders::ContractCall,
    providers::Provider,
};

use std::str::FromStr;

use rad_common::ethereum;

const RADICLE_GOVERNANCE_ADDRESS: &str = "0x690e775361AD66D1c4A25d89da9fCd639F5198eD";
const PUBLIC_RESOLVER_ABI: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/abis/Governance.json"));

pub struct Governance<M> {
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
    SignerMiddleware(#[from] SignerMiddlewareError<Provider<Http>, ethereum::Wallet>),
}

type Proposal = (Address, U256, U256, U256, U256, U256, bool, bool);

impl<M> Governance<M>
where
    M: Middleware,
    Error<M>: From<<M as Middleware>::Error>,
{
    pub fn new(client: impl Into<Arc<M>>) -> Self {
        let abi: Abi = serde_json::from_str(PUBLIC_RESOLVER_ABI).expect("The ABI is valid");
        let addr = Address::from_str(RADICLE_GOVERNANCE_ADDRESS).unwrap();
        let contract = Contract::new(addr, abi, client);

        Self { contract }
    }

    pub async fn get_proposal(&self, id: U256) -> Result<Proposal, Error<M>> {
        let proposal: Proposal = self
            .contract
            .method("proposals", id)
            .map_err(ContractError::from)?
            .call()
            .await?;

        Ok(proposal)
    }

    pub fn cast_vote(&self, id: U256, support: bool) -> Result<ContractCall<M, ()>, AbiError> {
        self.contract.method("castVote", (id, support))
    }

    pub fn propose(
        &self,
        targets: Vec<Address>,
        values: Vec<U256>,
        signatures: Vec<String>,
        calldatas: Vec<Vec<u8>>,
        description: String,
    ) -> Result<ContractCall<M, ()>, AbiError> {
        use ethers::core::abi::Token;

        self.contract.method(
            "propose",
            (
                Token::Array(targets.into_iter().map(Token::Address).collect()),
                Token::Array(values.into_iter().map(Token::Uint).collect()),
                Token::Array(signatures.into_iter().map(Token::String).collect()),
                Token::Array(calldatas.into_iter().map(Token::Bytes).collect()),
                description,
            ),
        )
    }
}
