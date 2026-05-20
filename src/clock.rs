use cairo::Context;

pub trait ClockTrait {
    // fn new () -> Self;
    fn draw (&mut self, cr: Context, x: f64, y: f64, w: f64, h: f64, battery_integrated: Option<crate::battery::BatteryStats>) -> (f64, f64);
    fn get_reserved_width (&self) -> f64;
    fn get_reserved_height (&self) -> f64;
}

pub struct NoClock {}
impl ClockTrait for NoClock {

    fn draw (&mut self, _cr: Context, _x: f64, _y: f64, _w: f64, _h: f64, _battery_integrated: Option<crate::battery::BatteryStats>) -> (f64, f64) {
        (0.0, 0.0)
    }

    fn get_reserved_width (&self) -> f64 {
        0.0
    }

    fn get_reserved_height (&self) -> f64 {
        0.0
    }
}

impl NoClock {
    pub fn new () -> Self {
        NoClock {}
    }
}