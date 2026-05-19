use cairo::Context;

pub trait ClockTrait {
    // fn new () -> Self;
    fn draw (&mut self, cr: Context, wheight: i32, right: u32, battery_integrated: Option<crate::battery::BatteryStats>);
    fn get_width (&self) -> f64;
}

pub struct NoClock {}
impl ClockTrait for NoClock {

    fn draw (&mut self, _cr: Context, _wheight: i32, _right: u32, _battery_integrated: Option<crate::battery::BatteryStats>) {}

    fn get_width (&self) -> f64 {
        0.0
    }
}

impl NoClock {
    pub fn new () -> Self {
        NoClock {}
    }
}