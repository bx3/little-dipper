use bytes::Bytes;
use futures:: {
    channel::{mpsc, oneshot},
    SinkExt,
};
use crate::application::mini_block::{MiniBlock, ProtoBlock};
use commonware_cryptography::PublicKey;


/// Message
pub enum Message {
    // communication with application actor
    PutProtoBlock {
        view: u64,
        proto_block: ProtoBlock,
    },
    GetProtoBlock {
        view: u64,
        response: oneshot::Sender<ProtoBlock>,
    },
    SendMiniBlock {
        view: u64,
        response: oneshot::Sender<bool>,
    },
    // communication with chat server
    LoadMiniBlockFromP2P {
        pubkey: PublicKey,
        mini_block: MiniBlock,
        response: oneshot::Sender<bool>,
    },
    LoadChat {
        // at which view should be included
        view: u64,
        data: Bytes,
    },
    CheckSufficientProtoBlock {
        view: u64,
        proto_block: ProtoBlock,
        response: oneshot::Sender<bool>,
    },
}


/// Mailbox for chatter
#[derive(Clone)]
pub struct Mailbox {
    sender: mpsc::Sender<Message>,
}

impl Mailbox {
    pub fn new(sender: mpsc::Sender<Message>) -> Self {
        Self { sender }
    }

    /// notify chatter app async to put mini blocks after finalization
    pub async fn put_proto_block(&mut self, view: u64, proto_block: ProtoBlock){
        self.sender
            .send(Message::PutProtoBlock { view, proto_block})
            .await
            .expect("Failed to send get mini blocks");
    }
    
    /// ask chatter to get mini-blocks for proposing
    pub async fn get_proto_block(&mut self, view: u64) -> oneshot::Receiver<ProtoBlock> {
        let (response, receiver) = oneshot::channel();
        self.sender
            .send(Message::GetProtoBlock { view, response })
            .await
            .expect("Failed to send get mini blocks");
        receiver
    }

    /// request the chatter to send to the chatter from leader
    /// Return if already sent
    pub async fn send_mini_block(&mut self, view: u64) -> oneshot::Receiver<bool> {
        let (response, receiver) = oneshot::channel();
        self.sender
            .send(Message::SendMiniBlock { view, response })
            .await
            .expect("Failed to send get mini blocks");
        receiver
    }

    pub async fn load_mini_block(&mut self, pubkey: PublicKey, mini_block: MiniBlock) -> oneshot::Receiver<bool> {
        let (response, receiver) = oneshot::channel();
        self.sender
            .send(Message::LoadMiniBlockFromP2P
                { pubkey, mini_block, response })
            .await
            .expect("Failed to send get mini blocks");
        receiver
    }

    pub async fn load_chat(&mut self, view: u64, data: Bytes){
        self.sender
            .send(Message::LoadChat
                { view, data })
            .await
            .expect("Failed to send get mini blocks");
    }

    pub async fn check_sufficient_mini_blocks(&mut self, view: u64, proto_block: ProtoBlock) -> oneshot::Receiver<bool> {
        let (response, receiver) = oneshot::channel();
        self.sender
            .send(Message::CheckSufficientProtoBlock { view: view, proto_block: proto_block, response: response })
            .await
            .expect("Failed to send get mini blocks");
        receiver
    }
}
