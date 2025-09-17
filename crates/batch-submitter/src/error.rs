pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Batch notifier error: {0}")]
    BatchNotifierError(#[from] tokio::sync::mpsc::error::TrySendError<u64>),
    #[error("Bitcoin io error: {0}")]
    BitcoinIoError(#[from] bitcoin::io::Error),
    #[error("Bitcoin RPC error: {0}")]
    BitcoinRPCError(#[from] bitcoincore_rpc::Error),
    #[error("Encode error: {0}")]
    EncodeError(#[from] bitcoin::consensus::encode::Error),
    #[error("Hex to array error: {0}")]
    HexToArrayError(#[from] bitcoin::hex::HexToArrayError),
    #[error("Internal Error: {0}")]
    Internal(String),
    #[error("secp256k1 error: {0}")]
    Secp256k1Error(#[from] bitcoin::secp256k1::Error),
    #[error("Sighash taproot error: {0}")]
    SighashTaprootError(#[from] bitcoin::sighash::TaprootError),
    #[error("Error building taproot")]
    TaprootError(#[from] bitcoin::taproot::TaprootBuilderError),
}
