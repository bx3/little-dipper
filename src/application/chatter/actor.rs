use commonware_consensus::Supervisor;
use futures::{channel::mpsc, StreamExt};
use super::{
    ingress::{Message, Mailbox},
    Config,
};
use tracing::info;
use std::collections::{BTreeMap, VecDeque};
use bytes::Bytes;
use crate::application::{p2p::ingress::Mailbox as P2PMailbox, supervisor::Supervisor as SupervisorImpl};
use crate::application::mini_block::{MiniBlock, MiniBlocks};
use commonware_utils::quorum;

pub struct Actor {
    control: mpsc::Receiver<Message>,
    mini_blocks_cache: BTreeMap<u64, BTreeMap<Bytes, MiniBlock>>, // view -> pubkey -> mini-block 
    chat_queue: VecDeque<Bytes>, // used to create local mini-block for some view
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
        supervisor: SupervisorImpl,
    ) {
        // TODO need to periodically purge mini-blocks
        while let Some(msg) = self.control.next().await {
            match msg {
                // validator sends the msg to the chatter for getting the next
                // block containing sufficient mini-blocks
                Message::GetMiniBlocks { view, response } => {
                    // Create a local mini-block, TODO the content should come from data received
                    // from Message::LoadChat, that should be connected to a tcp server listening
                    // from users
                    let mut data: Vec<u8> = vec![0; 32];
                    let sig : Vec<u8> = vec![0; 32];

                    data[1..9].copy_from_slice(&view.to_be_bytes());
                    let local_mini_block = MiniBlock {
                        view: view,
                        data: data.into(),
                        sig: sig.into(),
                    };

                    // TODO should have taken all the mini-blocks to remove mem issue
                    let mini_blocks: MiniBlocks = match self.mini_blocks_cache.get(&view) {
                        Some(m) => {
                            // convert to MiniBlocks
                            info!("hello GetMiniBlocks num of cached mini blocks at view {:?} is {:?}", view, m.len());
                            let mut mini_blocks: Vec<MiniBlock> = vec![local_mini_block];
                            for value in m.values() {
                                mini_blocks.push(value.clone());
                            }
                            MiniBlocks{
                                mini_blocks: mini_blocks,
                            }
                        },
                        None => {
                

                            info!("hello GetMiniBlocks no cached mini block at view {:?}", view);
                            MiniBlocks {
                                mini_blocks: vec![local_mini_block],
                            }
                        }
                    };

                    // TODO add a timeline to wait for peers about their mini-block for this view
                    // if they cannot get sufficient number of them
                    // if view is 1, it is ok, since it is starting
                    let quorum_participants_at_view = quorum(supervisor.participants(view).unwrap().len() as u32).unwrap() as usize;
                    if mini_blocks.mini_blocks.len() >= quorum_participants_at_view || view==1 {
                        response.send(mini_blocks);
                    } else {
                        info!("insufficint mini block at view {:?}. not respond anything. num miniblock {}, quorum {}", view, mini_blocks.mini_blocks.len(), quorum_participants_at_view);
                    }                
                }
                Message::PutMiniBlocks { view, mini_blocks, response } => {
                    // tell user server that mini-blocks are done
                }
                Message::SendMiniBlock { view, response } => {
                    info!("chatter SendMiniBlock over P2P to leader");
                    // TODO create a mini_block from local cache
                    // This is like traffic generator

                    // tell p2p server to send the mini-block for next view
                    let mut data = vec![0u8; 32];
                    data[1..9].copy_from_slice(&view.to_be_bytes());
                    
                    // This mini block is for the next view
                    let mini_block = MiniBlock {
                        view: view+1,
                        data: data,
                        sig:  vec![0u8, 32],
                    };
                        
                    let p2p_response = p2p_mailbox.send_mini_block_to_leader(view, mini_block).await;
                    // TODO not having the response is probably ok
                    let sent = p2p_response.await.unwrap();
                    info!("sent MiniBlock over P2P to leader {}", sent);
                    response.send(true);
                }
                // used by p2p server to receive mini blocks from peers 
                Message::LoadMiniBlockFromP2P {pubkey, mini_block, response } => {
                    info!("chatter LoadMiniBlockFromP2P for view {}", mini_block.view);

                    let view = mini_block.view;
                    let mut alreay_has = false;

                    // TODO cache those mini-blocks to be used in the next proposal

                    match self.mini_blocks_cache.get_mut(&view) {
                        Some(m) => {
                            if let Some(_) = m.get(&pubkey) {
                                alreay_has = true;
                            }
                            // update anyway
                            m.insert(pubkey, mini_block);
                        },
                        None => {
                            self.mini_blocks_cache.insert(view, BTreeMap::new());                        
                            self.mini_blocks_cache.get_mut(&view).unwrap().insert(pubkey, mini_block);
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
