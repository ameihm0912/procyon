use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::net::TcpStream;
use std::sync::mpsc;
use std::thread;
use std::time;

mod config;
mod irc;
mod log;
mod socket;

#[derive(PartialEq)]
enum Status {
    Disconnected,
    Connected,
}

struct State<'a> {
    cfg: &'a config::Config,
    status: Status,
    logtx: &'a mpsc::Sender<log::LogMessage>,

    conn: Option<std::net::TcpStream>,
    conn_attempts: i32,
}

impl<'a> State<'a> {
    fn new(cfg: &'a config::Config, logtx: &'a mpsc::Sender<log::LogMessage>) -> Self {
        State {
            cfg: cfg,
            status: Status::Disconnected,
            logtx: logtx,
            conn: None,
            conn_attempts: 0,
        }
    }
}

fn net_conn(state: &mut State) {
    if state.status != Status::Disconnected {
        return;
    }
    log::log(
        state.logtx,
        format!("attempting connection to {}", state.cfg.socket_addr).as_str(),
    );

    match TcpStream::connect(&state.cfg.socket_addr) {
        Ok(fd) => {
            log::log(state.logtx, "TCP connection established");
            state.conn = Some(fd);
            state.status = Status::Connected;
        }
        Err(e) => {
            log::log(state.logtx, format!("connection failed: {}", e).as_str());
        }
    }
}

fn socket_handler(state: &mut State) -> bool {
    let (stx, srx) = mpsc::channel();
    let readtx = stx.clone();
    let readsock = state.conn.as_ref().unwrap().try_clone().unwrap();
    thread::spawn(move || {
        let mut rdr = BufReader::new(readsock);
        let mut input = String::new();
        loop {
            match rdr.read_line(&mut input) {
                Ok(l) => {
                    if l == 0 {
                        readtx
                            .send(socket::SocketMessage::Disconnected(()))
                            .unwrap();
                        break;
                    }
                    readtx
                        .send(socket::SocketMessage::Input(
                            input.clone().replace("\n", "").replace("\r", ""),
                        ))
                        .unwrap();
                    input.clear();
                }
                Err(_) => {
                    readtx.send(socket::SocketMessage::Error(())).unwrap();
                    break;
                }
            }
        }
    });

    let (mut ircproc, itx) =
        irc::Irc::new(stx.clone(), state.logtx.clone(), state.cfg.channel.clone());
    let irct = thread::spawn(move || ircproc.run());
    itx.send(irc::IrcMessage::Register(())).unwrap();

    let mut should_exit = false;

    irc::schedule_control(&itx);
    for msg in srx.iter() {
        match msg {
            socket::SocketMessage::Input(s) => {
                log::log(state.logtx, format!("socket input: {}", s).as_str());
                itx.send(irc::IrcMessage::Message(s)).unwrap();
            }
            socket::SocketMessage::Output(s) => {
                log::log(state.logtx, format!("socket output: {}", s).as_str());
                write!(state.conn.as_ref().unwrap(), "{}\r\n", s).unwrap();
            }
            socket::SocketMessage::Error(_) => {
                log::log(state.logtx, "socket error, returning from socket handler");
                break;
            }
            socket::SocketMessage::WantDisconnected(_) => {
                should_exit = true;
                log::log(state.logtx, "pquit, sending QUIT command");
                write!(state.conn.as_ref().unwrap(), "QUIT :Leaving\r\n").unwrap();
            }
            socket::SocketMessage::Disconnected(_) => {
                log::log(
                    state.logtx,
                    "socket disconnected, returning from socket handler",
                );
                itx.send(irc::IrcMessage::Shutdown(())).unwrap();
                break;
            }
        }
    }
    irct.join().unwrap();
    should_exit
}

fn run(cfg: &config::Config) {
    let (logger, logtx) = log::Log::new();
    let logt = thread::spawn(move || logger.run());
    log::log(&logtx, "started logging thread");

    let mut state = State::new(cfg, &logtx);

    loop {
        net_conn(&mut state);
        match state.status {
            Status::Disconnected => {
                state.conn_attempts += 1;
                if state.conn_attempts >= 5 {
                    break;
                }
                log::log(&logtx, "status is disconnected, sleeping for network retry");
                thread::sleep(time::Duration::from_secs(5));
            }
            Status::Connected => {
                state.conn_attempts = 0;
                if socket_handler(&mut state) {
                    break;
                }
                state.status = Status::Disconnected;
                thread::sleep(time::Duration::from_secs(5));
            }
        }
    }
    log::log(&logtx, "shutting down logging thread");
    log::stop(&logtx);
    logt.join().unwrap();
}

fn main() {
    let cfg = config::Config::new().unwrap_or_else(|e| {
        eprintln!("error creating configuration: {}", e);
        std::process::exit(1);
    });
    run(&cfg);
}
