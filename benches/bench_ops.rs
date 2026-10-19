use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use lez_node_ffi_poc::indexer::LezIndexer;
use lez_node_ffi_poc::storage::fixture::load_deterministic_fixtures;
use tempfile::tempdir;

fn bench_accounts(c: &mut Criterion) {
    let dir = tempdir().unwrap();
    let indexer = LezIndexer::open(dir.path()).unwrap();

    let mut accounts = Vec::new();
    for i in 0..100 {
        let mut acc = [0u8; 32];
        acc[0..8].copy_from_slice(&(i as u64).to_be_bytes());
        accounts.push(acc);
    }
    load_deterministic_fixtures(&indexer.store, 50, &accounts).unwrap();

    let mut group = c.benchmark_group("account_reads");
    for size in [1, 10, 100] {
        let subset: Vec<[u8; 32]> = accounts[0..size].to_vec();

        group.bench_with_input(BenchmarkId::new("single_reads", size), &subset, |b, ids| {
            b.iter(|| {
                for id in ids {
                    let _ = indexer.query_account(id).unwrap();
                }
            });
        });

        group.bench_with_input(
            BenchmarkId::new("batch_multi_get", size),
            &subset,
            |b, ids| {
                b.iter(|| {
                    let _ = indexer.query_accounts(ids).unwrap();
                });
            },
        );
    }
    group.finish();
}

fn bench_pagination(c: &mut Criterion) {
    let mut group = c.benchmark_group("pagination_page");
    for blocks in [50, 200] {
        let dir = tempdir().unwrap();
        let indexer = LezIndexer::open(dir.path()).unwrap();

        let mut acc = [0u8; 32];
        acc[0] = 0xAA;
        load_deterministic_fixtures(&indexer.store, blocks, &[acc]).unwrap();

        let caller = [1u8; 32];
        group.bench_with_input(BenchmarkId::new("page_size_10", blocks), &blocks, |b, _| {
            b.iter(|| {
                let _ = indexer
                    .query_transactions_by_account(&caller, &acc, None, 10, true)
                    .unwrap();
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_accounts, bench_pagination);
criterion_main!(benches);
