#[cfg(feature = "secp256k1")]
pub mod ecdsa;
#[cfg(feature = "ed25519")]
pub mod eddsa;
mod error;

#[cfg(feature = "secp256k1")]
pub use ecdsa::{SigningKey, VerifyingKey};
#[cfg(feature = "ed25519")]
pub use eddsa::{SigningKey, VerifyingKey};
pub use error::SignatureError;

use serde::{Deserialize, Serialize};
use std::str::FromStr;

pub trait Signer: FromStr<Err = SignatureError> + Sized {
    fn from_slice(slice: &[u8]) -> Result<Self, SignatureError>;

    fn sign<T: Serialize>(&self, message: &T) -> Result<Signature, SignatureError>;
}

pub trait Verifier:
    FromStr<Err = SignatureError> + Sized + Deserialize<'static> + Serialize
{
    fn from_slice(slice: &[u8]) -> Result<Self, SignatureError>;

    fn verify<T: Serialize>(
        &self,
        message: &T,
        signature: &Signature,
    ) -> Result<bool, SignatureError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum SignatureScheme {
    Ed25519,
    Secp256k1,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Signature {
    pub bytes: Vec<u8>,
    pub scheme: SignatureScheme,
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use hex::FromHex;

//     /// use anvil 0 account for test in here: https://getfoundry.sh/anvil/overview/
//     /// address: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
//     /// private_key: 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80

//     #[test]
//     #[cfg(feature = "secp256k1")]
//     fn test_secp256k1_address_from_anvil_acc0_pk() {
//         let anvil_acc0_key = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
//         let signing_key = SigningKey::from_str(&anvil_acc0_key).unwrap();
//         let sk = secp256k1::SecretKey::from_slice(&anvil_acc0_key).unwrap();
//         let pub_key = secp256k1::PublicKey::from_secret_key(&secp, &sk);

//         let address = address_from_pubkey(&VerifyingKey::Secp256k1(pub_key)).unwrap();
//         let address = hex::encode(address);
//         assert_eq!(
//             address,
//             "f39Fd6e51aad88F6F4ce6aB8827279cffFb92266".to_lowercase()
//         );
//         print!("address expected  : \"f39Fd6e51aad88F6F4ce6aB8827279cffFb92266\"\naddress calculated: {:?}", address);
//     }

//     #[test]
//     #[cfg(feature = "secp256k1")]
//     fn test_secp256k1_sign_and_verify() {
//         let anvil_acc0_key: [u8; 32] = <[u8; 32]>::from_hex(
//             "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
//         )
//         .unwrap();
//         let secp = Secp256k1::new();
//         let sk = secp256k1::SecretKey::from_slice(&anvil_acc0_key).unwrap();
//         let signing_key =
//             SigningKey::from_bytes(SignatureScheme::Secp256k1, &anvil_acc0_key).unwrap();
//         let pub_key = secp256k1::PublicKey::from_secret_key(&secp, &sk);
//         let msg = b"Hello World";

//         let signature = sign(&signing_key, msg).unwrap();
//         let res = verify(&VerifyingKey::Secp256k1(pub_key), msg, &signature);
//         assert!(res.is_ok())
//     }

//     /// made key pair using `solana-keygen new --no-passphrase`
//     ///
//     /// [
//     ///   144,  45, 220,  66,  89, 201,   7, 239,
//     ///    86, 173, 155, 227,  31, 102,  64, 151,
//     ///   142, 184, 211, 146, 225, 143, 253, 224,
//     ///   165, 105, 222, 216,   4, 223,  35, 225,
//     ///
//     ///   104, 129, 238,  30, 109,  80,  35,  40,
//     ///   222, 122, 189, 203, 126, 168,  28, 216,
//     ///   229, 110, 167,  57, 192, 114, 219, 225,
//     ///   233, 104,   3,  71,   9, 159, 103, 127
//     /// ]
//     ///
//     /// first 32 bytes for secret key,
//     /// second 32 bytes for public key.

//     #[test]
//     #[cfg(feature = "ed25519")]
//     fn test_ed25519_get_public_key_from_private_key() {
//         let private_key: [u8; 32] = [
//             144, 45, 220, 66, 89, 201, 7, 239, 86, 173, 155, 227, 31, 102, 64, 151, 142, 184, 211,
//             146, 225, 143, 253, 224, 165, 105, 222, 216, 4, 223, 35, 225,
//         ];
//         let signing_key = SigningKey::from_bytes(&private_key).unwrap();
//         let public_key = match signing_key {
//             SigningKey::Ed25519(key) => key.verifying_key().to_bytes(),
//             _ => panic!("Invalid Signing key type"),
//         };

//         let expected_pub_key: String = hex::encode([
//             104, 129, 238, 30, 109, 80, 35, 40, 222, 122, 189, 203, 126, 168, 28, 216, 229, 110,
//             167, 57, 192, 114, 219, 225, 233, 104, 3, 71, 9, 159, 103, 127,
//         ]);

//         let calculated_pub_key = hex::encode(public_key);

//         print!(
//             "expected  : {:?}\ncalculated: {:?}",
//             expected_pub_key, calculated_pub_key
//         );
//         assert_eq!(expected_pub_key, calculated_pub_key);
//     }

//     #[test]
//     #[cfg(feature = "ed25519")]
//     fn test_ed25519_sign_and_verify() {
//         let private_key: [u8; 32] = [
//             144, 45, 220, 66, 89, 201, 7, 239, 86, 173, 155, 227, 31, 102, 64, 151, 142, 184, 211,
//             146, 225, 143, 253, 224, 165, 105, 222, 216, 4, 223, 35, 225,
//         ];
//         let signing_key = SigningKey::from_bytes(SignatureScheme::Ed25519, &private_key).unwrap();
//         let verifying_key = match &signing_key {
//             SigningKey::Ed25519(key) => key.verifying_key(),
//             _ => panic!("Invalid Signing key type"),
//         };
//         let msg = b"Hello World";

//         let signature = sign(&signing_key, msg).unwrap();
//         let res = verify(&VerifyingKey::Ed25519(verifying_key), msg, &signature);

//         assert!(res.is_ok())
//     }

//     // TODO: Add negative (failure) test cases as well
// }
