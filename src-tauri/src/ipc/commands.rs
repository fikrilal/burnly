use serde::Serialize;

use super::response::{IpcResponse, CONTRACT_VERSION};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ContractProbeResponse {
    status: &'static str,
    contract_version: u16,
}

#[tauri::command]
pub(super) fn __burnly_contract_probe() -> IpcResponse<ContractProbeResponse> {
    IpcResponse::success(ContractProbeResponse {
        status: "ok",
        contract_version: CONTRACT_VERSION,
    })
}
