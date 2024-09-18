use std::sync::mpsc;

pub enum LogMessage {
    Message(String),
    Shutdown(())
}

pub struct Log {
    rx: mpsc::Receiver<LogMessage>
}

impl Log {
    pub fn new() -> (Self, mpsc::Sender<LogMessage>) {
        let (ltx, lrx) = mpsc::channel();
        (Log { rx: lrx }, ltx)
    }

    pub fn run(&self) {
        for msg in self.rx.iter() {
            match msg {
                LogMessage::Message(s) => println!("procyon: {}", s),
                LogMessage::Shutdown(_) => break,
            }
        }
    }
}

pub fn log(tx: &mpsc::Sender<LogMessage>, s: &str) {
    tx.send(LogMessage::Message(s.to_string())).unwrap()
}

pub fn stop(tx: &mpsc::Sender<LogMessage>) {
    tx.send(LogMessage::Shutdown(())).unwrap()
}
