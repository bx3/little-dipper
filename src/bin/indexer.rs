use bytes::Bytes;
use clap::{value_parser, Arg, Command};
use little_dipper::{wire, APPLICATION_NAMESPACE, CONSENSUS_SUFFIX, INDEXER_NAMESPACE};
use commonware_consensus::threshold_simplex::Prover;
use commonware_cryptography::{
    bls12381::primitives::group::{self, Element},
    Ed25519, Hasher, Scheme, Sha256,
};
use commonware_runtime::{tokio::Executor, Listener, Network, Runner, Spawner};
use commonware_stream::{
    public_key::{Config, Connection, IncomingConnection},
    Receiver, Sender,
};
use commonware_utils::{from_hex, hex, union};
use futures::{
    channel::{mpsc, oneshot},
    SinkExt, StreamExt,
};
use prost::Message as _;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};
use tracing::{debug, info};

enum Message {
    PutBlock {
        incoming: wire::PutBlock,
        response: oneshot::Sender<bool>, // wait to broadcast consensus message
    },
    GetBlock {
        incoming: wire::GetBlock,
        response: oneshot::Sender<Option<Bytes>>,
    },
    PutFinalization {
        incoming: wire::PutFinalization,
        response: oneshot::Sender<bool>, // wait to delete from validator storage
    },
    GetFinalization {
        incoming: wire::GetFinalization,
        response: oneshot::Sender<Option<Bytes>>,
    },
}

fn main() {
    // Parse arguments
    let matches = Command::new("indexer")
        .about("collect blocks and finality certificates")
        .arg(Arg::new("me").long("me").required(true))
        .get_matches();

    // Create logger
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    // Configure my identity
    let me = matches
        .get_one::<String>("me")
        .expect("Please provide identity");
    let parts = me.split('@').collect::<Vec<&str>>();
    if parts.len() != 2 {
        panic!("Identity not well-formed");
    }
    let key = parts[0].parse::<u64>().expect("Key not well-formed");
    let signer = Ed25519::from_seed(key);
    tracing::info!(key = hex(&signer.public_key()), "loaded signer");

    // Configure my port
    let port = parts[1].parse::<u16>().expect("Port not well-formed");
    tracing::info!(port, "loaded port");
    let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);


    // Create runtime
    let (executor, runtime) = Executor::default();
    executor.start(async move {

        // Start listener
        let mut listener = runtime.bind(socket).await.expect("failed to bind listener");
        let config = Config {
            crypto: signer,
            namespace: INDEXER_NAMESPACE.to_vec(),
            max_message_size: 1024 * 1024,
            synchrony_bound: Duration::from_secs(1),
            max_handshake_age: Duration::from_secs(60),
            handshake_timeout: Duration::from_secs(5),
        };
        loop {
            // Listen for connection
            let Ok((_, sink, stream)) = listener.accept().await else {
                debug!("failed to accept connection");
                continue;
            };

            // Handshake
            let incoming =
                match IncomingConnection::verify(&runtime, config.clone(), sink, stream).await {
                    Ok(partial) => partial,
                    Err(e) => {
                        debug!(error = ?e, "failed to verify incoming handshake");
                        continue;
                    }
                };
            let peer = incoming.peer();
            let stream = match Connection::upgrade_listener(runtime.clone(), incoming).await {
                Ok(connection) => connection,
                Err(e) => {
                    debug!(error = ?e, peer=hex(&peer), "failed to upgrade connection");
                    continue;
                }
            };
            info!(peer = hex(&peer), "upgraded connection");

        }
    });
}
