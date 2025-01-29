use commonware_consensus::{Supervisor, ThresholdSupervisor};
use commonware_cryptography::{bls12381::primitives::group::Element, Ed25519, Scheme};
use futures::{channel::mpsc, StreamExt};
use super::{
    ingress::{Message, Mailbox},
    Config,
};
use tracing::info;
use std::collections::{BTreeMap, HashSet, VecDeque};
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
        mut crypto: Ed25519,
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

                    // bls pubkey
                    let pubkey = crypto.public_key();

                    data[1..9].copy_from_slice(&view.to_be_bytes());
                    let mut local_mini_block = MiniBlock {
                        view: view,
                        data: data.into(),
                        pubkey: pubkey.into(),
                        sig: sig.into(),
                    };

                    // sign message
                    let sig = local_mini_block.sign(&mut crypto);
                    local_mini_block.sig = sig;

                    // TODO should have taken all the mini-blocks to remove mem issue
                    let mini_blocks: MiniBlocks = match self.mini_blocks_cache.get(&view) {
                        Some(m) => {
                            // convert to MiniBlocks
                            info!("hello GetMiniBlocks num of cached mini blocks at view {:?} is {:?}", view, m.len());
                            let mut mini_blocks: Vec<MiniBlock> = vec![local_mini_block];
                            for (pubkey, mini_block) in m.into_iter() {
                                if mini_block.verify() {
                                    mini_blocks.push(mini_block.clone());
                                }         
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

                    // uncomment to simulate attack
                    //mini_blocks.mini_blocks.pop();

                    // TODO add a timeline to wait for peers about their mini-block for this view
                    // if they cannot get sufficient number of them
                    // if view is 1, it is ok, since it is starting
                    let quorum_participants_at_view = quorum(supervisor.participants(view).unwrap().len() as u32).unwrap() as usize;
                    if mini_blocks.mini_blocks.len() >= quorum_participants_at_view || view==1 {
                        response.send(mini_blocks);
                    } else {
                        info!("insufficint mini block at view {:?}. not respond anything. num miniblock {}, quorum {}", view, mini_blocks.mini_blocks.len(), quorum_participants_at_view);
                        // uncomment to simulate leader attack
                        //response.send(mini_blocks);
                    }                
                }
                Message::CheckSufficientMiniBlocks { view, mini_blocks, response } => {
                    // TODO should verify against the sigs and so on
                    // TODO check public keys are indeed from participants
                    let mut participants = HashSet::new();                        

                    for mini_block in mini_blocks.mini_blocks.into_iter() {                               
                        // make sure mini block is for curreent view
                        if mini_block.view != view {
                            continue
                        }
                        // if participant already sent a mini-block skip it
                        if participants.contains(&mini_block.pubkey) {
                            continue
                        }

                        if mini_block.is_participant(view, &supervisor)  {
                            if mini_block.verify() {                                
                                participants.insert(mini_block.pubkey);
                            }
                        }
                    }

                    info!("num_valid_mini_block {} at view {}", participants.len(), view);

                    // TODO it is very inefficient to send the entire miniBlocks struct. Should have a proposal struct
                    // that derives some smaller struct for sending over data
                    let quorum_participants_at_view = quorum(supervisor.participants(view).unwrap().len() as u32).unwrap() as usize;
                    if participants.len() >= quorum_participants_at_view || view == 1 {
                        response.send(true);
                    } else {
                        response.send(false);
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

                    // bls pubkey
                    let pubkey = crypto.public_key();
            
                    // This mini block is for the next view
                    let mut mini_block = MiniBlock {
                        view: view+1,
                        data: data,
                        pubkey: pubkey.into(),
                        sig:  vec![0u8; 0],
                    };

                    // sign message
                    let sig = mini_block.sign(&mut crypto);

                    mini_block.sig = sig;
                        
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
