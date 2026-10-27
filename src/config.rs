use std::fs;
use rand::Rng;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxStyle {
    Pill,
    Notch,
}

impl Default for BoxStyle {
    fn default() -> Self {
        Self::Pill
    }
}

impl<'de> Deserialize<'de> for BoxStyle {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;

        match value {
            serde_json::Value::Null => Ok(Self::Pill),
            serde_json::Value::String(s) if s == "pill" => Ok(Self::Pill),
            serde_json::Value::String(s) if s == "notch" => Ok(Self::Notch),
            _ => {
                eprintln!("Invalid boxstyle in JSON configuration: {:?}; using pill", value);
                Ok(Self::Pill)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum FrameColor {
    None,
    Rgba(f64, f64, f64, f64),
    WorstResource,
}

impl Default for FrameColor {
    fn default() -> Self {
        Self::None
    }
}

impl<'de> Deserialize<'de> for FrameColor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;

        match value {
            serde_json::Value::Null => Ok(Self::None),
            serde_json::Value::String(s) => match s.as_str() {
                "random" => {
                    let mut rng = rand::rng();
                    Ok(Self::Rgba(
                        rng.random(),
                        rng.random(),
                        rng.random(),
                        1.0,
                    ))
                }
                "worst-resource" => Ok(Self::WorstResource),
                _ => {
                    eprintln!("Unrecognized value in framecolor config: {:?}", s);
                    Ok(Self::None)
                }
            },
            serde_json::Value::Array(arr) if arr.len() == 4 => {
                let vals: Vec<f64> = arr
                    .iter()
                    .filter_map(|v| v.as_f64().map(|x| x as f64))
                    .collect();
                if vals.len() == 4 {
                    Ok(Self::Rgba(vals[0], vals[1], vals[2], vals[3]))
                } else {
                    eprintln!("Invalid color array, using None");
                    Ok(Self::None)
                }
            }
            _ => {
                eprintln!("Invalid color in JSON configuration {:?}", value);
                Ok(Self::None)
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Config {
    //#[serde(default)]
    // pub frame_color: FrameColor,
    #[serde(default)]
    pub pill_border_color: FrameColor,
    #[serde(default = "default_show_devices_battery_max_level")]
    pub show_devices_battery_max_level: f64,
    #[serde(default)]
    pub hide_missing_ratatoskr: bool,
    #[serde(default)]
    pub show_watts: bool,
    #[serde(default)]
    pub style: BoxStyle,
}

fn default_show_devices_battery_max_level() -> f64 {
    80.0
}

impl Config {
    pub fn load_from_file(path: &str) -> Self {
        let expanded_path = shellexpand::tilde(path);
        let data = fs::read_to_string(expanded_path.as_ref()).unwrap_or_else(|_| {
            eprintln!("Cannot read configuration file: {}", path);
            "{}".to_string()
        });

        serde_json::from_str(&data).unwrap_or_else(|err| {
            eprintln!("Config JSON invalid, using default values: {err}");
            Self::default()
        })
    }
}
