use crate::log;
use crate::socket;
use std::sync::mpsc;
use std::thread;
use std::time;

pub enum IrcMessage {
    Message(String),
    Control(()),
    Register(()),
    Shutdown(()),
}

#[derive(Debug, PartialEq)]
enum ChannelStatus {
    NotJoined,
    Attempting,
    Joined,
}

struct Source {
    nick: Option<String>,
}

impl Source {
    fn new(source: String) -> Self {
        let mut s = source.clone();
        if s.chars().next().unwrap() != ':' {
            return Source { nick: None };
        }
        s.remove(0);
        let parts = s.split("!").collect::<Vec<&str>>();
        if parts.len() != 2 {
            return Source { nick: None };
        }
        Source {
            nick: Some(parts[0].to_string()),
        }
    }

    fn is_me(&self) -> bool {
        self.nick.as_ref().unwrap() == "procyon"
    }
}

pub struct Irc {
    irx: mpsc::Receiver<IrcMessage>,
    itx: mpsc::Sender<IrcMessage>,
    stx: mpsc::Sender<socket::SocketMessage>,
    ltx: mpsc::Sender<log::LogMessage>,

    registered: bool,
    channel: String,
    channel_status: ChannelStatus,
}

impl Irc {
    pub fn new(
        stx: mpsc::Sender<socket::SocketMessage>,
        ltx: mpsc::Sender<log::LogMessage>,
        channel: String,
    ) -> (Self, mpsc::Sender<IrcMessage>) {
        let (itx, irx) = mpsc::channel();
        (
            Irc {
                irx: irx,
                itx: itx.clone(),
                stx: stx,
                ltx: ltx,

                registered: false,
                channel: channel,
                channel_status: ChannelStatus::NotJoined,
            },
            itx,
        )
    }

    fn send_registration(&self) {
        self.stx
            .send(socket::SocketMessage::Output("NICK procyon".to_string()))
            .unwrap();
        self.stx
            .send(socket::SocketMessage::Output(
                "USER procyon @ procyon :procyon".to_string(),
            ))
            .unwrap();
    }

    pub fn channel(&mut self) {
        if self.channel_status != ChannelStatus::NotJoined {
            return;
        }
        log::log(
            &self.ltx,
            format!("attemping to join {}", self.channel).as_str(),
        );
        self.channel_status = ChannelStatus::Attempting;
        let j = format!("JOIN :{}", self.channel);
        self.stx.send(socket::SocketMessage::Output(j)).unwrap();
    }

    pub fn run(&mut self) {
        loop {
            let msg = self.irx.recv().unwrap();
            match msg {
                IrcMessage::Message(s) => {
                    let parts = s.split(" ").collect::<Vec<&str>>();
                    if parts.len() < 2 {
                        continue;
                    }

                    if parts[0] == "PING" {
                        let buf = format!("PONG {}", parts[1]);
                        self.stx.send(socket::SocketMessage::Output(buf)).unwrap();
                        continue;
                    }

                    let source = Source::new(parts[0].to_string());
                    match parts[1] {
                        "001" => {
                            log::log(&self.ltx, "registration successful");
                            self.registered = true;
                        }
                        "JOIN" => {
                            if source.is_me() {
                                self.channel_status = ChannelStatus::Joined;
                                log::log(
                                    &self.ltx,
                                    format!("{} marked as joined", self.channel).as_str(),
                                );
                            }
                        }
                        "KICK" => {
                            if parts[3] == "procyon" {
                                self.channel_status = ChannelStatus::NotJoined;
                            }
                        }
                        "PRIVMSG" => {
                            if parts[3] == ":pquit" {
                                self.stx
                                    .send(socket::SocketMessage::WantDisconnected(()))
                                    .unwrap();
                            }
                        }
                        _ => {}
                    }
                }
                IrcMessage::Control(_) => {
                    log::log(&self.ltx, "received control message");
                    schedule_control(&self.itx);
                    if !self.registered {
                        continue;
                    }
                    self.channel();
                }
                IrcMessage::Register(_) => {
                    self.send_registration();
                }
                IrcMessage::Shutdown(_) => {
                    log::log(&self.ltx, "returning from IRC thread");
                    break;
                }
            }
        }
    }
}

pub fn schedule_control(itx: &mpsc::Sender<IrcMessage>) {
    let t = itx.clone();
    thread::spawn(move || {
        thread::sleep(time::Duration::from_secs(5));
        let _ = t.send(IrcMessage::Control(()));
    });
}
