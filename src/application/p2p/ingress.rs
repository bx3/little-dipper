use futures:: {
    channel::{mpsc, oneshot},
    SinkExt,
};

pub enum Message {
    /// view, mini-block
    SendMiniBlockToLeader {
        view: u64,
        mini_block: Vec<u8>,
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
    pub async fn send_mini_block_to_leader(&mut self, view: u64, mini_block: Vec<u8>) -> oneshot::Receiver<bool>{
        let (response, receiver) = oneshot::channel();
        self.sender
            .send(Message::SendMiniBlockToLeader { view, mini_block, response })
            .await
            .expect("Failed to send get mini blocks");
        receiver
    }
}
