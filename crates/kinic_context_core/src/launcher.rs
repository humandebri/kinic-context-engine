// Where: crates/kinic_context_core/src/launcher.rs
// What: Read-only launcher canister calls using the existing service.did interface.
// Why: Validate source backing instances against the existing launcher without introducing new backend types.
use anyhow::Result;
use candid::{CandidType, Deserialize};

use crate::client::QueryClient;

const LIST_INSTANCE_METHOD: &str = "list_instance";

#[derive(Clone, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub enum LauncherState {
    Empty(String),
    Pending(String),
    Creation(String),
    Installation((candid::Principal, String)),
    SettingUp(candid::Principal),
    Running(candid::Principal),
}

pub async fn list_instances(
    client: &QueryClient,
    launcher_canister_id: &str,
) -> Result<Vec<LauncherState>> {
    client
        .query_args::<Vec<LauncherState>>(launcher_canister_id, LIST_INSTANCE_METHOD, ())
        .await
}
