use tokio::sync::mpsc::{unbounded_channel, UnboundedSender, UnboundedReceiver};
use serde::{Serialize, Deserialize};

use std::convert::TryInto;

use crate::protocol::*;

const BOOTSTRAP_IP: &'static str = "185.25.116.107";
const BOOTSTRAP_PORT: u16 = 33445;
const BOOTSTRAP_KEY: &'static str =
    "DA4E4ED4B697F2E9B000EEFE3A34B554ACD3F45F5C96EAEA2516DD7FF9AF7B43";

const CLIENT_NAME: &'static str = "ws-client";

pub struct ToxHandle {
    pub request_tx: std::sync::mpsc::Sender<Request>,
    pub answer_rx: UnboundedReceiver<Answer>,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum Answer {
    Response(Response),
    Event(Event),
}

fn run_request(tox: &mut rstox::core::Tox, request: &Request) -> Option<Response> {
    use Request as R;

    match request {
        R::Info => {
            let name = tox.get_name();
            let tox_id = format!("{}", tox.get_address());
            let response = Response::Info {
                name, tox_id
            };

            return Some(response)
        },
        R::AddFriend { tox_id, message } => {
            let address: rstox::core::Address = tox_id.parse().ok()?;

            let response = tox.add_friend(&address, &message)
                .map(|()| Response::Ok)
                .unwrap_or_else(|e| Response::AddFriendError {
                    error: e.try_into().expect("unexpected friend add error")
                });

            return Some(response)
        },
        R::SendFriendMessage { friend, kind, message } => {
            let response = tox.send_friend_message(*friend, (*kind).into(), message)
                .map(|_| Response::Ok)
                .unwrap_or_else(|e| Response::SendFriendMessageError {
                    error: e.try_into().expect("unexpected send friend message error")
                });

            return Some(response)
        },
        _ => drop(dbg!(request)),
    }

    None
}

fn tox_loop(request_rx: std::sync::mpsc::Receiver<Request>, mut answer_tx: UnboundedSender<Answer>) {
    use rstox::core::{Tox, ToxOptions};

    let mut tox = Tox::new(ToxOptions::new(), None).unwrap();

    tox.set_name(CLIENT_NAME).unwrap();
    let bootstrap_key = BOOTSTRAP_KEY.parse().unwrap();
    tox.bootstrap(BOOTSTRAP_IP, BOOTSTRAP_PORT, bootstrap_key).unwrap();

    dbg!(format!("Server Tox ID: {}", tox.get_address());

    loop {
        if let Ok(req) = request_rx.try_recv() {
            if let Some(resp) = run_request(&mut tox, &req) {
                drop(answer_tx.try_send(Answer::Response(resp)))
            }
        }

        for ev in tox.iter() {
            if let Some(e) = crate::protocol::Event::from_tox_event(&ev) {
                drop(answer_tx.try_send(Answer::Event(e)))
            }
            else {
                dbg!(ev);
            }
        }

        tox.wait();
    }
}

pub fn spawn_tox() -> ToxHandle {
    use std::sync::mpsc;

    let (request_tx, request_rx) = mpsc::channel();
    let (answer_tx, answer_rx) = unbounded_channel();

    std::thread::spawn(move || tox_loop(request_rx, answer_tx));

    ToxHandle {
        request_tx, answer_rx
    }
}
