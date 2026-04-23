use cairo::Context;

pub trait ClockTrait {
    fn new () -> Self;
    fn draw (&mut self, cr: Context, wheight: i32, right: u32, battery_recharging: Option<bool>, battery_eta: Option<f64>);
}

pub struct Clock {
    
}