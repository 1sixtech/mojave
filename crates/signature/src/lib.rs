#[cfg(feature = "ed25519")]
use ed25519::Verifier;
#[cfg(feature = "ed25519")]
use ed25519_dalek as ed25519;
#[cfg(feature = "secp256k1")]
use secp256k1::{ecdsa::Signature as SecpSig, Message, Secp256k1};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::LazyLock;
use tiny_keccak::{Hasher, Keccak};

pub mod error;
pub use error::SignatureError;

static SECP256K1_SIGNING: LazyLock<Secp256k1<secp256k1::SignOnly>> =
    LazyLock::new(|| Secp256k1::signing_only());
static SECP256K1_VERIFY: LazyLock<Secp256k1<secp256k1::VerifyOnly>> =
    LazyLock::new(|| Secp256k1::verification_only());

#[derive(PartialEq, Serialize, Deserialize)]
pub enum SignatureScheme {
    #[cfg(feature = "ed25519")]
    Ed25519,
    #[cfg(feature = "secp256k1")]
    Secp256k1,
}

#[derive(Clone, Debug)]
pub enum SigningKey {
    #[cfg(feature = "ed25519")]
    Ed25519(ed25519::SigningKey),
    #[cfg(feature = "secp256k1")]
    Secp256k1(secp256k1::SecretKey),
}

#[derive(Serialize, Deserialize)]
pub enum VerifyingKey {
    #[cfg(feature = "ed25519")]
    Ed25519(ed25519::VerifyingKey),
    #[cfg(feature = "secp256k1")]
    Secp256k1(secp256k1::PublicKey),
}

#[derive(Serialize, Deserialize)]
pub struct Signature {
    pub bytes: Vec<u8>,
    pub scheme: SignatureScheme,
}

impl Signature {
    pub const BYTE_SIZE: usize = 64;

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

pub fn default_scheme() -> SignatureScheme {
    #[cfg(all(feature = "secp256k1", not(feature = "ed25519")))]
    {
        SignatureScheme::Secp256k1
    }
    #[cfg(all(feature = "ed25519", not(feature = "secp256k1")))]
    {
        SignatureScheme::Ed25519
    }
    #[cfg(all(feature = "secp256k1", feature = "ed25519"))]
    {
        SignatureScheme::Secp256k1
    }
    #[cfg(not(any(feature = "secp256k1", feature = "ed25519")))]
    {
        compile_error!("At least one signature scheme feature must be enabled");
    }
}

impl SigningKey {
    pub fn from_bytes(scheme: SignatureScheme, bytes: &[u8]) -> Result<Self, SignatureError> {
        match scheme {
            #[cfg(feature = "ed25519")]
            SignatureScheme::Ed25519 => {
                let secret_key: &[u8; ed25519::SECRET_KEY_LENGTH] =
                    bytes
                        .try_into()
                        .map_err(|_| SignatureError::InvalidKeyLength {
                            expected: ed25519::SECRET_KEY_LENGTH,
                            actual: bytes.len(),
                        })?;
                let signing_key = ed25519::SigningKey::from_bytes(secret_key);
                Ok(SigningKey::Ed25519(signing_key))
            }
            #[cfg(feature = "secp256k1")]
            SignatureScheme::Secp256k1 => {
                let secret_key = secp256k1::SecretKey::from_slice(bytes)?;
                Ok(Self::Secp256k1(secret_key))
            }
        }
    }

    pub fn from_bytes_default(bytes: &[u8]) -> Result<Self, SignatureError> {
        Self::from_bytes(default_scheme(), bytes)
    }
}

impl VerifyingKey {
    pub fn from_bytes(scheme: SignatureScheme, bytes: &[u8]) -> Result<Self, SignatureError> {
        match scheme {
            #[cfg(feature = "ed25519")]
            SignatureScheme::Ed25519 => {
                let pub_key_arr: &[u8; ed25519::PUBLIC_KEY_LENGTH] =
                    bytes
                        .try_into()
                        .map_err(|_| SignatureError::InvalidKeyLength {
                            expected: ed25519::PUBLIC_KEY_LENGTH,
                            actual: bytes.len(),
                        })?;
                let verifying_key = ed25519::VerifyingKey::from_bytes(pub_key_arr)?;
                Ok(VerifyingKey::Ed25519(verifying_key))
            }
            #[cfg(feature = "secp256k1")]
            SignatureScheme::Secp256k1 => {
                let pub_key = secp256k1::PublicKey::from_slice(bytes)?;
                Ok(VerifyingKey::Secp256k1(pub_key))
            }
        }
    }

    pub fn from_signing_key(signing_key: &SigningKey) -> Result<Self, SignatureError> {
        match &signing_key {
            #[cfg(feature = "ed25519")]
            SigningKey::Ed25519(key) => Ok(VerifyingKey::Ed25519(key.verifying_key())),
            #[cfg(feature = "secp256k1")]
            SigningKey::Secp256k1(key) => {
                let pub_key = secp256k1::PublicKey::from_secret_key(&SECP256K1_SIGNING, key);
                Ok(VerifyingKey::Secp256k1(pub_key))
            }
        }
    }

    pub fn from_bytes_default(bytes: &[u8]) -> Result<Self, SignatureError> {
        Self::from_bytes(default_scheme(), bytes)
    }
}

pub fn sign(signing_key: &SigningKey, msg: &[u8]) -> Result<Signature, SignatureError> {
    match signing_key {
        #[cfg(feature = "ed25519")]
        SigningKey::Ed25519(key) => {
            use ed25519::Signer;
            let signature = key.sign(msg);
            Ok(Signature {
                bytes: signature.to_bytes().to_vec(),
                scheme: SignatureScheme::Ed25519,
            })
        }

        #[cfg(feature = "secp256k1")]
        SigningKey::Secp256k1(key) => {
            let msg_hash = Sha256::digest(msg);
            let message = Message::from_digest_slice(msg_hash.as_slice())?;
            let secp256k1 = &SECP256K1_SIGNING;
            let signature = secp256k1.sign_ecdsa(&message, &key).serialize_compact();
            Ok(Signature {
                bytes: signature.to_vec(),
                scheme: SignatureScheme::Secp256k1,
            })
        }
    }
}

pub fn verify(
    verifying_key: &VerifyingKey,
    msg: &[u8],
    signature: &Signature,
) -> Result<(), SignatureError> {
    match (verifying_key, &signature.scheme) {
        #[cfg(feature = "ed25519")]
        (VerifyingKey::Ed25519(key), SignatureScheme::Ed25519) => {
            let sig_bytes: [u8; ed25519::SIGNATURE_LENGTH] = signature
                .bytes
                .as_slice()
                .try_into()
                .map_err(|_| SignatureError::InvalidSignatureLength)?;

            let sig = ed25519::Signature::from_bytes(&sig_bytes);
            key.verify(msg, &sig)?;
            Ok(())
        }
        #[cfg(feature = "secp256k1")]
        (VerifyingKey::Secp256k1(key), SignatureScheme::Secp256k1) => {
            let secp = &SECP256K1_VERIFY;
            let digest = Sha256::digest(msg);
            let msg = Message::from_digest_slice(&digest)?;
            let sig = SecpSig::from_compact(&signature.bytes)?;
            secp.verify_ecdsa(&msg, &sig, key)?;
            Ok(())
        }
        _ => Err(SignatureError::SchemeDoesNotMatch),
    }
}

pub fn address_from_pubkey(verifying_key: &VerifyingKey) -> Result<Vec<u8>, SignatureError> {
    match verifying_key {
        #[cfg(feature = "ed25519")]
        VerifyingKey::Ed25519(key) => Ok(key.to_bytes().to_vec()),
        #[cfg(feature = "secp256k1")]
        VerifyingKey::Secp256k1(key) => {
            let public_key = key.serialize_uncompressed();

            let mut hasher = Keccak::v256();
            // Remove the 0x04 prefix byte from uncompressed public key
            hasher.update(&public_key[1..]);
            let mut hash = [0u8; 32];
            hasher.finalize(&mut hash);

            let mut address = [0u8; 20];
            address.copy_from_slice(&hash[12..]);

            Ok(address.to_vec())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hex::FromHex;

    /// use anvil 0 account for test in here: https://getfoundry.sh/anvil/overview/
    /// address: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
    /// private_key: 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80

    #[test]
    #[cfg(feature = "secp256k1")]
    fn test_secp256k1_address_from_anvil_acc0_pk() {
        let anvil_acc0_key: [u8; 32] = <[u8; 32]>::from_hex(
            "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
        )
        .unwrap();
        let secp = Secp256k1::new();
        let sk = secp256k1::SecretKey::from_slice(&anvil_acc0_key).unwrap();
        let pub_key = secp256k1::PublicKey::from_secret_key(&secp, &sk);

        let address = address_from_pubkey(&VerifyingKey::Secp256k1(pub_key)).unwrap();
        let address = hex::encode(address);
        assert_eq!(
            address,
            "f39Fd6e51aad88F6F4ce6aB8827279cffFb92266".to_lowercase()
        );
        print!("address expected  : \"f39Fd6e51aad88F6F4ce6aB8827279cffFb92266\"\naddress calculated: {:?}", address);
    }

    #[test]
    #[cfg(feature = "secp256k1")]
    fn test_secp256k1_sign_and_verify() {
        let anvil_acc0_key: [u8; 32] = <[u8; 32]>::from_hex(
            "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
        )
        .unwrap();
        let secp = Secp256k1::new();
        let sk = secp256k1::SecretKey::from_slice(&anvil_acc0_key).unwrap();
        let signing_key =
            SigningKey::from_bytes(SignatureScheme::Secp256k1, &anvil_acc0_key).unwrap();
        let pub_key = secp256k1::PublicKey::from_secret_key(&secp, &sk);
        let msg = b"Hello World";

        let signature = sign(&signing_key, msg).unwrap();
        let res = verify(&VerifyingKey::Secp256k1(pub_key), msg, &signature);
        assert!(res.is_ok())
    }

    /// made key pair using `solana-keygen new --no-passphrase`
    ///
    /// [
    ///   144,  45, 220,  66,  89, 201,   7, 239,
    ///    86, 173, 155, 227,  31, 102,  64, 151,
    ///   142, 184, 211, 146, 225, 143, 253, 224,
    ///   165, 105, 222, 216,   4, 223,  35, 225,
    ///
    ///   104, 129, 238,  30, 109,  80,  35,  40,
    ///   222, 122, 189, 203, 126, 168,  28, 216,
    ///   229, 110, 167,  57, 192, 114, 219, 225,
    ///   233, 104,   3,  71,   9, 159, 103, 127
    /// ]
    ///
    /// first 32 bytes for secret key,
    /// second 32 bytes for public key.

    #[test]
    #[cfg(feature = "ed25519")]
    fn test_ed25519_get_public_key_from_private_key() {
        let private_key: [u8; 32] = [
            144, 45, 220, 66, 89, 201, 7, 239, 86, 173, 155, 227, 31, 102, 64, 151, 142, 184, 211,
            146, 225, 143, 253, 224, 165, 105, 222, 216, 4, 223, 35, 225,
        ];
        let signing_key = SigningKey::from_bytes(SignatureScheme::Ed25519, &private_key).unwrap();
        let public_key = match signing_key {
            SigningKey::Ed25519(key) => key.verifying_key().to_bytes(),
            _ => panic!("Invalid Signing key type"),
        };

        let expected_pub_key: String = hex::encode([
            104, 129, 238, 30, 109, 80, 35, 40, 222, 122, 189, 203, 126, 168, 28, 216, 229, 110,
            167, 57, 192, 114, 219, 225, 233, 104, 3, 71, 9, 159, 103, 127,
        ]);

        let calculated_pub_key = hex::encode(public_key);

        print!(
            "expected  : {:?}\ncalculated: {:?}",
            expected_pub_key, calculated_pub_key
        );
        assert_eq!(expected_pub_key, calculated_pub_key);
    }

    #[test]
    #[cfg(feature = "ed25519")]
    fn test_ed25519_sign_and_verify() {
        let private_key: [u8; 32] = [
            144, 45, 220, 66, 89, 201, 7, 239, 86, 173, 155, 227, 31, 102, 64, 151, 142, 184, 211,
            146, 225, 143, 253, 224, 165, 105, 222, 216, 4, 223, 35, 225,
        ];
        let signing_key = SigningKey::from_bytes(SignatureScheme::Ed25519, &private_key).unwrap();
        let verifying_key = match &signing_key {
            SigningKey::Ed25519(key) => key.verifying_key(),
            _ => panic!("Invalid Signing key type"),
        };
        let msg = b"Hello World";

        let signature = sign(&signing_key, msg).unwrap();
        let res = verify(&VerifyingKey::Ed25519(verifying_key), msg, &signature);

        assert!(res.is_ok())
    }

    // TODO: Add negative (failure) test cases as well
}
