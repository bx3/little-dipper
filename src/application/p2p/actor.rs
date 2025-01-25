use super::{
    ingress::{Message, Mailbox},
};
use super::super::chatter::ingress::Mailbox as ChatterMailbox;
use futures::{channel::{mpsc, oneshot}, SinkExt,StreamExt};

use crate::application::supervisor::Supervisor;
use crate::wire;
use commonware_consensus::{
    ThresholdSupervisor as TSU,
};


use commonware_utils::hex;

use commonware_p2p::{Receiver, Sender, Recipients};
use commonware_macros::select;
use tracing::info;

use commonware_cryptography::{
    bls12381::primitives::{
        group::{self, Element},
    },
};


use prost::Message as _;


pub struct Actor {
    control: mpsc::Receiver<Message>,
    chatter_mailbox: ChatterMailbox,
    supervisor: Supervisor,
}

enum P2PMessage {
    PutMiniBlock {
        incoming: wire::PutMiniBlock,
        response: oneshot::Sender<bool>,
    },
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
            select! {
                // receive request from chatter to send mini-block to leader over direct conn
                chatter_msg = self.control.next() => {
                    match chatter_msg.unwrap() {
                        Message::SendMiniBlockToLeader{view, mini_block, response} => {
                            info!("p2p server will send mini block to leader by broadcast");
                            // TODO serialize data into bytes array
                            let msg = mini_block; 
                            // TODO send over p2p only to the current leader
                            let filler_seed = group::Signature::one();
                            let next_leader = self.supervisor.leader(view, filler_seed).unwrap();

                            info!("next leader is {:?}", hex(&next_leader));
                            sender.send(Recipients::One(next_leader), msg.into(), false).await.unwrap();
                            response.send(true);
                        }
                    }
                },
                // receive from p2p about mini-block and send it to chatter
                p2p_msg = receiver.recv() => {
                    info!("p2p server got mini block from over p2p");
                    let Ok((s, msg)) = p2p_msg else {
                        break;
                    };

                    let pubKey = vec![0u8; 32];

                    // TODO use 1 mini-block struct, or convert between two, proto and
                    // message
                    // TODO decode msg from Protobuf
                    let response = self.chatter_mailbox.load_mini_block(pubKey.into(), msg.into()).await;
                    let _sent = response.await;
                    info!("p2p server delivered mini block to chatter actor");


                    /*
                    let Ok(msg) = wire::Inbound::decode(msg) else {
                        panic!("cannot decode p2p msg");
                    };

                    let Some(payload) = msg.payload else {
                        panic!("payload is empty ");
                    };
                    match payload {
                        wire::inbound::Payload::PutMiniBlock(msg) => {
                            // TODO send data to chatter
                            let pubKey = vec![0u8; 32];

                            // TODO use 1 mini-block struct, or convert between two, proto and
                            // message
                            let response = self.chatter_mailbox.load_mini_block(pubKey.into(), msg.data.into()).await;
                            let _sent = response.await;
                            info!("p2p server delivered mini block to chatter actor");

                        },
                    }
                    */
                },
            }
        }


    }

}
