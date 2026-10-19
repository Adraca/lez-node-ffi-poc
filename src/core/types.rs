use std::ffi::{c_char, CString};
use std::ptr;

#[repr(C)]
#[derive(Debug)]
pub struct FfiShard {
    pub program: [u8; 32],
    pub data: *mut u8,
    pub data_len: usize,
    pub data_cap: usize,
}

#[repr(C)]
#[derive(Debug)]
pub struct FfiAccount {
    pub account_id: [u8; 32],
    pub nonce: u64,
    pub balance: u64,
    pub shards: *mut FfiShard,
    pub shards_len: usize,
}

impl FfiAccount {
    pub fn zeroed(account_id: [u8; 32]) -> Self {
        Self {
            account_id,
            nonce: 0,
            balance: 0,
            shards: ptr::null_mut(),
            shards_len: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct FfiAccountsBatch {
    pub accounts: *mut FfiAccount,
    pub count: usize,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct FfiChainTip {
    pub height: u64,
    pub hash: [u8; 32],
    pub timestamp: u64,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct FfiTxDigest {
    pub tx_hash: [u8; 32],
    pub block_height: u64,
    pub index_in_block: u32,
}

#[repr(C)]
#[derive(Debug)]
pub struct FfiTxPage {
    pub items: *mut FfiTxDigest,
    pub count: usize,
    pub has_more: bool,
    pub next_cursor: *mut c_char,
}

/// Frees an FfiAccount and its internal shard array.
///
/// # Safety
/// If `ptr` is non-null, it must have been allocated by lez-node-ffi-poc. Passing NULL is safe and a no-op.
#[no_mangle]
pub unsafe extern "C" fn lez_free_account(ptr: *mut FfiAccount) {
    if ptr.is_null() { return; }
    let account = unsafe { Box::from_raw(ptr) };
    if !account.shards.is_null() && account.shards_len > 0 {
        let mut shards = unsafe {
            Vec::from_raw_parts(account.shards, account.shards_len, account.shards_len)
        };
        for shard in shards.iter_mut() {
            if !shard.data.is_null() && shard.data_cap > 0 {
                let _ = unsafe { Vec::from_raw_parts(shard.data, shard.data_len, shard.data_cap) };
            }
        }
    }
}

/// Frees an FfiAccountsBatch.
///
/// # Safety
/// If `ptr` is non-null, it must have been allocated by lez-node-ffi-poc. Passing NULL is safe and a no-op.
#[no_mangle]
pub unsafe extern "C" fn lez_free_accounts_batch(ptr: *mut FfiAccountsBatch) {
    if ptr.is_null() { return; }
    let batch = unsafe { Box::from_raw(ptr) };
    if !batch.accounts.is_null() && batch.count > 0 {
        let mut accounts = unsafe {
            Vec::from_raw_parts(batch.accounts, batch.count, batch.count)
        };
        for acc in accounts.iter_mut() {
            if !acc.shards.is_null() && acc.shards_len > 0 {
                let mut shards = unsafe {
                    Vec::from_raw_parts(acc.shards, acc.shards_len, acc.shards_len)
                };
                for shard in shards.iter_mut() {
                    if !shard.data.is_null() && shard.data_cap > 0 {
                        let _ = unsafe { Vec::from_raw_parts(shard.data, shard.data_len, shard.data_cap) };
                    }
                }
            }
        }
    }
}

/// Frees an FfiTxPage.
///
/// # Safety
/// If `ptr` is non-null, it must have been allocated by lez-node-ffi-poc. Passing NULL is safe and a no-op.
#[no_mangle]
pub unsafe extern "C" fn lez_free_tx_page(ptr: *mut FfiTxPage) {
    if ptr.is_null() { return; }
    let page = unsafe { Box::from_raw(ptr) };
    if !page.items.is_null() && page.count > 0 {
        let _ = unsafe { Vec::from_raw_parts(page.items, page.count, page.count) };
    }
    if !page.next_cursor.is_null() {
        let _ = unsafe { CString::from_raw(page.next_cursor) };
    }
}

/// Frees an FfiShard.
///
/// # Safety
/// If `ptr` is non-null, it must have been allocated by lez-node-ffi-poc. Passing NULL is safe and a no-op.
#[no_mangle]
pub unsafe extern "C" fn lez_free_shard(ptr: *mut FfiShard) {
    if ptr.is_null() { return; }
    let shard = unsafe { Box::from_raw(ptr) };
    if !shard.data.is_null() && shard.data_cap > 0 {
        let _ = unsafe { Vec::from_raw_parts(shard.data, shard.data_len, shard.data_cap) };
    }
}

/// Frees an FfiChainTip.
///
/// # Safety
/// If `ptr` is non-null, it must have been allocated by lez-node-ffi-poc. Passing NULL is safe and a no-op.
#[no_mangle]
pub unsafe extern "C" fn lez_free_chain_tip(ptr: *mut FfiChainTip) {
    if ptr.is_null() { return; }
    let _ = unsafe { Box::from_raw(ptr) };
}
