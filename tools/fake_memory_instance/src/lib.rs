// Where: tools/fake_memory_instance/src/lib.rs
// What: Test-only fake memory instance canister for PocketIC integration tests.
// Why: Provide deterministic `search` responses without relying on real KINIC data or external services.
use std::cell::RefCell;

use ic_cdk::{init, query};

thread_local! {
    static RESULTS: RefCell<Vec<(f32, String)>> = const { RefCell::new(Vec::new()) };
}

#[init]
fn init(results: Vec<(f32, String)>) {
    RESULTS.with(|state| {
        *state.borrow_mut() = results;
    });
}

#[query]
fn search(_embedding: Vec<f32>) -> Vec<(f32, String)> {
    RESULTS.with(|state| state.borrow().clone())
}
