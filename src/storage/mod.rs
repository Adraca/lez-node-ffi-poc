pub mod cursors;
pub mod fixture;
pub mod subscriptions;

use crate::core::status::OperationStatus;
use rocksdb::{ColumnFamilyDescriptor, DBWithThreadMode, MultiThreaded, Options};
use std::path::Path;
use std::sync::Arc;

pub const CF_BLOCK: &str = "cf_block";
pub const CF_META: &str = "cf_meta";
pub const CF_HASH_TO_ID: &str = "cf_hash_to_id";
pub const CF_TX_TO_ID: &str = "cf_tx_to_id";
pub const CF_ACC_META: &str = "cf_acc_meta";
pub const CF_ACC_TO_TX: &str = "cf_acc_to_tx";
pub const CF_STATE: &str = "cf_state";

pub struct RocksStore {
    db: Arc<DBWithThreadMode<MultiThreaded>>,
}

impl RocksStore {
    pub fn open(path: &Path) -> Result<Self, OperationStatus> {
        let mut cf_opts = Options::default();
        cf_opts.create_if_missing(true);

        let descriptors = vec![
            ColumnFamilyDescriptor::new("default", cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_BLOCK, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_META, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_HASH_TO_ID, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_TX_TO_ID, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_ACC_META, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_ACC_TO_TX, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_STATE, cf_opts),
        ];

        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);

        let db =
            DBWithThreadMode::<MultiThreaded>::open_cf_descriptors(&db_opts, path, descriptors)
                .map_err(|_| OperationStatus::BackendFailureStorage)?;

        Ok(Self { db: Arc::new(db) })
    }

    #[inline]
    pub fn db(&self) -> &DBWithThreadMode<MultiThreaded> {
        &self.db
    }

    pub fn cf(&self, name: &str) -> Result<Arc<rocksdb::BoundColumnFamily<'_>>, OperationStatus> {
        self.db
            .cf_handle(name)
            .ok_or(OperationStatus::BackendFailureStorage)
    }
}
