use futures::{channel::mpsc, StreamExt};
use super::{
    ingress::{Message, Mailbox, MiniBlock},
    Config,
};
use tracing::info;
use std::collections::{BTreeMap, VecQueue};
use bytes::Bytes;

pub struct Actor {
    control: mpsc::Receiver<Message>,
    mini_blocks_cache: BTreeMap<u64, BTreeMap<Bytes, MiniBlock>>, // view -> pubkey -> mini-block 
    chat_queue: VecQueue<Bytes>,
}

impl Actor {
    pub fn new(cfg: Config) -> (Self, Mailbox) {
        let (control_sender, control_receiver) = mpsc::channel(cfg.mailbox_size);
        (
            Self {
                control: control_receiver,
                mini_blocks_cache: BTreeMap::new(),
                chat_queue: VecQueue::new(),
            },
            Mailbox::new(control_sender),
        )
    }

    pub async fn run(mut self) {
        // TODO need to periodically purge mini-blocks
        while let Some(msg) = self.control.next().await {
            match msg {
                Message::GetMiniBlocks { view, response } => {
                    info!("hello GetMiniBlocks");
                    // if not such local view, return empty
                    if let None = self.mini_blocks_cache.get(&view) {
                        response.send((vec![], false));
                    }
                    let mut mini_blocks: Vec<MiniBlock> = Vec::new();
                    for value in self.mini_blocks_cache[&view].values(){
                        mini_blocks.push(*value);
                    }
                    response.send((mini_blocks, true));
                }
                Message::PutMiniBlocks { view, mini_blocks, response } => {
                    // tell user server that mini-blocks are done
                }
                Message::SendMiniBlock { view, response } => {
                    // tell p2p server to send the mini-block for next view
                }
                // used by p2p server to receive mini blocks from peers 
                Message::LoadMiniBlock { pubkey, mini_block, response } => {
                    let view = mini_block.view;
                    let mut alreay_has = false;
                    // if alreayd received notify true for alreayd receivd
                    match self.mini_blocks_cache.get(&view) {
                        Some(m) => {
                            if let Some(_) = self.mini_blocks_cache[&view].get(&pubkey) {
                                alreay_has = true;
                            }
                            // update anyway
                            self.mini_blocks_cache[&view][&pubkey] = mini_block;
                        },
                        None => {
                            self.mini_blocks_cache[&view] = BTreeMap::new();
                            self.mini_blocks_cache[&view][&pubkey] = mini_block;
                        },
                    };

                    response.send(alreay_has);
                }
                // used by server to receive chat from users
                Message::LoadChat { data, response } => {
                    self.chat_queue.push(data);
                    // TODO if chat queue is too large, ask other end to stop
                    response.send(true);
                }
            }
        }
    }
}
