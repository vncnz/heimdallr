use std::fs;
use rand::Rng;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub enum FrameColor {
    None,
    Rgba(f64, f64, f64, f64),
    // Random,
    WorstResource,
}

#[derive(Debug, Clone)]
pub enum ClockCfg {
    None,
    Clock1,
    Clock2,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub frame_color: FrameColor,
    pub show_clock: ClockCfg,
    pub show_always_bluetooth: bool
    // pub border_width: u32,
}

#[derive(Debug, Deserialize)]
struct RawConfig {
    frame_color: Option<serde_json::Value>,
    show_clock: Option<serde_json::Value>,
    show_always_bluetooth: Option<bool>
    // border_width: Option<u32>,
}

impl FrameColor {
    fn from_json(value: Option<serde_json::Value>) -> Self {
        match value {
            Some(serde_json::Value::Null) => FrameColor::None,
            None => FrameColor::None,

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
                    eprintln!("Unrecognized value in framecolor config: {:?}", s);
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
                    eprintln!("Invalid frame_color array, using None");
                    FrameColor::None
                }
            }

            _ => {
                eprintln!("Invalid frame_color in JSON configuration {:?}", value);
                FrameColor::None
            }
        }
    }
}

impl ClockCfg {
    fn from_json(value: Option<serde_json::Value>) -> Self {
        match value {
            Some(serde_json::Value::Null) => ClockCfg::None,
            None => ClockCfg::None,

            Some(serde_json::Value::String(s)) => match s.as_str() {
                "clock1" => { ClockCfg::Clock1 },
                "clock2" => { ClockCfg::Clock2 },
                _ => {
                    eprintln!("Unrecognized value in clock config: {:?}. Accepted types are \"clock1\", \"clock2\", null", s);
                    ClockCfg::None
                }
            },

            _ => {
                eprintln!("Invalid clock value in JSON configuration {:?}. Accepted types are \"clock1\", \"clock2\", null", value);
                ClockCfg::None
            }
        }
    }
}

impl Config {
    pub fn load_from_file(path: &str) -> Self {
        let expanded_path = shellexpand::tilde(path);
        let data = fs::read_to_string(expanded_path.as_ref())
            .unwrap_or_else(|_| {
                eprintln!("Cannot read configuration file: {}", path);
                "{}".to_string()
            });

        let raw: RawConfig = serde_json::from_str(&data).unwrap_or_else(|_| {
            eprintln!("Config JSON non valido, uso valori di default");
            RawConfig {
                frame_color: None,
                show_clock: None,
                show_always_bluetooth: None
                // border_width: None,
            }
        });

        Config {
            frame_color: FrameColor::from_json(raw.frame_color),
            show_clock: ClockCfg::from_json(raw.show_clock), // raw.show_clock.unwrap_or(true),
            show_always_bluetooth: raw.show_always_bluetooth.unwrap_or(true)
            // border_width: raw.border_width.unwrap_or(2),
        }
    }
}
