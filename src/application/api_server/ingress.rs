use crate::application::mini_block::ProtoBlock;
use futures:: {
    channel::mpsc,
    SinkExt,
};

pub enum Message {
    /// when consensus is making progress
    ProtoBlockPrepared {
        proto_block: ProtoBlock,
    },
    /// when consensus block is finalized
    ProtoBlockFinalized {
        proto_block: ProtoBlock,
    },
    /// when consensus has temporary unavailability
    ProtoBlockNullify {
        view: u64,
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

    /// notify chatter app async that a new proto_block is prepared
    /// server can use this info to inform user about status
    pub async fn send_proto_block_prepared(&mut self, proto_block: ProtoBlock) {
        self.sender
            .send(Message::ProtoBlockPrepared { proto_block } )
            .await
            .expect("Failed to send get mini blocks");
    }

    /// notify chatter app async that a new proto_block is finalized
    /// server can use this info to inform user about status
    pub async fn send_proto_block_finalized(&mut self, proto_block: ProtoBlock) {
        self.sender
            .send(Message::ProtoBlockFinalized { proto_block } )
            .await
            .expect("Failed to send get mini blocks");
    }

    /// notify chatter app async that a new proto_block is undergoing nullifying
    /// server can use this info to inform user about status. It is possible that the view
    /// if finalized in the end, or it could the case the view has nullification
    pub async fn send_proto_block_nullify(&mut self, view: u64) {
        self.sender
            .send(Message::ProtoBlockNullify { view: view } )
            .await
            .expect("Failed to send get mini blocks");
    }
}