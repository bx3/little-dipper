use commonware_cryptography::{Digest, Hasher, Scheme};
use commonware_cryptography::{
    bls12381::primitives::{
        group::{self, Element},
        poly::{self, Poly, PartialSignature},
        ops,
    },
    PublicKey,
    bls12381,
};
use bytes::Bytes;

/// A single mini block from a chatter
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct MiniBlock {
    pub view: u64,
    pub data: Vec<u8>,
    pub sig: Vec<u8>,
}

/// Data ready to become a proposal
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct MiniBlocks {
    pub mini_blocks: Vec<MiniBlock>,
}

impl MiniBlock {
    pub fn non_sig_bytes(&self) -> Vec<u8> {
        let mut v: Vec<u8> = Vec::new();
        v.extend_from_slice(&self.view.to_be_bytes());
        v.extend_from_slice(&self.data);
        v
    }

    pub fn sign(&self, share: &group::Share) -> Vec<u8> {
        let b = self.non_sig_bytes();
        let sig = ops::sign_message(&share.private, None, &b);
        sig.serialize()
    }

    pub fn verify(&self, pubkey: &Bytes) -> bool {
        let p = group::Public::deserialize(pubkey).unwrap();
        let b = self.non_sig_bytes();
        let s = group::Signature::deserialize(&self.sig).unwrap();
        if ops::verify_message(
            &p,
            None,
            &b,
            &s,
        )
        .is_err() {
            return false
        }
        true
    }
}
