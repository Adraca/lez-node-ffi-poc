use std::ptr;
use crate::core::status::OperationStatus;
use crate::core::types::{FfiAccount, FfiAccountsBatch, FfiChainTip};
use crate::indexer::LezIndexer;
use crate::storage::{CF_BLOCK, CF_META, CF_STATE};

impl LezIndexer {
    pub fn query_chain_tip(&self) -> Result<FfiChainTip, OperationStatus> {
        let cf = self.store.cf(CF_META)?;
        let val = self.store.db().get_cf(&cf, b"chain_tip")
            .map_err(|_| OperationStatus::BackendFailureStorage)?
            .ok_or(OperationStatus::NotFoundBlock)?;

        if val.len() < 48 { return Err(OperationStatus::BackendFailureStorage); }
        let height = u64::from_be_bytes(val[0..8].try_into().unwrap());
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&val[8..40]);
        let timestamp = u64::from_be_bytes(val[40..48].try_into().unwrap());
        Ok(FfiChainTip { height, hash, timestamp })
    }

    /// F#10: Returns account state or default zeroed account for unknown ID.
    pub fn query_account(&self, account_id: &[u8; 32]) -> Result<FfiAccount, OperationStatus> {
        let cf = self.store.cf(CF_STATE)?;
        match self.store.db().get_cf(&cf, account_id).map_err(|_| OperationStatus::BackendFailureStorage)? {
            Some(val) if val.len() >= 16 => {
                let nonce = u64::from_be_bytes(val[0..8].try_into().unwrap());
                let balance = u64::from_be_bytes(val[8..16].try_into().unwrap());
                Ok(FfiAccount { account_id: *account_id, nonce, balance, shards: ptr::null_mut(), shards_len: 0 })
            }
            _ => Ok(FfiAccount::zeroed(*account_id)),
        }
    }

    /// F#11, F#14: Pinned height + hash verification.
    pub fn query_account_at_block(&self, account_id: &[u8; 32], height: u64, hash: &[u8; 32]) -> Result<FfiAccount, OperationStatus> {
        let cf_block = self.store.cf(CF_BLOCK)?;
        let block_val = self.store.db().get_cf(&cf_block, height.to_be_bytes())
            .map_err(|_| OperationStatus::BackendFailureStorage)?
            .ok_or(OperationStatus::NotFoundBlock)?;

        if block_val.len() < 32 || &block_val[0..32] != hash {
            return Err(OperationStatus::InvalidArgumentBlockPin);
        }

        let cf_state = self.store.cf(CF_STATE)?;
        let mut pinned_key = Vec::with_capacity(40);
        pinned_key.extend_from_slice(account_id);
        pinned_key.extend_from_slice(&height.to_be_bytes());

        match self.store.db().get_cf(&cf_state, &pinned_key).map_err(|_| OperationStatus::BackendFailureStorage)? {
            Some(val) if val.len() >= 16 => {
                let nonce = u64::from_be_bytes(val[0..8].try_into().unwrap());
                let balance = u64::from_be_bytes(val[8..16].try_into().unwrap());
                Ok(FfiAccount { account_id: *account_id, nonce, balance, shards: ptr::null_mut(), shards_len: 0 })
            }
            _ => Ok(FfiAccount::zeroed(*account_id)),
        }
    }

    /// F#13, R#4: Batch account read via batched_multi_get_cf with pinned snapshot (consistent, not yet faster).
    pub fn query_accounts(&self, account_ids: &[[u8; 32]]) -> Result<FfiAccountsBatch, OperationStatus> {
        let cf = self.store.cf(CF_STATE)?;
        let snapshot = self.store.db().snapshot();
        let mut read_opts = rocksdb::ReadOptions::default();
        read_opts.set_snapshot(&snapshot);

        let is_sorted = account_ids.windows(2).all(|w| w[0] <= w[1]);
        let results = self.store.db().batched_multi_get_cf_opt(&cf, account_ids, is_sorted, &read_opts);

        let mut accounts = Vec::with_capacity(account_ids.len());
        for (idx, res) in results.into_iter().enumerate() {
            let acc = match res.map_err(|_| OperationStatus::BackendFailureStorage)? {
                Some(val) if val.len() >= 16 => {
                    let nonce = u64::from_be_bytes(val[0..8].try_into().unwrap());
                    let balance = u64::from_be_bytes(val[8..16].try_into().unwrap());
                    FfiAccount { account_id: account_ids[idx], nonce, balance, shards: ptr::null_mut(), shards_len: 0 }
                }
                _ => FfiAccount::zeroed(account_ids[idx]),
            };
            accounts.push(acc);
        }

        let boxed_slice = accounts.into_boxed_slice();
        let count = boxed_slice.len();
        let accounts_ptr = Box::into_raw(boxed_slice) as *mut FfiAccount;
        Ok(FfiAccountsBatch { accounts: accounts_ptr, count })
    }
}
