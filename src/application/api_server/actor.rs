use std::collections::BTreeMap;

use crate::application::chatter::ingress::Mailbox as ChatterMailbox;
use crate::application::api_server::ingress::{Mailbox, Message};
use crate::application::mini_block::ProtoBlock;

use futures::{channel::mpsc, StreamExt};
use futures::lock::Mutex;

use std::sync::{Arc, RwLock};
use bytes::Bytes;

use axum::{
    routing::{get, post},
    http::StatusCode,
    Json, Router,
    extract::{DefaultBodyLimit, Path, State},
    debug_handler,
};
use serde::{Deserialize, Serialize};

use commonware_runtime::{Blob, Clock, Spawner, Storage, tokio::Context};
use governor::clock::Clock as GClock;
use rand::{CryptoRng, Rng};


pub struct Actor {
    /// runtime
    runtime: Context,
    /// for receiving message defined in the ingress
    control: mpsc::Receiver<Message>,
    /// for sending message to chatter
    chatter_mailbox: ChatterMailbox,
    // connector between server and chatter
    validator_id: u64,
}

#[derive(Copy, Clone, Serialize, Deserialize)]
pub enum ViewState {
    Empty,
    Prepared,
    Finalized,
    Nullify, // temporary state
}

#[derive(Clone)]
pub struct AppState {
    pub proto_blocks: BTreeMap<u64, ProtoBlock>,
    pub view_state: BTreeMap<u64, ViewState>,
    pub chatter_mailbox: ChatterMailbox,
    pub chatter_busy: bool,
}

type SharedState = Arc<Mutex<AppState>>;

impl Actor {
    pub fn new(chatter_mailbox: ChatterMailbox, runtime: Context, validator_id: u64) -> (Self, Mailbox) {
        let (control_sender, control_receiver) = mpsc::channel(100);
    
        (
            Self {
                control: control_receiver,
                chatter_mailbox: chatter_mailbox,            
                runtime: runtime,
                validator_id: validator_id,
            },
            Mailbox::new(control_sender),
        )
    }

    /// Start an api server
    pub async fn run(mut self) {
        let app_state = AppState {
            proto_blocks: BTreeMap::new(),
            view_state: BTreeMap::new(),
            chatter_mailbox: self.chatter_mailbox,
            chatter_busy: false,
        };

        let shared_state = SharedState::new( Mutex::new(app_state));
        let server_clone = shared_state.clone();
        // run server
        let amux_server_handle = self.runtime.spawn("amux_server", async move {
            let app = Router::new()
                .route("/", get(chat))
                .route("/status", get(view_status))
                .route("/get_block", get(get_block))
                .with_state(server_clone);            

            // run our app with hyper, listening globally on port 3000
            let port = (3000 + self.validator_id).to_string();
            let mut socket="0.0.0.0:".to_string();
            socket.push_str(&port);
            let listener = tokio::net::TcpListener::bind(socket).await.unwrap();
            axum::serve(listener, app).await.unwrap();
        });

        // manage internal state
        while let Some(msg) = self.control.next().await {
            match msg {
                Message::ProtoBlockPrepared { proto_block } => {
                    let mut writer = shared_state.lock().await;
                    let view = proto_block.view;
                    writer.proto_blocks.insert(view, proto_block);
                    writer.view_state.insert(view, ViewState::Prepared);
                }
                Message::ProtoBlockFinalized { proto_block } => {
                    let mut writer = shared_state.lock().await;
                    let view = proto_block.view;
                    writer.proto_blocks.insert(view, proto_block);
                    writer.view_state.insert(view, ViewState::Prepared);
                }
                Message::ProtoBlockNullify { view } => {
                    let mut writer = shared_state.lock().await;
                    writer.view_state.insert(view, ViewState::Nullify);
                }
            }
        }

        amux_server_handle.await.unwrap();   
    }
}


#[debug_handler]
async fn chat(
    State(state): State<SharedState>,
    Json((view, data)): Json<(u64, Vec<u8>)>,
) {
    // just send to chatter 
    // if chatter is busy, this chatter is locking. seems problematic
    state.lock().await.chatter_mailbox.load_chat(view, Bytes::copy_from_slice(&data)).await;
}

#[debug_handler]
async fn view_status(
    State(state): State<SharedState>,
    Json(view): Json<u64>,
) -> Json<ViewState> {
    match state.lock().await.view_state.get(&view) {
        Some(a) => Json(a.clone()),
        None => Json(ViewState::Empty),
    }
}

#[debug_handler]
async fn get_block(
    State(state): State<SharedState>,
    Json(view): Json<u64>,
) -> Json<Vec<u8>> {
    match state.lock().await.proto_blocks.get(&view) {
        Some(a) => Json(serde_json::to_vec(a).unwrap()),
        None => Json(vec![]),
    }
}
