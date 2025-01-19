use bytes::Bytes;
use futures:: {
    channel::{mpsc, oneshot},
    SinkExt,
};

#[derive(Clone, Debug, Default)]
pub struct MiniBlock {
    pub view: u64,
    pub data: Bytes,
    pub sig: Bytes,
}

/// Message
pub enum Message {
    // communication with application actor
    PutMiniBlocks {
        view: u64,
        mini_blocks: Vec<MiniBlock>,
        response: oneshot::Sender<bool>,
    },
    GetMiniBlocks {
        view: u64,
        response: oneshot::Sender<(Vec<MiniBlock>, bool)>,
    },
    SendMiniBlock {
        view: u64,
        response: oneshot::Sender<bool>,
    },
    // communication with chat server
    LoadMiniBlock {
        pubkey: Bytes,
        mini_block: MiniBlock,
        response: oneshot::Sender<bool>,
    },
    LoadChat {
        data: Bytes,
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
    /// return bool, it alreayd received
    async fn put_mini_blocks(&mut self, view: u64, mini_blocks: Vec<MiniBlock>) -> oneshot::Receiver<bool>{
        let (response, receiver) = oneshot::channel();
        self.sender
            .send(Message::PutMiniBlocks { view, mini_blocks, response })
            .await
            .expect("Failed to send get mini blocks");
        receiver
    }
    
    /// ask chatter to get mini-blocks for proposing
    async fn get_mini_blocks(&mut self, view: u64) -> oneshot::Receiver<Vec<MiniBlock>> {
        let (response, receiver) = oneshot::channel();
        self.sender
            .send(Message::GetMiniBlocks { view, response })
            .await
            .expect("Failed to send get mini blocks");
        receiver
    }

    /// request the chatter to send to the chatter from leader
    /// Return if already sent
    async fn send_mini_block(&mut self, view: u64) -> oneshot::Receiver<bool> {
        let (response, receiver) = oneshot::channel();
        self.sender
            .send(Message::SendMiniBlock { view, response })
            .await
            .expect("Failed to send get mini blocks");
        receiver
    }

    async fn load_mini_block(&mut self, pubkey: Bytes, mini_block: MiniBlock) -> oneshot::Receiver<bool> {
        let (response, receiver) = oneshot::channel();
        self.sender
            .send(Message::LoadMiniBlock
                { pubkey, mini_block, response })
            .await
            .expect("Failed to send get mini blocks");
        receiver
    }

    async fn load_chat(&mut self, data: Bytes) -> oneshot::Receiver<bool> {
        let (response, receiver) = oneshot::channel();
        self.sender
            .send(Message::LoadChat
                { data, response })
            .await
            .expect("Failed to send get mini blocks");
        receiver
    }
}


