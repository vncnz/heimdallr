use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::mpsc::Sender;
use std::thread;

use std::ffi::CString;

use libc;

fn create_fifo(path: &str) -> std::io::Result<()> {
    let c_path = CString::new(path).unwrap();
    let mode = 0o600; // rw------- (solo utente)
    let ret = unsafe { libc::mkfifo(c_path.as_ptr(), mode as libc::mode_t) };
    if ret == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

pub fn start_command_listener(tx: Sender<String>, fifo_path: &str) -> std::io::Result<()> {
    let path = Path::new(fifo_path);

    // Se esiste un file precedente (vecchia FIFO o file normale), lo rimuoviamo
    if path.exists() {
        fs::remove_file(path)?;
    }

    // Creiamo la FIFO (named pipe)
    create_fifo(fifo_path)?;

    // Lanciamo un thread dedicato alla lettura
    let path_owned = fifo_path.to_string();
    thread::spawn(move || {
        loop {
            // Apriamo la FIFO in sola lettura (bloccante finché qualcuno scrive)
            let file = match fs::File::open(&path_owned) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("Error opening FIFO: {e}");
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    continue;
                }
            };

            let reader = BufReader::new(file);

            // Leggiamo riga per riga e inviamo al channel
            for line in reader.lines() {
                match line {
                    Ok(cmd) if !cmd.trim().is_empty() => {
                        if let Err(e) = tx.send(cmd.clone()) {
                            eprintln!("Failed to send command '{cmd}': {e}");
                            break;
                        }
                    }
                    Ok(_) => {} // linea vuota → ignora
                    Err(e) => eprintln!("FIFO read error: {e}"),
                }
            }

            // Quando la FIFO viene chiusa dal lato scrittore, riapriamo il loop
        }
    });

    Ok(())
}
