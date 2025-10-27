use tiny_keccak::{Hasher, Keccak};

pub fn compute_keccak(bytes: &[u8]) -> String {
    let mut hasher = Keccak::v256();
    hasher.update(bytes);
    let mut out = [0u8; 32];
    hasher.finalize(&mut out);
    hex::encode(out)
}
