use crate::wire;

use super::{
    ingress::{Mailbox, Message},
    supervisor::Supervisor,
    Config,
};
use bytes::BufMut;
use commonware_consensus::simplex::Prover;
use commonware_cryptography::{
    bls12381::primitives::{group::Element, poly},
    Hasher,
    Scheme,
    Bls12381,

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
    chatter_sender: mpsc::Receiver<Message>,
    prover: Prover<Bls12381, H>,
    hasher: H,
    mailbox: mpsc::Receiver<Message>,
}

impl<R: Rng, H: Hasher, Si: Sink, St: Stream> Application<R, H, Si, St> {
    /// Create a new application actor.
    pub fn new(runtime: R, config: Config<H, Si, St>) -> (Self, Supervisor, Mailbox) {
        let (sender, mailbox) = mpsc::channel(config.mailbox_size);
        (
            Self {
                runtime,
                chatter_sender: config.chatter,
                prover: config.prover,
                hasher: config.hasher,
                mailbox,
            },
            Supervisor::new(config.identity, config.participants),
            Mailbox::new(sender),
        )
    }

    /// Run the application actor.
    pub async fn run(mut self) {
        let (mut chatter_sender, mut chatter_receiver) = self.chatter.split();
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
                    // Either propose a random message (prefix=0) or include some message from
                    // chatter (prefix=1)
                    // Fetch chat from the chatter server
                    let msg = wire::GetMiniBlocks {
                        view: index,
                    };
                    let msg = wire::Inbound {
                        payload: Some(wire::inbound::Payload::GetMiniBlocks(msg)),
                    }
                    .encode_to_vec();
                    chatter_sender
                        .send(&msg)
                        .await
                        .expect("failed to send GetChat to chatter");
                    let result = chatter_receiver
                        .receive()
                        .await
                        .expect("failed to receive from indexer");
                    let msg =
                        wire::Outbound::decode(result).expect("failed to decode result");
                    let payload = msg.payload.expect("missing payload");
                    let mut mini_blocks = match payload {
                        wire::outbound::Payload::Insufficient(_) => {
                            debug!("insufficient mini blocks");
                            continue;
                        },
                        wire::outbound::Payload::MiniBlocks(f) => f,
                        _ => panic!("unexpected response"),
                    };

                    // If nothing from chatter server, then generate a random message
                    // TODO generate random bytes for mini blocks
                    self.runtime.fill(&mut mini_blocks[..]);
                    
                    // Use chat as message
                    let mut msg = Vec::with_capacity(1 + mini_blocks.len());
                    msg.put_u8(1);
                    msg.extend(mini_blocks);

                    debug!(view = index, "mini blocks included");

                    // Send digest to consensus once we confirm indexer has underlying data
                    let _ = response.send(msg.into());
                }
                Message::Verify { payload, response } => {
                    // Ensure payload is a valid digest
                    if !H::validate(&payload) {
                        let _ = response.send(false);
                        continue;
                    }

                    // TODO validate if sufficient mini-blocks are signed and included
                    let result = true;

                    // If payload exists and is valid, return
                    let _ = response.send(result);

                    // TODO async notify chatter to prepare and send the next mini-block to next leader
                }
                Message::Prepared { proof, payload } => {
                    //let (view, _, _, signature, seed) =
                    //    self.prover.deserialize_notarization(proof).unwrap();
                    //let signature = signature.serialize();
                    //let seed = seed.serialize();
                    info!(
                        payload = hex(&payload),
                        "prepared"
                    )

                    // TODO async notify chatter preivous block is WIP
                    // send blocks for rendering
                }
                Message::Finalized { proof, payload } => {
                    //let (view, _, _, signature, seed) =
                    //    self.prover.deserialize_finalization(proof.clone()).unwrap();
                    //let signature = signature.serialize();
                    //let seed = seed.serialize();
                    //info!(
                    //    view,
                    //    payload = hex(&payload),
                    //    signature = hex(&signature),
                    //    seed = hex(&seed),
                    //    "finalized"
                    //);

                    // TODO async notify chatter to update chats are finalized
                    // debug!(view, "finalization posted");
                }
            }
        }
    }
}
