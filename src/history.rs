use crate::core::policy::CallerId;
use crate::core::status::OperationStatus;
use crate::core::types::{FfiTxDigest, FfiTxPage};
use crate::indexer::LezIndexer;
use crate::storage::cursors::CursorState;
use crate::storage::{CF_ACC_META, CF_ACC_TO_TX, CF_TX_TO_ID};
use std::ffi::CString;
use std::ptr;

impl LezIndexer {
    /// F#37–43, R#5: Paginated transaction history with opaque, stable cursors.
    pub fn query_transactions_by_account(
        &self,
        caller_id: &CallerId,
        account_id: &[u8; 32],
        cursor_token: Option<&str>,
        limit: usize,
        oldest_first: bool,
    ) -> Result<FfiTxPage, OperationStatus> {
        if limit == 0 {
            return Err(OperationStatus::InvalidArgumentLimit);
        }

        let (mut current_index, snapshot_tip, order_is_oldest) = match cursor_token {
            Some(tok) => {
                let state = self.cursors.validate_token(tok, caller_id, account_id)?;
                (state.next_index, state.snapshot_tip, state.oldest_first)
            }
            None => {
                let cf_acc_meta = self.store.cf(CF_ACC_META)?;
                let total = match self
                    .store
                    .db()
                    .get_cf(&cf_acc_meta, account_id)
                    .map_err(|_| OperationStatus::BackendFailureStorage)?
                {
                    Some(val) if val.len() >= 8 => {
                        u64::from_be_bytes(val[0..8].try_into().unwrap())
                    }
                    _ => 0,
                };
                if total == 0 {
                    return Ok(FfiTxPage {
                        items: ptr::null_mut(),
                        count: 0,
                        has_more: false,
                        next_cursor: ptr::null_mut(),
                    });
                }
                let start = if oldest_first {
                    0
                } else {
                    total.saturating_sub(1)
                };
                (start, total, oldest_first)
            }
        };

        let cf_att = self.store.cf(CF_ACC_TO_TX)?;
        let cf_tx = self.store.cf(CF_TX_TO_ID)?;
        let mut digests = Vec::with_capacity(limit);

        let mut has_more = false;
        let mut next_cursor_str = None;

        for _ in 0..limit {
            if order_is_oldest {
                if current_index >= snapshot_tip {
                    break;
                }
            } else if current_index >= snapshot_tip {
                break;
            }

            let mut key = Vec::with_capacity(40);
            key.extend_from_slice(account_id);
            key.extend_from_slice(&current_index.to_be_bytes());

            if let Some(tx_hash_bytes) = self
                .store
                .db()
                .get_cf(&cf_att, &key)
                .map_err(|_| OperationStatus::BackendFailureStorage)?
            {
                let mut tx_hash = [0u8; 32];
                tx_hash.copy_from_slice(&tx_hash_bytes[0..32]);

                let block_h = match self
                    .store
                    .db()
                    .get_cf(&cf_tx, tx_hash)
                    .map_err(|_| OperationStatus::BackendFailureStorage)?
                {
                    Some(b) if b.len() >= 8 => u64::from_be_bytes(b[0..8].try_into().unwrap()),
                    _ => 0,
                };

                digests.push(FfiTxDigest {
                    tx_hash,
                    block_height: block_h,
                    index_in_block: (current_index % 2) as u32,
                });
            }

            if order_is_oldest {
                current_index += 1;
                if current_index >= snapshot_tip {
                    break;
                }
            } else {
                if current_index == 0 {
                    break;
                }
                current_index -= 1;
            }
        }

        // Determine if more records exist within snapshot boundary
        if (order_is_oldest && current_index < snapshot_tip)
            || (!order_is_oldest && current_index > 0)
        {
            has_more = true;
        }

        if has_more {
            let tok = self.cursors.create_token(CursorState {
                caller_id: *caller_id,
                account_id: *account_id,
                next_index: current_index,
                snapshot_tip,
                oldest_first: order_is_oldest,
                created_at: std::time::Instant::now(),
            });
            next_cursor_str = Some(tok);
        }

        let next_cursor_ptr = match next_cursor_str {
            Some(s) => match CString::new(s) {
                Ok(c) => c.into_raw(),
                Err(_) => return Err(OperationStatus::BackendFailureSerialization),
            },
            None => ptr::null_mut(),
        };

        let boxed_slice = digests.into_boxed_slice();
        let count = boxed_slice.len();
        let items_ptr = Box::into_raw(boxed_slice) as *mut FfiTxDigest;

        Ok(FfiTxPage {
            items: items_ptr,
            count,
            has_more,
            next_cursor: next_cursor_ptr,
        })
    }

    /// F#58: Machine-readable schema export.
    pub fn query_schema(&self) -> String {
        serde_json::json!({
            "spec": "RFP-027",
            "version": "0.1.0",
            "abi_version": {
                "major": 1,
                "minor": 0,
                "patch": 0
            },
            "operations": [
                { "name": "lez_abi_version", "type": "meta", "id": "Audit#5" },
                { "name": "lez_query_chain_tip", "type": "query", "id": "F#47" },
                { "name": "lez_query_account", "type": "query", "id": "F#10" },
                { "name": "lez_query_account_at_block", "type": "query", "id": "F#11" },
                { "name": "lez_query_accounts", "type": "query", "id": "F#13" },
                { "name": "lez_query_transactions_by_account", "type": "query", "id": "F#37-43" },
                { "name": "lez_query_schema", "type": "query", "id": "F#58" },
                { "name": "lez_admin_start_indexer", "type": "admin", "id": "F#67" },
                { "name": "lez_admin_stop_indexer", "type": "admin", "id": "F#67" },
                { "name": "lez_admin_reset_storage", "type": "admin", "id": "F#67" }
            ]
        })
        .to_string()
    }
}
