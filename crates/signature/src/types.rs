use crate::error::Error;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

pub trait Signer: FromStr<Err = Error> + Sized {
    fn from_slice(slice: &[u8]) -> Result<Self, Error>;

    fn sign<T: Serialize>(&self, message: &T) -> Result<Signature, Error>;
}

pub trait Verifier: FromStr<Err = Error> + Sized + Deserialize<'static> + Serialize {
    fn from_slice(slice: &[u8]) -> Result<Self, Error>;

    fn verify<T: Serialize>(&self, message: &T, signature: &Signature) -> Result<(), Error>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum SignatureScheme {
    Ed25519,
    Secp256k1,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Signature {
    pub bytes: Vec<u8>,
    pub scheme: SignatureScheme,
}
