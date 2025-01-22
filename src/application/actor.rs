use crate::wire;

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
use commonware_runtime::{Sink, Stream};
use commonware_stream::{public_key::Connection, Receiver, Sender};
use commonware_utils::hex;
use futures::{channel::mpsc, StreamExt};
use prost::Message as _;
use rand::Rng;
use tracing::{debug, info};

/// Genesis message to use during initialization.
const GENESIS: &[u8] = b"commonware is neat";

/// Application actor.
pub struct Application<R: Rng, H: Hasher, Si: Sink, St: Stream> {
    runtime: R,
    indexer: Connection<Si, St>,
    prover: Prover<H>,
    public: Vec<u8>,
    hasher: H,
    mailbox: mpsc::Receiver<Message>,
    chatter_mailbox: ChatterMailbox,
}

impl<R: Rng, H: Hasher, Si: Sink, St: Stream> Application<R, H, Si, St> {
    /// Create a new application actor.
    pub fn new(runtime: R, config: Config<H, Si, St>, chatter_mailbox: ChatterMailbox) -> (Self, Supervisor, Mailbox) {
        let (sender, mailbox) = mpsc::channel(config.mailbox_size);
        (
            Self {
                runtime,
                indexer: config.indexer,
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
        let (mut indexer_sender, mut indexer_receiver) = self.indexer.split();
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
                    //let mut msg: Vec<u8> = vec![0; 32];
                    //self.runtime.fill(&mut msg[1..]);
                    //msg[1..9].copy_from_slice(&index.to_be_bytes());

                    // TODO use chatter_mailbox to request data
                    let chatter_response = self.chatter_mailbox.get_mini_blocks(index).await;
                    let mini_blocks = chatter_response.await.unwrap();
                    let mini_block = mini_blocks[0].clone();

                    // TODO hack use the first mini-blocks' data to populate

                    let _ = response.send(mini_block.data.into());
                }
                Message::Verify { payload, response } => {
                    // Ensure payload is a valid digest
                    if payload[0] == 0 {
                        let _ = response.send(payload.len() == 32);
                    } else {
                        info!("other data {:?}", payload);
                    }
                }
                Message::Prepared { proof, payload } => {
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
                }
                Message::Finalized { proof, payload } => {
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
                }
            }
        }
    }
}
