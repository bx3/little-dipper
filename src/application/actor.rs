use crate::application::mini_block::ProtoBlock;

use super::{
    ingress::{Mailbox, Message},
    supervisor::Supervisor,
    Config,
};

use super::chatter::ingress::Mailbox as ChatterMailbox;

use commonware_consensus::threshold_simplex::Prover;
use commonware_cryptography::{
    bls12381::primitives::{group::Element, poly},
    Hasher,
};

use futures::{channel::mpsc, StreamExt};

use rand::Rng;
use tracing::info;

/// Genesis message to use during initialization.
const GENESIS: &[u8] = b"commonware is neat";

/// Application actor.
pub struct Application<R: Rng, H: Hasher> {
    runtime: R,
    prover: Prover<H>,
    public: Vec<u8>,
    hasher: H,
    mailbox: mpsc::Receiver<Message>,
    chatter_mailbox: ChatterMailbox,
}

impl<R: Rng, H: Hasher> Application<R, H> {
    /// Create a new application actor.
    pub fn new(runtime: R, config: Config<H>, chatter_mailbox: ChatterMailbox) -> (Self, Supervisor, Mailbox) {
        let (sender, mailbox) = mpsc::channel(config.mailbox_size);
        (
            Self {
                runtime,
                prover: config.prover,
                public: poly::public(&config.identity).serialize(),
                hasher: config.hasher,
                mailbox,
                chatter_mailbox: chatter_mailbox,
            },
            Supervisor::new(config.identity, config.participants, config.share),
            Mailbox::new(sender),
        )
    }

    /// Run the application actor.
    pub async fn run(mut self) {
        while let Some(message) = self.mailbox.next().await {
            match message {
                Message::Genesis { response } => {
                    // Use the digest of the genesis message as the initial
                    // payload.
                    self.hasher.update(GENESIS);
                    let digest = self.hasher.finalize();
                    let _ = response.send(digest);
                }
                Message::Propose { index, response } => {
                    // Generate a random message
                    // bytes has to be power of 2, because consensus assume it has hash

                    // TODO use chatter_mailbox to request data
                    let chatter_response = self.chatter_mailbox.get_proto_block(index).await;
                    
                    match chatter_response.await {
                        Ok(proto_block) => {
                            info!("application with sufficient mini blocksx");
                            // TODO use more efficient format
                            let proto_block_json = serde_json::to_vec(&proto_block).unwrap();

                            let mut msg: Vec<u8> = vec![0; 32];
                            self.runtime.fill(&mut msg[1..]);

                            // TODO this is super hacky. The voter.propose from consensus expect an digest.
                            // Right now it is using Bytes, so we actually can stuff anydata into it.
                            // But it is very inefficient since, Proposal is included in all notarization and
                            // finalization. Need a separate channel to send the actual block
                            let _ = response.send(proto_block_json.into());
                        },
                        Err(e) => info!("insuficient miniblock {:?}", e),
                    }
                }
                Message::Verify { index, payload, response } => {
                    // Ensure payload is a valid digest
                    let view = index;
                    info!("validator sent miniblock while verify the data");
                    let chatter_response = self.chatter_mailbox.send_mini_block(view).await;
                    // TODO can probably remove the need to wait for sent
                    match chatter_response.await {
                        Ok(_) => info!("chatter response ok"),
                        Err(e) => info!("errr {:?}", e),
                    }
                
                    let proto_block: ProtoBlock = serde_json::from_slice(&payload).unwrap();
                    
                    // check mini_blocks comes from unique particiants and verify against their sigs
                    let chatter_response = self.chatter_mailbox.check_sufficient_mini_blocks(view, proto_block).await;

                    // TODO can probably remove the need to wait for sent
                    let result = match chatter_response.await {
                        Ok(r) => r,
                        Err(e) => {
                            info!("verify insufficient mini-blocks errr {:?}", e);
                            false
                        },
                    };
                    info!("verify sufficient mini-blocks result {:?}", result);

                    // TODO always correct for now
                    let _ = response.send(result);
                }
                Message::Nullify { index } => {
                    // When there is some gap in the state transition,
                    // either because GST or a malicious leader
                    // We let the chatter to send its mini-block to the next leader
                    // so it is ready to propose when ready
                    let view = index;
                    info!("Nullfy took place received by application validator");
                    // sed the current view, the +1 is performed inside the chatter
                    let chatter_response = self.chatter_mailbox.send_mini_block(view).await;
                    // TODO can probably remove the need to wait for sent
                    match chatter_response.await {
                        Ok(_) => info!("chatter response ok"),
                        Err(e) => info!("errr {:?}", e),
                    }
                }
                Message::Prepared { proof, payload } => {
                    // TODO remove restriction for threshold consensus such that payload has to be size of 32 bytes
                    // the notarizatio inside the consensus assume verification from the another thresh
                    /*
                    let (view, _, _, signature, seed) =
                        self.prover.deserialize_notarization(proof).unwrap();
                    let signature = signature.serialize();
                    let seed = seed.serialize();
                    info!(
                        view,
                        payload = hex(&payload),
                        signature = hex(&signature),
                        seed = hex(&seed),
                        "prepared"
                    )
                    */
                    info!("prepared");
                }
                Message::Finalized { proof, payload } => {
                    // TODO remove unnecessary parts for threshold consensus

                    /*
                    let (view, _, _, signature, seed) =
                        self.prover.deserialize_finalization(proof.clone()).unwrap();
                    let signature = signature.serialize();
                    let seed = seed.serialize();
                    info!(
                        view,
                        payload = hex(&payload),
                        signature = hex(&signature),
                        seed = hex(&seed),
                        "finalized"
                    );
                     */
                    info!("finalized");
                }
            }
        }
    }
}
