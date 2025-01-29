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
use crate::application::mini_block::{MiniBlock, ProtoBlock};
use commonware_utils::quorum;

pub struct Actor {
    /// for receiving message from other actors who have its mailbox
    control: mpsc::Receiver<Message>,
    /// view -> pubkey -> mini-block
    mini_blocks_cache: BTreeMap<u64, BTreeMap<Bytes, MiniBlock>>,  
    /// used to create local mini-block for some view
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
        supervisor: SupervisorImpl,
        mut crypto: Ed25519,
    ) {
        // TODO need to periodically purge mini-blocks
        while let Some(msg) = self.control.next().await {
            match msg {
                // validator sends the msg to the chatter for getting the next
                // block containing sufficient mini-blocks
                Message::GetProtoBlock { view, response } => {
                    // Create a local mini-block, TODO the content should come from data received
                    // from Message::LoadChat, that should be connected to a tcp server listening
                    // from users
                    let data = view.to_be_bytes();
                    let mut local_mini_block = MiniBlock::new(view, data.into(), crypto.public_key().into());

                    // sign message
                    local_mini_block.sign(&mut crypto);

                    // TODO should have taken all the mini-blocks to remove mem issue
                    let proto_block: ProtoBlock = match self.mini_blocks_cache.get(&view) {
                        Some(m) => {
                            // convert to ProtoBlock
                            let mut mini_blocks: Vec<MiniBlock> = vec![local_mini_block];
                            for (_, mini_block) in m.into_iter() {
                                if mini_block.verify() {
                                    mini_blocks.push(mini_block.clone());
                                }         
                            }
                            ProtoBlock{
                                mini_blocks: mini_blocks,
                            }
                        },
                        None => {
                            info!("hello GetProtoBlock no cached mini block at view {:?}", view);
                            ProtoBlock {
                                mini_blocks: vec![local_mini_block],
                            }
                        }
                    };

                    // uncomment to simulate attack // mini_blocks.mini_blocks.pop();

                    // TODO add a timeline to wait for peers about their mini-block for this view
                    // if they cannot get sufficient number of them
                    // if view is 1, it is ok, since it is starting
                    let quorum_participants_at_view = quorum(supervisor.participants(view).unwrap().len() as u32).unwrap() as usize;
                    if proto_block.mini_blocks.len() >= quorum_participants_at_view || view==1 {
                        response.send(proto_block).unwrap();
                    } else {
                        info!("insufficint mini block at view {:?}. not respond anything. num miniblock {}, quorum {}", view, proto_block.mini_blocks.len(), quorum_participants_at_view);
                        // uncomment to simulate leader attack //response.send(mini_blocks);
                    }                
                }
                // Used by consensus Verify to check if sufficient mini-blocks are proposed  
                // TODO it is very inefficient to send the entire ProtoBlock struct. Should have a proposal struct
                // that derives some smaller struct for sending over data
                Message::CheckSufficientProtoBlock { view, proto_block, response } => {                
                    let mut participants = HashSet::new();                        

                    for mini_block in proto_block.mini_blocks.into_iter() {                               
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
                    let quorum_participants_at_view = quorum(supervisor.participants(view).unwrap().len() as u32).unwrap() as usize;
                    if participants.len() >= quorum_participants_at_view || view == 1 {
                        response.send(true).unwrap();
                    } else {
                        response.send(false).unwrap();
                    }    
                }
                // Used by the chatter to tell api server that mini-blocks are finished
                Message::PutProtoBlock { view, proto_block, response } => {
                    unimplemented!()
                }
                // Used by all non-leader validator to send mini-block to the leader in the next view. It is triggered by
                // either Verify request from the consensus logics, or nullify signal from the consensus 
                // Behave like a traffic generator
                Message::SendMiniBlock { view, response } => {
                    info!("chatter SendMiniBlock over P2P to leader");
                    // TODO Generate mini-blocks from local cache received from the server                    

                    // tell p2p server to send the mini-block for next view
                    let data = view.to_be_bytes();
            
                    // This mini block is for the next view
                    let mut mini_block = MiniBlock::new(view+1, data.into(), crypto.public_key().into())                    ;

                    // sign message
                    mini_block.sign(&mut crypto);                
                        
                    let p2p_response = p2p_mailbox.send_mini_block_to_leader(view, mini_block).await;
                    // TODO not having the response is probably ok
                    match p2p_response.await {
                        Ok(r) => response.send(r).unwrap(),
                        Err(_) => response.send(false).unwrap(),
                    }                            
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
                    response.send(true).unwrap();
                }
            }
        }
    }
}
