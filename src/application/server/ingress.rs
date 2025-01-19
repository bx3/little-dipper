
pub enum Message {
    PutMiniBlock {
        view: u64,
        mini_blocks: Vec<MiniBlock>,
        response: oneshot::Sender<bool>,
    },
    GetMiniBlocks {
        view: u64,
        response: oneshot::Sender<Vec<MiniBlock>>,
    },
    SendMiniBlock {
        view: u64,
        response: oneshot::Sender<bool>,
    }
}
