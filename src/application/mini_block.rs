/// A single mini block from a chatter
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct MiniBlock {
    pub view: u64,
    pub data: Vec<u8>,
    pub sig: Vec<u8>,
}

/// Data ready to become a proposal
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct MiniBlocks {
    pub mini_blocks: Vec<MiniBlock>,
}



