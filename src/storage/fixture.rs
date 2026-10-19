use rocksdb::WriteBatch;
use crate::core::status::OperationStatus;
use crate::storage::{
    RocksStore, CF_ACC_META, CF_ACC_TO_TX, CF_BLOCK, CF_HASH_TO_ID, CF_META, CF_STATE, CF_TX_TO_ID,
};

/// Populates the RocksDB store with a deterministic set of blocks, txs, and accounts.
pub fn load_deterministic_fixtures(
    store: &RocksStore,
    num_blocks: u64,
    accounts: &[[u8; 32]],
) -> Result<(), OperationStatus> {
    let cf_block = store.cf(CF_BLOCK)?;
    let cf_meta = store.cf(CF_META)?;
    let cf_hash_to_id = store.cf(CF_HASH_TO_ID)?;
    let cf_tx_to_id = store.cf(CF_TX_TO_ID)?;
    let cf_acc_meta = store.cf(CF_ACC_META)?;
    let cf_acc_to_tx = store.cf(CF_ACC_TO_TX)?;
    let cf_state = store.cf(CF_STATE)?;

    let mut write_batch = WriteBatch::default();

    for h in 1..=num_blocks {
        let mut block_hash = [0u8; 32];
        block_hash[0..8].copy_from_slice(&h.to_be_bytes());
        block_hash[31] = 0xAA;

        let timestamp = 1_700_000_000u64 + h * 12;

        let mut block_val = Vec::with_capacity(40);
        block_val.extend_from_slice(&block_hash);
        block_val.extend_from_slice(&timestamp.to_be_bytes());
        write_batch.put_cf(&cf_block, h.to_be_bytes(), &block_val);
        write_batch.put_cf(&cf_hash_to_id, block_hash, h.to_be_bytes());

        for (acc_idx, acc) in accounts.iter().enumerate() {
            let tx_idx = (h - 1) * 2;
            let mut tx_hash_1 = [0u8; 32];
            tx_hash_1[0..8].copy_from_slice(&h.to_be_bytes());
            tx_hash_1[8..16].copy_from_slice(&(acc_idx as u64).to_be_bytes());
            tx_hash_1[30] = 0x01;

            let mut tx_hash_2 = tx_hash_1;
            tx_hash_2[30] = 0x02;

            write_batch.put_cf(&cf_tx_to_id, tx_hash_1, h.to_be_bytes());
            write_batch.put_cf(&cf_tx_to_id, tx_hash_2, h.to_be_bytes());

            let mut key_1 = Vec::with_capacity(40);
            key_1.extend_from_slice(acc);
            key_1.extend_from_slice(&tx_idx.to_be_bytes());
            write_batch.put_cf(&cf_acc_to_tx, &key_1, tx_hash_1);

            let mut key_2 = Vec::with_capacity(40);
            key_2.extend_from_slice(acc);
            key_2.extend_from_slice(&(tx_idx + 1).to_be_bytes());
            write_batch.put_cf(&cf_acc_to_tx, &key_2, tx_hash_2);

            write_batch.put_cf(&cf_acc_meta, acc, (tx_idx + 2).to_be_bytes());

            let nonce = tx_idx + 2;
            let balance = (acc_idx as u64 + 1) * 1000 + h * 10;
            let mut state_val = Vec::with_capacity(16);
            state_val.extend_from_slice(&nonce.to_be_bytes());
            state_val.extend_from_slice(&balance.to_be_bytes());
            write_batch.put_cf(&cf_state, acc, &state_val);

            let mut pinned_key = Vec::with_capacity(40);
            pinned_key.extend_from_slice(acc);
            pinned_key.extend_from_slice(&h.to_be_bytes());
            write_batch.put_cf(&cf_state, &pinned_key, &state_val);
        }

        let mut tip_val = Vec::with_capacity(48);
        tip_val.extend_from_slice(&h.to_be_bytes());
        tip_val.extend_from_slice(&block_hash);
        tip_val.extend_from_slice(&timestamp.to_be_bytes());
        write_batch.put_cf(&cf_meta, b"chain_tip", &tip_val);
    }

    store.db().write(write_batch).map_err(|_| OperationStatus::BackendFailureStorage)?;
    Ok(())
}
