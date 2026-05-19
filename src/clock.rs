use cairo::Context;

use crate::notch_security::NotchTrait;

pub trait ClockTrait {
    // fn new () -> Self;
    fn draw (&mut self, cr: Context, wheight: i32, right: u32, battery_integrated: Option<crate::battery::BatteryStats>);
    fn get_width (&self) -> f64;
}

pub struct NoClock {}
impl NotchTrait for NoClock {

    fn draw (&mut self, _cr: Context, _x: f64, _y: f64, _w: f64, _h: f64) {}

    fn get_width (&self) -> f64 {
        0.0
    }
    
    fn update_data (&mut self, cr: &Context) -> bool { false }
    
    fn need_redraw(&self) -> bool { false }
    
    fn get_position (&self) -> crate::utils::Anchor { crate::utils::Anchor::None }
    
    fn get_height (&self) -> f64 { 0.0 }
    
    fn is_active (&self) -> bool { false }
}

impl NoClock {
    pub fn new () -> Self {
        NoClock {}
    }
}