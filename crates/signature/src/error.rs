#[derive(Debug, thiserror::Error)]
pub enum SignatureError {
    #[error("Invalid key length. Expected:{expected}, Got:{actual}")]
    InvalidKeyLength { expected: usize, actual: usize },
    #[error("Invalid signature length")]
    InvalidSignatureLength,
    #[error("Signature verification failed")]
    VerificationFailed,
    #[error("Signature scheme does not match")]
    SchemeDoesNotMatch,
    #[error("Fail to decode hex string: {0}")]
    HexDecodeError(#[from] hex::FromHexError),
    #[error("Ed25519 library error: {0}")]
    Ed25519LibError(#[from] ed25519_dalek::SignatureError),
    #[error("Secp256k1 library error: {0}")]
    Secp256k1LibError(#[from] secp256k1::Error),
}
