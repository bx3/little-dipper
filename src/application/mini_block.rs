use commonware_cryptography::{Digest, Ed25519, Hasher, Scheme, PublicKey, Signature};
use commonware_cryptography::{
    bls12381::primitives::{
        group::{self, Element},
        poly::{self, Poly, PartialSignature},
        ops,
    },
    bls12381,
};
use bytes::Bytes;
use commonware_consensus::{Supervisor, ThresholdSupervisor};

use crate::application::supervisor::Supervisor as SupervisorImpl;


/// A single mini block from a chatter
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct MiniBlock {
    pub view: u64,
    pub data: Vec<u8>,
    pub pubkey: Vec<u8>, // ed25519, not the bls threshold sig
    pub sig: Vec<u8>,
}

/// Data ready to become a proposal
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct ProtoBlock {
    pub mini_blocks: Vec<MiniBlock>,
}

impl MiniBlock {
    pub fn non_sig_bytes(&self) -> Vec<u8> {
        let mut v: Vec<u8> = Vec::new();
        v.extend_from_slice(&self.view.to_be_bytes());
        v.extend_from_slice(&self.data);
        v.extend_from_slice(&self.pubkey);
        v
    }

    pub fn sign(&self, crypto: &mut Ed25519) -> Vec<u8> {
        let b = self.non_sig_bytes();
        let sig = crypto.sign(None, &b);
        sig.into()
    }

    pub fn verify(&self) -> bool {
        let p = PublicKey::copy_from_slice(&self.pubkey);
        let b = self.non_sig_bytes();
        let s = Signature::copy_from_slice(&self.sig);

        Ed25519::verify(None, &b, &p, &s)
    }

    pub fn is_participant(&self, view: u64, supervisor: &SupervisorImpl) -> bool {
        let pubkey = PublicKey::copy_from_slice(&self.pubkey);
        
        if let Some(_) = supervisor.is_participant(view, &pubkey) {
            return true
        }
        return false
    }
}
