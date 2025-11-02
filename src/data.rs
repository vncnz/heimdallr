use std::{
    net::Shutdown,
    os::unix::net::UnixDatagram,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct SharedState {
    pub volume: u8,
    pub muted: bool,
}

pub fn start_socket_listener(state: Arc<Mutex<SharedState>>) {
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
                    if let Ok(new_data) = serde_json::from_str::<SharedState>(&msg) {
                        if let Ok(mut data) = state.lock() {
                            *data = new_data;
                        }
                    } else {
                        eprintln!("Messaggio JSON non valido: {msg}");
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

fn main() {
    let state = Arc::new(Mutex::new(SharedState {
        volume: 50,
        muted: false,
    }));

    // Avvia listener in background
    start_socket_listener(Arc::clone(&state));

    // Simuliamo il rendering loop
    loop {
        {
            let data = state.lock().unwrap();
            println!("Rendering con volume={} muted={}", data.volume, data.muted);
        }
        thread::sleep(Duration::from_secs(2));
    }
}
