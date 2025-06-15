// src/main.rs

use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;

use image::{ImageBuffer, Rgba};
use serde::Deserialize;
use serde_json::Value;
use tiny_skia::*;
use fontdue::Font;

#[derive(Default, Deserialize, Debug)]
pub struct RamStats {
    pub total_memory: u64,
    pub used_memory: u64,
    pub total_swap: u64,
    pub used_swap: u64,
    pub mem_percent: u64,
    pub swap_percent: u64,
    pub mem_color: String,
    pub swap_color: String
}

#[derive(Default, Deserialize, Debug)]
pub struct DiskStats {
    pub total_size: u64,
    pub used_size: u64,
    pub used_percent: u64,
    pub color: String
}

#[derive(Default, Deserialize, Debug)]
pub struct TempStats {
    pub sensor: String,
    pub value: f32,
    pub color: String,
    pub icon: String
}

#[derive(Default, Deserialize, Debug)]
pub struct WeatherStats {
    pub icon: String,
    pub icon_name: String,
    pub temp: i8,
    pub temp_real: i8,
    pub temp_unit: String,
    pub text: String,
    pub day: String,
    pub sunrise: String,
    pub sunset: String,
    pub sunrise_mins: u64,
    pub sunset_mins: u64,
    pub daylight: f64,
    pub locality: String,
    pub humidity: u8
}

#[derive(Default, Deserialize, Debug)]
pub struct AvgLoadStats {
    pub m1: f64,
    pub m5: f64,
    pub m15: f64,
    pub ncpu: usize,
    pub critical_factor: f64,
    pub color: String
}

#[derive(Default, Deserialize, Debug)]
pub struct VolumeStats {
    pub value: i64,
    pub icon: String,
    pub color: String,
    pub clazz: String
}

#[derive(Default, Deserialize, Debug)]
struct SystemStats {
    ram: RamStats,
    disk: DiskStats,
    temperature: TempStats,
    weather: WeatherStats,
    loadavg: AvgLoadStats,
    volume: VolumeStats,
    written_at: u64,
    metronome: bool
}

macro_rules! extract_json {
    ($data:expr => { $($path:literal => $method:ident),+ $(,)? }) => {
        (
            $(
                {
                    fn get_nested<'a>(data: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
                        path.split('.').fold(Some(data), |acc, key| acc?.get(key))
                    }
                    get_nested($data, $path).and_then(|v| v.$method())
                }
            ),+
        )
    };
}

fn hex_to_color(hex: &str) -> Option<Vec<u8>> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }

    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;

    Some(vec![r, g, b])
}

fn read_json<P: AsRef<Path>>(path: P) -> Option<SystemStats> {
    let json = fs::read_to_string(path).ok()?;
    serde_json::from_str(&json).ok()
}

fn draw_text(canvas: &mut Pixmap, font: &Font, text: &str, x: f32, y: f32) {
    let font_size = 28.0;
    let mut cursor_x = x;

    for ch in text.chars() {
        let (metrics, bitmap) = font.rasterize(ch, font_size);
        let width = metrics.width as u32;
        let height = metrics.height as u32;

        // println!("{} {} {}", ch, width, height);
        if width > 0 {
            let mut img = Pixmap::new(width, height).unwrap();
            for (i, pixel) in img.pixels_mut().into_iter().enumerate() {
                let alpha = bitmap.get(i).copied().unwrap_or(0);
                if let Some(c) = PremultipliedColorU8::from_rgba(255, 255, 255, alpha) {
                    *pixel = c;
                }
            }

            canvas.draw_pixmap(
                cursor_x as i32 + metrics.xmin,
                y as i32 + metrics.ymin,
                img.as_ref(),
                &PixmapPaint::default(),
                Transform::identity(),
                None,
            );
        }

        cursor_x += metrics.advance_width;
    }
}

fn draw_image(status: &SystemStats, width: u32, height: u32) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let mut pixmap = Pixmap::new(width, height).unwrap();
    pixmap.fill(Color::from_rgba8(0, 0, 0, 255));

    let font_data = include_bytes!("/usr/share/fonts/noto/NotoSans-Regular.ttf") as &[u8];
    let font = Font::from_bytes(font_data, fontdue::FontSettings::default()).unwrap();

    let messages = vec![
        format!("Time: {}", status.written_at),
        format!("CPU: {:.1}%", status.loadavg.m1),
        format!("RAM: {:.1} GB", status.ram.total_memory),
        format!("Temp: {:.1} °C", status.temperature.sensor)
    ];

    for (i, msg) in messages.iter().enumerate() {
        draw_text(&mut pixmap, &font, msg, 50.0, 80.0 + i as f32 * 50.0);
    }

    // let raw = pixmap.data();
    // ImageBuffer::from_raw(width, height, raw.to_vec()).expect("Buffer conversion failed")
    let mut img = ImageBuffer::new(width, height);
    for (i, pixel) in pixmap.pixels().into_iter().enumerate() {
        let x = (i as u32) % width;
        let y = (i as u32) / width;
        img.put_pixel(x, y, Rgba([pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()]));
    }
    img
}

fn main() {
    let json_path = "/tmp/ratatoskr.json";
    let output_path = "/tmp/dynamic-wallpaper.png";
    let (width, height) = (1920, 1080);

    loop {
        println!("Looping");

        let mut ss = SystemStats::default();

        if let Ok(contents) = fs::read_to_string("/tmp/ratatoskr.json") {
            let res: Result<Value, serde_json::Error> = serde_json::from_str(&contents);
            if let Ok(data) = res {

                if let (Some(avg_m1), Some(avg_m5), Some(avg_m15), Some(avg_color)) = extract_json!(&data => {
                    "loadavg.m1" => as_f64,
                    "loadavg.m5" => as_f64,
                    "loadavg.m15" => as_f64,
                    "loadavg.color" => as_str
                }) {
                    // spans.push(Span::styled(format!("[AVG {avg_m1} {avg_m5} {avg_m15}]"), Style::default().fg(hex_to_color(avg_color).unwrap())));
                    ss.loadavg.m1 = avg_m1;
                    ss.loadavg.m5 = avg_m5;
                    ss.loadavg.m15 = avg_m15;
                    ss.loadavg.color = avg_color.into();
                }

                /* if let Some(status) = read_json(json_path) {
                    let img = draw_image(&ss, width, height);
                    img.save(output_path).expect("Could not save wallpaper image");
                    println!("Image saved");
                } else {
                    println!("Missing data");
                } */
               let img = draw_image(&ss, width, height);
                img.save(output_path).expect("Could not save wallpaper image");
                println!("Image saved");
            }
        }
        thread::sleep(Duration::from_secs(2));
    }
}
