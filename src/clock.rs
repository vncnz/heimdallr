use cairo::Context;

pub trait ClockTrait {
    // fn new () -> Self;
    fn draw (&mut self, cr: Context, wheight: i32, right: u32, battery_recharging: Option<bool>, battery_eta: Option<f64>);
    fn get_reserved_width (&self) -> f64;
}

pub struct NoClock {}
impl ClockTrait for NoClock {

    fn draw (&mut self, _cr: Context, _wheight: i32, _right: u32, _battery_recharging: Option<bool>, _battery_eta: Option<f64>) {}

    fn get_reserved_width (&self) -> f64 {
        0.0
    }
}

impl NoClock {
    pub fn new () -> Self {
        NoClock {}
    }
}