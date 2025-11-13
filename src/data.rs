use std::sync::mpsc::{Sender,Receiver,channel};
use std::os::unix::net::UnixStream;
use std::io::Read;
use serde::Deserialize;

#[derive(Default, Deserialize, Debug)]
pub struct PartialMsg {
    pub resource: String,
    pub warning: f64,
    pub icon: String,
    pub data: Option<serde_json::Value>,
}
/*
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
*/


pub struct HeimdallrSocket {
    stream: Option<UnixStream>,
    path: &'static str,
    tx: Sender<PartialMsg>,
    pub rx: Receiver<PartialMsg>,
    recv_buf: String,
}

impl HeimdallrSocket {
    pub fn new(path: &'static str) -> Self {
        let (tx, rx) = channel();
        Self { stream: None, path, tx, rx, recv_buf: "".to_string() }
    }

    pub fn try_connect(&mut self) {
        if self.stream.is_some() {
            return;
        }

        match UnixStream::connect(self.path) {
            Ok(mut stream) => {
                println!("Ratatoskr connected");
                stream.set_nonblocking(true).ok();
                self.stream = Some(stream);
                let _ = self.tx.send(PartialMsg {
                    resource: "ratatoskr".to_string(),
                    icon: "".into(),
                    warning: 0.0,
                    data: None
                });
            }
            Err(_) => {
                // non connesso, riproveremo
            }
        }
    }

    pub fn poll_messages(&mut self) {
        if let Some(stream) = self.stream.as_mut() {
            let mut buf = [0u8; 4096];
            match stream.read(&mut buf) {
                Ok(0) => {
                    println!("Ratatoskr disconnected");
                    let _ = self.tx.send(PartialMsg {
                        resource: "ratatoskr".to_string(),
                        icon: "".into(),
                        warning: 1.0,
                        data: None,
                    });
                    self.stream = None;
                }
                Ok(n) => {
                    if let Ok(chunk) = std::str::from_utf8(&buf[..n]) {
                        // aggiunge al buffer cumulativo
                        self.recv_buf.push_str(chunk);

                        // finché trovi un newline, estrai un messaggio completo
                        while let Some(pos) = self.recv_buf.find('\n') {
                            let msg = self.recv_buf[..pos].trim();
                            if !msg.is_empty() {
                                if let Ok(data) = serde_json::from_str::<PartialMsg>(msg) {
                                    let _ = self.tx.send(data);
                                } else {
                                    eprintln!("Invalid JSON fragment: {msg}");
                                }
                            }
                            // rimuove la parte processata
                            self.recv_buf.drain(..=pos);
                        }
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // nessun dato nuovo
                }
                Err(e) => {
                    eprintln!("Errore socket: {e}");
                    self.stream = None;
                }
            }
        } else {
            self.try_connect();
        }
    }

}