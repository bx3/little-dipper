use clap::{value_parser, Arg, Command};
use little_dipper::{
    application, APPLICATION_NAMESPACE, CONSENSUS_SUFFIX, P2P_SUFFIX,
};
use little_dipper::application::chatter::actor::Actor;
use little_dipper::application::p2p::actor::Actor as P2PActor;

use commonware_consensus::threshold_simplex::{self, Engine, Prover};
use commonware_cryptography::{
    bls12381::primitives::{
        group,
        poly::{self, Poly},
    },
    Ed25519, Scheme, Sha256,
};
use commonware_p2p::authenticated;
use commonware_runtime::{
    tokio::{self, Executor},
    Network, Runner, Spawner,
};
use commonware_storage::journal::{self, Journal};
use commonware_utils::{from_hex, hex, quorum, union};
use governor::Quota;
use prometheus_client::registry::Registry;
use std::sync::{Arc, Mutex};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    num::NonZeroU32,
};
use std::{str::FromStr, time::Duration};

fn main() {
    // Parse arguments
    let matches = Command::new("validator")
        .about("produce finality certificates and verify external finality certificates")
        .arg(
            Arg::new("bootstrappers")
                .long("bootstrappers")
                .required(false)
                .value_delimiter(',')
                .value_parser(value_parser!(String)),
        )
        .arg(Arg::new("me").long("me").required(true))
        .arg(
            Arg::new("participants")
                .long("participants")
                .required(true)
                .value_delimiter(',')
                .value_parser(value_parser!(u64))
                .help("All participants"),
        )
        .arg(Arg::new("storage-dir").long("storage-dir").required(true))
        .arg(Arg::new("identity").long("identity").required(true))
        .arg(Arg::new("share").long("share").required(true))
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

    // Configure allowed peers
    let mut validators = Vec::new();
    let participants = matches
        .get_many::<u64>("participants")
        .expect("Please provide allowed keys")
        .copied();
    if participants.len() == 0 {
        panic!("Please provide at least one participant");
    }
    for peer in participants {
        let verifier = Ed25519::from_seed(peer).public_key();
        tracing::info!(key = hex(&verifier), "registered authorized key",);
        validators.push(verifier);
    }

    // Configure bootstrappers (if provided)
    let bootstrappers = matches.get_many::<String>("bootstrappers");
    let mut bootstrapper_identities = Vec::new();
    if let Some(bootstrappers) = bootstrappers {
        for bootstrapper in bootstrappers {
            let parts = bootstrapper.split('@').collect::<Vec<&str>>();
            let bootstrapper_key = parts[0]
                .parse::<u64>()
                .expect("Bootstrapper key not well-formed");
            let verifier = Ed25519::from_seed(bootstrapper_key).public_key();
            let bootstrapper_address =
                SocketAddr::from_str(parts[1]).expect("Bootstrapper address not well-formed");
            bootstrapper_identities.push((verifier, bootstrapper_address));
        }
    }

    // Configure storage directory
    let storage_directory = matches
        .get_one::<String>("storage-dir")
        .expect("Please provide storage directory");

    // Configure threshold
    let threshold = quorum(validators.len() as u32).expect("Threshold not well-formed");
    let identity = matches
        .get_one::<String>("identity")
        .expect("Please provide identity");
    let identity = from_hex(identity).expect("Identity not well-formed");
    let identity: Poly<group::Public> =
        Poly::deserialize(&identity, threshold).expect("Identity not well-formed");
    let public = poly::public(&identity);
    let share = matches
        .get_one::<String>("share")
        .expect("Please provide share");
    let share = from_hex(share).expect("Share not well-formed");
    let share = group::Share::deserialize(&share).expect("Share not well-formed");

    // Initialize runtime
    let runtime_cfg = tokio::Config {
        storage_directory: storage_directory.into(),
        ..Default::default()
    };
    let (executor, runtime) = Executor::init(runtime_cfg.clone());


    // Configure network
    let p2p_cfg = authenticated::Config::aggressive(
        signer.clone(),
        &union(APPLICATION_NAMESPACE, P2P_SUFFIX),
        Arc::new(Mutex::new(Registry::default())),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port),
        bootstrapper_identities.clone(),
        1024 * 1024, // 1MB
    );

    // Start runtime
    executor.start(async move {
        // Setup p2p
        let (mut network, mut oracle) = authenticated::Network::new(runtime.clone(), p2p_cfg);

        // Provide authorized peers
        //
        // In a real-world scenario, this would be updated as new peer sets are created (like when
        // the composition of a validator set changes).
        oracle.register(0, validators.clone()).await;

        // Register consensus channels
        //
        // If you want to maximize the number of views per second, increase the rate limit
        // for this channel.
        let (voter_sender, voter_receiver) = network.register(
            0,
            Quota::per_second(NonZeroU32::new(10).unwrap()),
            256, // 256 messages in flight
            Some(3),
        );
        let (resolver_sender, resolver_receiver) = network.register(
            1,
            Quota::per_second(NonZeroU32::new(10).unwrap()),
            256, // 256 messages in flight
            Some(3),
        );

        // Register chatter channels
        let (chatter_p2p_sender, chatter_p2p_reciever) = network.register(
            2,
            Quota::per_second(NonZeroU32::new(10).unwrap()),
            256, // 256 messages in flight
            Some(3),
        );

        // Initialize storage
        let journal = Journal::init(
            runtime.clone(),
            journal::Config {
                registry: Arc::new(Mutex::new(Registry::default())),
                partition: String::from("log"),
            },
        )
        .await
        .expect("Failed to initialize journal");

        // Initialize chatter
        let (chatter_actor, chatter_mailbox) = Actor::new();
        // Initialize application
        let consensus_namespace = union(APPLICATION_NAMESPACE, CONSENSUS_SUFFIX);
        let hasher = Sha256::default();
        let prover: Prover<Sha256> = Prover::new(public, &consensus_namespace);
        let (application, supervisor, mailbox) = application::Application::new(
            runtime.clone(),
            application::Config {
                prover,
                hasher: hasher.clone(),
                mailbox_size: 1024,
                identity,
                participants: validators.clone(),
                share,
            },
            chatter_mailbox.clone(),
        );

        let (p2p_actor, p2p_mailbox) = P2PActor::new(chatter_mailbox, supervisor.clone());
        let chatter_supervisor = supervisor.clone();

        // Initialize consensus
        let engine = Engine::new(
            runtime.clone(),
            journal,
            threshold_simplex::Config {
                crypto: signer.clone(),
                hasher,
                automaton: mailbox.clone(),
                relay: mailbox.clone(),
                committer: mailbox,
                supervisor,
                registry: Arc::new(Mutex::new(Registry::default())),
                namespace: consensus_namespace,
                mailbox_size: 1024,
                replay_concurrency: 1,
                leader_timeout: Duration::from_secs(1),
                notarization_timeout: Duration::from_secs(2),
                nullify_retry: Duration::from_secs(10),
                fetch_timeout: Duration::from_secs(1),
                activity_timeout: 10,
                max_fetch_count: 32,
                max_fetch_size: 1024 * 512,
                fetch_concurrent: 2,
                fetch_rate_per_peer: Quota::per_second(NonZeroU32::new(1).unwrap()),
            },
        );

        // Start consensus
        runtime.spawn("network", network.run());
        runtime.spawn(
            "engine",
            engine.run(
                (voter_sender, voter_receiver),
                (resolver_sender, resolver_receiver),
            ),
        );

        
        runtime.spawn("chatter", chatter_actor.run(p2p_mailbox, chatter_supervisor, signer.clone()));

        runtime.spawn("p2p", p2p_actor.run(chatter_p2p_sender, chatter_p2p_reciever));

        // Block on application
        application.run().await;
    });
}
