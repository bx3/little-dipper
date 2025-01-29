use commonware_cryptography::{Ed25519, Scheme, PublicKey, Signature};
use commonware_consensus::Supervisor;
use crate::application::supervisor::Supervisor as SupervisorImpl;
use crate::APPLICATION_P2P_NAMESPACE;


/// A single mini block from a chatter
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct MiniBlock {
    pub view: u64,
    pub data: Vec<u8>,
    pub pubkey: Vec<u8>, // ed25519, not the bls threshold sig
    pub sig: Vec<u8>,
}

/// ProtoBlock is a collections of mini-blocks treated as the content for 
/// a consensus blok
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct ProtoBlock {
    pub mini_blocks: Vec<MiniBlock>,
}

impl MiniBlock {
    pub fn new(view: u64, data: Vec<u8>, pubkey: Vec<u8>) -> Self {
        Self {
            view: view,
            data: data,
            pubkey: pubkey,
            sig: vec![],
        }
    }

    pub fn non_sig_bytes(&self) -> Vec<u8> {
        let mut v: Vec<u8> = Vec::new();
        v.extend_from_slice(&self.view.to_be_bytes());
        v.extend_from_slice(&self.data);
        v.extend_from_slice(&self.pubkey);
        v
    }

    pub fn sign(&mut self, crypto: &mut Ed25519) {
        self.sig = crypto.sign(
            Some(APPLICATION_P2P_NAMESPACE),
            &self.non_sig_bytes(),
        ).into();
    }

    pub fn verify(&self) -> bool {
        Ed25519::verify(
            Some(APPLICATION_P2P_NAMESPACE),
            &self.non_sig_bytes(), 
            &PublicKey::copy_from_slice(&self.pubkey), 
            &Signature::copy_from_slice(&self.sig),
        )
    }

    pub fn is_participant(&self, view: u64, supervisor: &SupervisorImpl) -> bool {
        let pubkey = PublicKey::copy_from_slice(&self.pubkey);
        supervisor.is_participant(view, &pubkey).is_some()
    }
}
