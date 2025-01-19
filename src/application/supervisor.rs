use commonware_consensus::{
    simplex::View, Activity, Proof, Supervisor as Su,
};
use commonware_cryptography::{
    bls12381::primitives::{
        group::{self, Element},
        poly::{self, Poly},
    },
    PublicKey,
};
use commonware_utils::modulo;
use std::collections::HashMap;

/// Implementation of `commonware-consensus::Supervisor`.
#[derive(Clone)]
pub struct Supervisor {
    identity: Poly<group::Public>,
    participants: Vec<PublicKey>,
    participants_map: HashMap<PublicKey, u32>,
}

impl Supervisor {
    pub fn new(
        identity: Poly<group::Public>,
        mut participants: Vec<PublicKey>,
    ) -> Self {
        // Setup participants
        participants.sort();
        let mut participants_map = HashMap::new();
        for (index, validator) in participants.iter().enumerate() {
            participants_map.insert(validator.clone(), index as u32);
        }

        // Return supervisor
        Self {
            identity,
            participants,
            participants_map,
        }
    }
}

impl Su for Supervisor {
    type Index = View;

    fn leader(&self, i: Self::Index) -> Option<PublicKey> {
        Some(self.participants[i as usize % self.participants.len()].clone())
    }

    fn participants(&self, _: Self::Index) -> Option<&Vec<PublicKey>> {
        Some(&self.participants)
    }

    fn is_participant(&self, _: Self::Index, candidate: &PublicKey) -> Option<u32> {
        self.participants_map.get(candidate).cloned()
    }

    async fn report(&self, _: Activity, _: Proof) {
        // We don't report activity in this example but you would otherwise use
        // this to collect uptime and fraud proofs.
    }
}
