use std::fs;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    // per ora vuota, ma potrai aggiungere:
    // pub colors: IconColors,
    // pub update_interval: u64,
}

impl Config {
    pub fn load_from_file(path: &str) -> Self {
        match fs::read_to_string(path) {
            Ok(data) => match serde_json::from_str::<Config>(&data) {
                Ok(cfg) => cfg,
                Err(e) => {
                    eprintln!("Errore nel parsing del file di configurazione: {e}");
                    Self::default()
                }
            },
            Err(_) => {
                eprintln!("File di configurazione non trovato, uso valori di default");
                Self::default()
            }
        }
    }

    /*
    In the future:
    - Update configuration on disk with serde_json::to_writer_pretty()
    - Add a watcher with notify?
    */
}
