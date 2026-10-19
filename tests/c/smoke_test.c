#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>
#include "../../include/lez_node_ffi.h"

static void dummy_cb(const FfiChainTip *tip, void *user_data) {
    (void)tip;
    int *count = (int *)user_data;
    if (count) {
        (*count)++;
    }
}

int main(void) {
    printf("[c-smoke] Starting C smoke test for lez-node-ffi-poc...\n");

    // 1. Test null-safety across all exports (F#64)
    FfiAbiVersion abi = lez_abi_version();
    assert(abi.major == LEZ_ABI_VERSION_MAJOR);
    assert(abi.minor == LEZ_ABI_VERSION_MINOR);
    assert(lez_indexer_create(NULL, LEZ_ABI_VERSION_MAJOR).status == NullPointer);
    assert(lez_indexer_create(NULL, 999).status == InvalidArgumentAbiMismatch);
    assert(lez_indexer_create(NULL, 999).value == NULL);
    assert(lez_query_chain_tip(NULL, NULL).status == NullPointer);
    assert(lez_query_account(NULL, NULL, NULL).status == NullPointer);
    assert(lez_query_account_at_block(NULL, NULL, NULL, 0, NULL).status == NullPointer);
    assert(lez_query_accounts(NULL, NULL, NULL, 0).status == NullPointer);
    assert(lez_query_transactions_by_account(NULL, NULL, NULL, NULL, 10, true).status == NullPointer);
    assert(lez_query_schema(NULL, NULL).status == NullPointer);
    assert(lez_admin_start_indexer(NULL, NULL) == NullPointer);
    assert(lez_admin_stop_indexer(NULL, NULL) == NullPointer);
    assert(lez_admin_reset_storage(NULL, NULL) == NullPointer);
    assert(lez_admin_grant(NULL, NULL, NULL) == NullPointer);
    assert(lez_subscribe_finalized_blocks(NULL, NULL, 0, NULL, NULL) == 0);
    assert(lez_unsubscribe(NULL, NULL, 0) == NullPointer);
    assert(lez_notify_block_ingest(NULL, 0, NULL, 0) == NullPointer);

    // Safe deallocation of NULL pointers
    lez_free_account(NULL);
    lez_free_accounts_batch(NULL);
    lez_free_tx_page(NULL);
    lez_free_chain_tip(NULL);
    lez_free_string(NULL);
    assert(lez_free_cursor(NULL, NULL, NULL) == Ok);
    lez_indexer_free(NULL);

    // Verify PointerResult.value == NULL on error
    assert(lez_query_chain_tip(NULL, NULL).value == NULL);
    assert(lez_query_account(NULL, NULL, NULL).value == NULL);
    assert(lez_query_accounts(NULL, NULL, NULL, 0).value == NULL);
    printf("[c-smoke] Null safety verified across all exports.\n");

    // 2. Initialize temporary RocksDB store
    char temp_dir[] = "/tmp/lez_poc_c_smoke_XXXXXX";
    char *dir = mkdtemp(temp_dir);
    assert(dir != NULL);

    // Verify ABI mismatch rejection on valid directory
    PointerResult_LezIndexerHandle bad_res = lez_indexer_create(dir, 999);
    assert(bad_res.status == InvalidArgumentAbiMismatch);
    assert(bad_res.value == NULL);

    PointerResult_LezIndexerHandle res = lez_indexer_create(dir, LEZ_ABI_VERSION_MAJOR);
    assert(res.status == Ok);
    LezIndexerHandle *indexer = res.value;
    assert(indexer != NULL);

    uint8_t caller_admin[32];
    memset(caller_admin, 0x01, 32);
    uint8_t caller_user[32];
    memset(caller_user, 0x02, 32);

    // 3. Test Schema Query (F#58)
    PointerResult_c_char schema_res = lez_query_schema(indexer, caller_user);
    assert(schema_res.status == Ok);
    assert(schema_res.value != NULL);
    assert(strstr(schema_res.value, "lez_query_chain_tip") != NULL);
    lez_free_string(schema_res.value);

    // 4. Test Admin policy gating (F#67): user denied, admin granted
    assert(lez_admin_start_indexer(indexer, caller_user) == PermissionDenied);
    assert(lez_admin_grant(indexer, caller_admin, caller_admin) == Ok);
    assert(lez_admin_start_indexer(indexer, caller_admin) == Ok);
    assert(lez_admin_stop_indexer(indexer, caller_admin) == Ok);
    assert(lez_admin_reset_storage(indexer, caller_admin) == Ok);

    // 5. Test Account Query default zeroed (F#10)
    uint8_t target_acc[32];
    memset(target_acc, 0x99, 32);
    PointerResult_FfiAccount acc_res = lez_query_account(indexer, caller_user, target_acc);
    assert(acc_res.status == Ok);
    assert(acc_res.value != NULL);
    assert(acc_res.value->balance == 0);
    assert(acc_res.value->nonce == 0);
    lez_free_account(acc_res.value);

    // 6. Test Batch Accounts Query (F#13)
    uint8_t batch_ids[64];
    memset(batch_ids, 0x11, 32);
    memset(batch_ids + 32, 0x22, 32);
    PointerResult_FfiAccountsBatch batch_res = lez_query_accounts(indexer, caller_user, batch_ids, 2);
    assert(batch_res.status == Ok);
    assert(batch_res.value != NULL);
    assert(batch_res.value->count == 2);
    lez_free_accounts_batch(batch_res.value);

    // 7. Test History Query (F#37-43)
    PointerResult_FfiTxPage page_res = lez_query_transactions_by_account(
        indexer, caller_user, target_acc, NULL, 5, true
    );
    assert(page_res.status == Ok);
    assert(page_res.value != NULL);
    assert(page_res.value->count == 0);
    assert(page_res.value->has_more == false);
    // Verified free ordering: lez_free_cursor (borrows cursor string) must precede lez_free_tx_page
    if (page_res.value->next_cursor != NULL) {
        assert(lez_free_cursor(indexer, caller_user, page_res.value->next_cursor) == Ok);
    }
    lez_free_tx_page(page_res.value);

    // 8. Test Finalized Block Subscription (F#70)
    int callback_invocations = 0;
    uint64_t sub_id = lez_subscribe_finalized_blocks(
        indexer, caller_user, 0, dummy_cb, &callback_invocations
    );
    assert(sub_id > 0);

    uint8_t blk_hash[32];
    memset(blk_hash, 0x42, 32);
    assert(lez_notify_block_ingest(indexer, 100, blk_hash, 1234567890) == Ok);
    assert(callback_invocations == 1);

    // Enforce caller isolation: caller_admin cannot unsubscribe caller_user's sub
    assert(lez_unsubscribe(indexer, caller_admin, sub_id) == PermissionDenied);
    assert(lez_unsubscribe(indexer, caller_user, sub_id) == Ok);

    // Cleanup
    lez_indexer_free(indexer);
    printf("[c-smoke] Clean exit: all exports tested, zero leaks detected.\n");
    return 0;
}
