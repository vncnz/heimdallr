use std::fs;
use rand::Rng;
use serde::Deserialize;

#[derive(Debug)]
pub enum FrameColor {
    None,
    Rgba(f64, f64, f64, f64),
    Random,
    WorstResource,
}

#[derive(Debug)]
pub struct Config {
    pub frame_color: FrameColor,
    pub show_clock: bool,
    // pub border_width: u32,
}

#[derive(Debug, Deserialize)]
struct RawConfig {
    frame_color: Option<serde_json::Value>,
    show_clock: Option<bool>,
    // border_width: Option<u32>,
}

impl FrameColor {
    fn from_json(value: Option<serde_json::Value>) -> Self {
        match value {
            Some(serde_json::Value::Null) => FrameColor::None,

            Some(serde_json::Value::String(s)) => match s.as_str() {
                "random" => {
                    let mut rng = rand::rng();
                    FrameColor::Rgba(
                        rng.random(),
                        rng.random(),
                        rng.random(),
                        1.0,
                    )
                },
                "worst-resource" => FrameColor::WorstResource,
                _ => {
                    eprintln!("⚠️  Valore frame_color non riconosciuto: {:?}", s);
                    FrameColor::None
                }
            },

            Some(serde_json::Value::Array(arr)) if arr.len() == 4 => {
                let vals: Vec<f64> = arr
                    .iter()
                    .filter_map(|v| v.as_f64().map(|x| x as f64))
                    .collect();
                if vals.len() == 4 {
                    FrameColor::Rgba(vals[0], vals[1], vals[2], vals[3])
                } else {
                    eprintln!("frame_color array non valido, uso None");
                    FrameColor::None
                }
            }

            _ => {
                eprintln!("Tipo di frame_color non valido nel JSON {:?}", value);
                FrameColor::None
            }
        }
    }
}

impl Config {
    pub fn load_from_file(path: &str) -> Self {
        let expanded_path = shellexpand::tilde(path);
        let data = fs::read_to_string(expanded_path.as_ref())
            .unwrap_or_else(|_| {
                eprintln!("Impossibile leggere il file di configurazione: {}", path);
                "{}".to_string()
            });

        let raw: RawConfig = serde_json::from_str(&data).unwrap_or_else(|_| {
            eprintln!("Config JSON non valido, uso valori di default");
            RawConfig {
                frame_color: None,
                show_clock: None,
                // border_width: None,
            }
        });

        Config {
            frame_color: FrameColor::from_json(raw.frame_color),
            show_clock: raw.show_clock.unwrap_or(true),
            // border_width: raw.border_width.unwrap_or(2),
        }
    }
}
