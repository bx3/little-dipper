#[doc(hidden)]
pub mod wire {
    include!(concat!(env!("OUT_DIR"), "/wire.rs"));
}
#[doc(hidden)]
pub mod application;
#[doc(hidden)]
pub const APPLICATION_NAMESPACE: &[u8] = b"_LITTLEDIPPER_CHAT";
#[doc(hidden)]
pub const P2P_SUFFIX: &[u8] = b"_P2P";
#[doc(hidden)]
pub const CONSENSUS_SUFFIX: &[u8] = b"_CONSENSUS";
#[doc(hidden)]
pub const APPLICATION_P2P_NAMESPACE: &[u8] = b"_LITTLEDIPPER_CHAT_P2P";
