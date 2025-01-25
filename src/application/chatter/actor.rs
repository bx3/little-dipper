use futures::{channel::mpsc, StreamExt};
use super::{
    ingress::{Message, Mailbox, MiniBlock},
    Config,
};
use tracing::info;
use std::collections::{BTreeMap, VecDeque};
use bytes::Bytes;
use crate::application::p2p::ingress::Mailbox as P2PMailbox;

pub struct Actor {
    control: mpsc::Receiver<Message>,
    mini_blocks_cache: BTreeMap<u64, BTreeMap<Bytes, MiniBlock>>, // view -> pubkey -> mini-block 
    chat_queue: VecDeque<Bytes>,
}

impl Actor {
    pub fn new() -> (Self, Mailbox) {
        let (control_sender, control_receiver) = mpsc::channel(100);
        (
            Self {
                control: control_receiver,
                mini_blocks_cache: BTreeMap::new(),
                chat_queue: VecDeque::new(),
            },
            Mailbox::new(control_sender),
        )
    }

    pub async fn run(
        mut self,
        mut p2p_mailbox: P2PMailbox,
    ) {
        // TODO need to periodically purge mini-blocks
        while let Some(msg) = self.control.next().await {
            match msg {
                Message::GetMiniBlocks { view, response } => {
                    let mut data: Vec<u8> = vec![0; 32];
                    let sig : Vec<u8> = vec![0; 32];

                    data[1..9].copy_from_slice(&view.to_be_bytes());
//                    let mini_block = MiniBlock {
//                        view: view,
//                        data: data.into(),
//                        sig: sig.into(),
//                    };
                    let mini_block = data.into();

                    info!("hello GetMiniBlocks");
                    /*
                    // if not such local view, return empty
                    if let None = self.mini_blocks_cache.get(&view) {
                        response.send((vec![], false));
                    }
                    for value in self.mini_blocks_cache[&view].values().take(){
                        mini_blocks.push(value);
                    }
                    */
                    let mut mini_blocks = Vec::new();
                    mini_blocks.push(mini_block);
                    response.send(mini_blocks);
                }
                Message::PutMiniBlocks { view, mini_blocks, response } => {
                    // tell user server that mini-blocks are done
                }
                Message::SendMiniBlock { view, response } => {
                    info!("chatter SendMiniBlock over P2P to leader");

                    // tell p2p server to send the mini-block for next view
                    // TODO create a mini_block from local cache
                    let mini_block = vec![0u8, 32];
                    let response = p2p_mailbox.send_mini_block_to_leader(view, mini_block).await;
                    // TODO not having the response is probably ok
                    let sent = response.await.unwrap();
                    info!("got response from p2p_mailbox for SendMiniBlock over P2P to leader");
                }
                // used by p2p server to receive mini blocks from peers 
                Message::LoadMiniBlockFromP2P { pubkey, mini_block, response } => {
                    info!("chatter LoadMiniBlockFromP2P");

                    // TODO parse view from mini-block in the future
                    let view = 0 ;//mini_block.view;
                    let mut alreay_has = false;
                    // if alreayd received notify true for alreayd receivd
                    match self.mini_blocks_cache.get(&view) {
                        Some(m) => {
                            if let Some(_) = self.mini_blocks_cache[&view].get(&pubkey) {
                                alreay_has = true;
                            }
                            // update anyway
                            //self.mini_blocks_cache[&view][&pubkey] = mini_block;
                        },
                        None => {
                            //self.mini_blocks_cache[&view] = BTreeMap::new();
                            //self.mini_blocks_cache[&view][&pubkey] = mini_block;
                        },
                    };

                    let _ = response.send(alreay_has);
                }
                // used by server to receive chat from users
                Message::LoadChat { data, response } => {
                    self.chat_queue.push_back(data);
                    // TODO if chat queue is too large, ask other end to stop
                    response.send(true);
                }
            }
        }
    }
}
