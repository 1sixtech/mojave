use ethrex_rpc::RpcErr;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Rpc(#[from] RpcErr),
    #[error(transparent)]
    MojaveClient(#[from] mojave_client::MojaveClientError),
}
