use std::sync::Arc;

use ethers::prelude::{signer::SignerMiddlewareError, Http, Middleware, ProviderError};
use ethers::types::{Address, U256};
use ethers::{
    abi::Abi,
    contract::{AbiError, Contract, ContractError},
    providers::Provider,
};

use std::str::FromStr;

use rad_common::ethereum::{
    self,
    signer::{ContractCall, ExtendedMiddleware},
};

const RADICLE_GOVERNANCE_ADDRESS: &str = "0x690e775361AD66D1c4A25d89da9fCd639F5198eD";
const PUBLIC_RESOLVER_ABI: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/abis/Governance.json"));

pub struct Governance<M> {
    contract: Contract<M>,
    legacy: bool,
    client: Arc<M>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error<M: Middleware> {
    #[error("Expected proposal state to be {1}, but currently is {0}")]
    ProposalStateMismatch(ProposalState, ProposalState),
    #[error(transparent)]
    Provider(#[from] ProviderError),
    #[error(transparent)]
    Contract(#[from] ContractError<M>),
    #[error(transparent)]
    Abi(#[from] ethers::abi::Error),
    #[error(transparent)]
    ContractAbi(#[from] AbiError),
    #[error(transparent)]
    SignerMiddleware(#[from] SignerMiddlewareError<Provider<Http>, ethereum::TypedWallet>),
}

type Proposal = (Address, U256, U256, U256, U256, U256, bool, bool);

#[derive(PartialEq, Debug)]
pub enum ProposalState {
    Pending,
    Active,
    Canceled,
    Defeated,
    Succeeded,
    Queued,
    Expired,
    Executed,
    Undefined,
}

impl std::fmt::Display for ProposalState {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl<M> Governance<M>
where
    M: ExtendedMiddleware,
    Error<M>: From<<M as Middleware>::Error>,
{
    pub fn new(client: impl Into<Arc<M>>, legacy: bool) -> Self {
        let abi: Abi = serde_json::from_str(PUBLIC_RESOLVER_ABI).expect("The ABI is valid");
        let addr = Address::from_str(RADICLE_GOVERNANCE_ADDRESS).unwrap();
        let client: Arc<M> = client.into();
        let contract = Contract::new(addr, abi, Arc::clone(&client));

        Self {
            contract,
            legacy,
            client,
        }
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

    async fn get_proposal_state(&self, id: U256) -> Result<ProposalState, Error<M>> {
        let state = self
            .contract
            .method("state", id)
            .map_err(ContractError::from)?
            .call()
            .await?;

        let state = match state {
            0 => ProposalState::Pending,
            1 => ProposalState::Active,
            2 => ProposalState::Canceled,
            3 => ProposalState::Defeated,
            4 => ProposalState::Succeeded,
            5 => ProposalState::Queued,
            6 => ProposalState::Expired,
            7 => ProposalState::Executed,
            _ => ProposalState::Undefined,
        };

        Ok(state)
    }

    pub fn cast_vote(&self, id: U256, support: bool) -> Result<ContractCall<M, ()>, AbiError> {
        self.contract
            .method("castVote", (id, support))
            .map(|tx| self.to_contract_call(tx))
    }

    pub async fn queue_proposal(&self, id: U256) -> Result<ContractCall<M, ()>, Error<M>> {
        let state = self.get_proposal_state(id).await?;
        let wanted = ProposalState::Succeeded;
        if state != wanted {
            return Err(Error::ProposalStateMismatch(state, wanted));
        }

        self.contract
            .method("queue", id)
            .map_err(Error::ContractAbi)
            .map(|tx| self.to_contract_call(tx))
    }

    pub async fn execute_proposal(&self, id: U256) -> Result<ContractCall<M, ()>, Error<M>> {
        let state = self.get_proposal_state(id).await?;
        let wanted = ProposalState::Queued;
        if state != wanted {
            return Err(Error::ProposalStateMismatch(state, wanted));
        }

        self.contract
            .method("execute", id)
            .map_err(Error::ContractAbi)
            .map(|tx| self.to_contract_call(tx))
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

        self.contract
            .method(
                "propose",
                (
                    Token::Array(targets.into_iter().map(Token::Address).collect()),
                    Token::Array(values.into_iter().map(Token::Uint).collect()),
                    Token::Array(signatures.into_iter().map(Token::String).collect()),
                    Token::Array(calldatas.into_iter().map(Token::Bytes).collect()),
                    description,
                ),
            )
            .map(|tx| self.to_contract_call(tx))
    }

    fn to_contract_call(
        &self,
        inner: ethers::contract::builders::ContractCall<M, ()>,
    ) -> ContractCall<M, ()> {
        ContractCall {
            inner,
            client: Arc::clone(&self.client),
            legacy: self.legacy,
        }
    }
}
