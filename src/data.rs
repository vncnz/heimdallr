use std::{
    os::unix::net::UnixDatagram,
    sync::{Arc, Mutex},
    sync::mpsc::{Sender},
    thread,
    time::Duration,
};

use serde::Deserialize;

#[derive(Default, Deserialize)]
pub struct PartialMsg {
    pub resource: String,
    pub warning: f64,
    pub data: Option<serde_json::Value>,
}

pub fn start_socket_listener(state: Arc<Mutex<PartialMsg>>, tx: Sender<PartialMsg>) {
    thread::spawn(move || {
        let sock_path = "/tmp/ratatoskr.sock";
        let _ = std::fs::remove_file(sock_path); // evita Address in use
        let listener = match UnixDatagram::bind(sock_path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Impossibile creare socket: {e}");
                return;
            }
        };

        let mut buf = [0u8; 4096];
        loop {
            match listener.recv(&mut buf) {
                Ok(size) => {
                    let msg = String::from_utf8_lossy(&buf[..size]);
                    if let Ok(new_data) = serde_json::from_str::<PartialMsg>(&msg) {
                        if let Ok(mut data) = state.lock() {
                            tx.send(PartialMsg { resource: new_data.resource.clone(), warning: new_data.warning, data: None }).ok();
                            *data = new_data;
                            // println!("Updated! {}", msg);
                        }
                    } else {
                        // eprintln!("Messaggio JSON non valido: {msg}");
                    }
                }
                Err(e) => {
                    eprintln!("Errore socket: {e}");
                    thread::sleep(Duration::from_secs(1));
                }
            }
        }
    });
}