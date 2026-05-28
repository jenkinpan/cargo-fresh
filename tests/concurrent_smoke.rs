//! Shape-level test for the concurrent updater pattern.
//!
//! We don't run the real updater here (it would need globally installed
//! cargo packages and network). Instead we reproduce the exact pattern
//! `src/main.rs` uses — JoinSet + Semaphore + completion-out-of-order +
//! sort-to-input-order — and assert that:
//!
//! 1. With cap=1, total wall time is ~sum(per-task delay)        (serial)
//! 2. With cap=N, total wall time is ~max(per-task delay)         (parallel)
//! 3. Results come back in input order after sort, regardless of
//!    completion order.

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Semaphore;
use tokio::task::JoinSet;

async fn run_with_cap(cap: usize, delays_ms: &[u64]) -> (Duration, Vec<usize>) {
    let sem = Arc::new(Semaphore::new(cap));
    let mut set: JoinSet<(usize, usize)> = JoinSet::new();
    let start = Instant::now();

    for (i, &ms) in delays_ms.iter().enumerate() {
        let permit = sem.clone().acquire_owned().await.unwrap();
        set.spawn(async move {
            let _p = permit;
            tokio::time::sleep(Duration::from_millis(ms)).await;
            (i, i * 10) // stub "result"
        });
    }

    let mut indexed: Vec<(usize, usize)> = Vec::new();
    while let Some(joined) = set.join_next().await {
        indexed.push(joined.unwrap());
    }
    indexed.sort_by_key(|(i, _)| *i);
    let ordered: Vec<usize> = indexed.into_iter().map(|(_, r)| r).collect();

    (start.elapsed(), ordered)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cap_1_is_serial() {
    let delays = [80u64, 80, 80, 80];
    let (elapsed, results) = run_with_cap(1, &delays).await;
    assert_eq!(results, vec![0, 10, 20, 30], "results must be in input order");
    assert!(
        elapsed >= Duration::from_millis(300),
        "serial should be ~320ms, got {elapsed:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cap_4_is_parallel() {
    let delays = [80u64, 80, 80, 80];
    let (elapsed, results) = run_with_cap(4, &delays).await;
    assert_eq!(results, vec![0, 10, 20, 30], "results must be in input order");
    assert!(
        elapsed < Duration::from_millis(250),
        "parallel should be ~80ms, must be < 250ms, got {elapsed:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn uneven_delays_still_input_ordered() {
    // Task 0 takes longest — without the sort, completion order would be 3,2,1,0.
    let delays = [200u64, 100, 60, 30];
    let (_elapsed, results) = run_with_cap(4, &delays).await;
    assert_eq!(
        results,
        vec![0, 10, 20, 30],
        "sort_by_key must restore input order even with reverse completion order"
    );
}
