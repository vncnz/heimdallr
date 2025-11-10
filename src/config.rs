use std::fs;
use rand::Rng;
use serde::{Deserialize};
use shellexpand;

#[derive(Debug, Clone)]
pub enum FrameColor {
    None,
    Random,
    Resources,
    Rgba(f64, f64, f64, f64),
}

#[derive(Debug, Deserialize, Default)]
pub struct Config {
    // per ora vuota, ma potrai aggiungere:
    // pub colors: IconColors,
    // pub update_interval: u64,
    pub frame_color: FrameColor,
    pub show_clock: bool
}

impl Default for FrameColor {
    fn default() -> Self {
        FrameColor::Rgba(0.5, 0.5, 0.5, 1.0)
    }
}

impl Config {
    pub fn load_from_file(path: &str) -> Self {
        let expanded_path = shellexpand::tilde(path);
        match fs::read_to_string(expanded_path.as_ref()) {
            Ok(data) => match serde_json::from_str::<Config>(&data) {
                Ok(mut cfg) => {
                    match &cfg.frame_color {
                        FrameColor::Random => {
                            let mut rng = rand::rng();
                            cfg.frame_color = FrameColor::Rgba(
                                rng.random(),
                                rng.random(),
                                rng.random(),
                                1.0,
                            );
                        }
                        _ => {}
                    }
                    cfg
                },
                Err(e) => {
                    eprintln!("Errore nel parsing del file di configurazione: {e}");
                    Self::default()
                }
            },
            Err(err) => {
                eprintln!("File di configurazione non trovato, uso valori di default");
                eprintln!("{}", err);
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

impl<'de> Deserialize<'de> for FrameColor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let lower = s.trim().to_ascii_lowercase();

        match lower.as_str() {
            "none" => Ok(FrameColor::None),
            "random" => Ok(FrameColor::Random),
            "resources" => Ok(FrameColor::Resources),
            _ => {
                let parts: Vec<f64> = lower
                    .split(',')
                    .map(|p| p.trim().parse::<f64>())
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(serde::de::Error::custom)?;

                if parts.len() == 4 {
                    Ok(FrameColor::Rgba(parts[0], parts[1], parts[2], parts[3]))
                } else {
                    Err(serde::de::Error::custom(
                        "Expected 4 comma-separated values for RGBA, or none, or resources, or random",
                    ))
                }
            }
        }
    }
}

impl FrameColor {
    pub fn to_rgba(&self) -> (f64, f64, f64, f64) {
        match self {
            FrameColor::None => (0.0, 0.0, 0.0, 0.0),
            FrameColor::Random => {
                use rand::Rng;
                let mut rng = rand::rng();
                (rng.random(), rng.random(), rng.random(), 1.0)
            }
            FrameColor::Resources => (0.5, 0.5, 0.5, 1.0), // TODO
            FrameColor::Rgba(r, g, b, a) => (*r, *g, *b, *a),
        }
    }
}