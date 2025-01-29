use futures::{channel::mpsc ,StreamExt};
use commonware_consensus::ThresholdSupervisor as TSU;
use crate::application::supervisor::Supervisor;
use crate::application::p2p::ingress::{Message, Mailbox};
use crate::application::chatter::ingress::Mailbox as ChatterMailbox;
use crate::wire;

use commonware_utils::hex;
use commonware_p2p::{Receiver, Sender, Recipients};
use commonware_macros::select;
use commonware_cryptography::bls12381::primitives::group::{self, Element};
use tracing::{info, error};
use prost::Message as _;

pub struct Actor {
    control: mpsc::Receiver<Message>,
    chatter_mailbox: ChatterMailbox,
    supervisor: Supervisor,
}

impl Actor {
    pub fn new(
        chatter_mailbox: ChatterMailbox,
        supervisor: Supervisor,
    ) -> (Self, Mailbox) {
        let (control_sender, control_receiver) = mpsc::channel(100);
        (
            Self {
                control: control_receiver,
                chatter_mailbox: chatter_mailbox,
                supervisor: supervisor,
            },
            Mailbox::new(control_sender),
        )
    }

    pub async fn run(
        mut self, 
        mut sender: impl Sender,
        mut receiver: impl Receiver,
    ) {
        loop {
            // select to listen for commands from chatter, or events from p2p network (only leader will receive)
            select! {
                // receive request from chatter to send mini-block to leader over direct conn
                chatter_msg = self.control.next() => {
                    match chatter_msg.unwrap() {
                        Message::SendMiniBlockToLeader{view, mini_block, response} => {
                            info!("p2p server will send mini block to leader by broadcast");
                            
                            let filler_seed = group::Signature::one();
                            // + 1 for next view
                            let next_leader = self.supervisor.leader(view+1, filler_seed).unwrap();

                            info!("next leader is {:?}", hex(&next_leader));
                            let mini_block_json = serde_json::to_vec(&mini_block).unwrap();

                            // to wire
                            let inbound_msg = wire::Inbound {
                                payload: Some(wire::inbound::Payload::PutMiniBlock(wire::PutMiniBlock{data: mini_block_json.into()})),
                            }
                            .encode_to_vec();

                            sender.send(Recipients::One(next_leader), inbound_msg.into(), false).await.unwrap();

                            let _ = response.send(true).map_err(|e| error!("send_mini_bock_to_leader p2p error {}", e));
                        }
                    }
                },
                // receive from p2p about mini-block and send it to chatter
                p2p_msg = receiver.recv() => {
                    info!("p2p server got mini block from over p2p");
                    let Ok((pubkey, msg)) = p2p_msg else {
                        break;
                    };

                    let msg = match wire::Inbound::decode(msg) {
                        Ok(msg) => msg,
                        Err(_) => continue,
                    };

                    let Some(payload) = msg.payload else {                        
                        continue
                    };

                    match payload {
                        wire::inbound::Payload::PutMiniBlock(msg) => {
                            let mini_block = serde_json::from_slice(&msg.data).unwrap();                            
                            let response = self.chatter_mailbox.load_mini_block(pubkey, mini_block).await;
                            let _sent = response.await;                            
                        },
                    }
                },
            }
        }


    }

}
