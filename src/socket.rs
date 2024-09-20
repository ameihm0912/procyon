pub enum SocketMessage {
    Input(String),
    Output(String),
    Disconnected(()),
    WantDisconnected(()),
    Error(()),
}
