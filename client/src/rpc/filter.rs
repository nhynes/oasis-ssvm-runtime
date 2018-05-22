use bigint::{Address, H256, U256};
use hexutil::*;
use block::{Log, Receipt};
use sha3::{Digest, Keccak256};
use std::str::FromStr;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use blockchain::chain::HeaderHash;
use rpc::RPCLogFilter;
use std::time::{Duration, SystemTime};
use std::thread;

use super::{RPCLog, Either};
use super::util::*;
use ekiden_rpc_client;

use error::Error;
use rlp;
use evm;
use futures::future::Future;

#[derive(Clone, Debug)]
pub enum TopicFilter {
    All,
    Or(Vec<H256>),
}

#[derive(Clone, Debug)]
pub struct LogFilter {
    pub from_block: usize,
    pub to_block: usize,
    pub address: Option<Address>,
    pub topics: Vec<TopicFilter>,
}

#[derive(Clone, Debug)]
pub enum Filter {
    PendingTransaction(usize),
    Block(usize, SystemTime),
    Log(LogFilter),
}

/*
fn check_log(log: &Log, index: usize, filter: &TopicFilter) -> bool {
    match filter {
        &TopicFilter::All => true,
        &TopicFilter::Or(ref hashes) => {
            if log.topics.len() >= index {
                false
            } else {
                let mut matched = false;
                for hash in hashes {
                    if hash == &log.topics[index] {
                        matched = true;
                    }
                }
                matched
            }
        },
    }
}

pub fn get_logs(state: &MinerState, filter: LogFilter) -> Result<Vec<RPCLog>, Error> {
    let mut current_block_number = filter.from_block;
    let mut ret = Vec::new();

    while current_block_number >= filter.to_block {
        if current_block_number > state.block_height() {
            break;
        }

        let block = state.get_block_by_number(current_block_number);
        for transaction in &block.transactions {
            let transaction_hash = H256::from(Keccak256::digest(&rlp::encode(transaction).to_vec()).as_slice());
            let receipt = state.get_receipt_by_transaction_hash(transaction_hash)?;
            for i in 0..receipt.logs.len() {
                let log = &receipt.logs[i];
                if check_log(log, 0, &filter.topics[0]) &&
                    check_log(log, 1, &filter.topics[1]) &&
                    check_log(log, 2, &filter.topics[2]) &&
                    check_log(log, 3, &filter.topics[3]) &&
                    match filter.address {
                        Some(address) => address == log.address,
                        None => true,
                    }
                    {
                        ret.push(to_rpc_log(&receipt, i, transaction, &block));
                    }
            }
        }

        current_block_number += 1;
    }

    return Ok(ret);
}
*/

pub struct FilterManager {
    client: Arc<Mutex<evm::Client<ekiden_rpc_client::backend::Web3RpcClientBackend>>>,
    filters: HashMap<usize, Filter>,
    unmodified_filters: HashMap<usize, Filter>,
}

impl FilterManager {
    pub fn new(client: Arc<Mutex<evm::Client<ekiden_rpc_client::backend::Web3RpcClientBackend>>>) -> Self {
        FilterManager {
            client,
            filters: HashMap::new(),
            unmodified_filters: HashMap::new(),
        }
    }

    pub fn from_log_filter(&self, log: RPCLogFilter) -> Result<LogFilter, Error> {
        /*
        let state = self.state.lock().unwrap();
        from_log_filter(&state, log)
        */
        Err(Error::NotImplemented)
    }

    pub fn install_log_filter(&mut self, filter: LogFilter) -> usize {
        let id = self.filters.len();
        self.filters.insert(id, Filter::Log(filter.clone()));
        self.unmodified_filters.insert(id, Filter::Log(filter.clone()));
        id
    }

    pub fn install_block_filter(&mut self) -> usize {
        let mut client = self.client.lock().unwrap();

        let block_height_str = client.get_block_height(true).wait().unwrap();
        let block_height = U256::from_str(&block_height_str).unwrap().as_usize();

        let id = self.filters.len();
        let current_time = SystemTime::now();
        self.filters.insert(id, Filter::Block(block_height, current_time));
        self.unmodified_filters.insert(id, Filter::Block(block_height, current_time));
        id
    }

    pub fn install_pending_transaction_filter(&mut self) -> usize {
        /*
        let mut client = self.client.lock().unwrap();

        let pending_transactions = state.all_pending_transaction_hashes();
        let id = self.filters.len();
        self.filters.insert(id, Filter::PendingTransaction(pending_transactions.len()));
        self.unmodified_filters.insert(id, Filter::PendingTransaction(pending_transactions.len()));
        id
        */
        0usize
    }

    pub fn uninstall_filter(&mut self, id: usize) {
        self.filters.remove(&id);
        self.unmodified_filters.remove(&id);
    }

    pub fn get_logs(&mut self, id: usize) -> Result<Vec<RPCLog>, Error> {
        /*
        let state = self.state.lock().unwrap();

        let filter = self.unmodified_filters.get(&id).ok_or(Error::NotFound)?;

        match filter {
            &Filter::Log(ref filter) => {
                let ret = get_logs(&state, filter.clone())?;
                Ok(ret)
            },
            _ => Err(Error::NotFound),
        }
        */
        Err(Error::NotImplemented)
    }

    pub fn get_changes(&mut self, id: usize) -> Result<Either<Vec<String>, Vec<RPCLog>>, Error> {
        let filter = self.filters.get_mut(&id).ok_or(Error::NotFound)?;

        match filter {
            &mut Filter::Block(ref mut next_start, ref mut last_checked) => {
                /// If we have called into the contract within the last 3 seconds for this filter,
                /// short-circuit to return no new blocks.
                ///
                /// NOTE: this is temporary workaround to prevent spamming, since tools such as
                /// truffle will request new changes every second and it currently takes more than
                /// a second to service each request, creating a backlog that prevents us from
                /// processing any other requests.
                ///
                /// TODO: figure out a better caching system for this method as well as others (tracked at #252)
                let since_last_check = last_checked.elapsed().unwrap().as_secs();
                if since_last_check < 3 {
                    return Ok(Either::Left(Vec::new()))
                }

                *last_checked = SystemTime::now();
                let mut client = self.client.lock().unwrap();

                let block_hashes = client.get_latest_block_hashes(format!("0x{:x}", *next_start)).wait().unwrap();
                *next_start += block_hashes.len();
                Ok(Either::Left(block_hashes))
            },
            /*
            &mut Filter::PendingTransaction(ref mut next_start) => {
                let pending_transactions = state.all_pending_transaction_hashes();
                let mut ret = Vec::new();
                while *next_start < pending_transactions.len() {
                    ret.push(format!("0x{:x}", &pending_transactions[*next_start]));
                    *next_start += 1;
                }
                Ok(Either::Left(ret))
            },
            &mut Filter::Log(ref mut filter) => {
                let ret = get_logs(&state, filter.clone())?;
                filter.from_block = state.block_height() + 1;
                Ok(Either::Right(ret))
            },
            */
            _ => { return Err(Error::NotImplemented) }
        }
    }
}