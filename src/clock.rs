use cairo::Context;
use enum_dispatch::enum_dispatch;

use crate::{clock1::Clock1, clock2::Clock2};

#[enum_dispatch]
pub trait ClockTrait {
    // fn new () -> Self;
    fn draw (&mut self, cr: Context, wheight: i32, right: u32, battery_integrated: Option<crate::battery::BatteryStats>);
    fn get_reserved_width (&self) -> f64;
}

pub struct NoClock {}
impl ClockTrait for NoClock {

    fn draw (&mut self, _cr: Context, _wheight: i32, _right: u32, _battery_integrated: Option<crate::battery::BatteryStats>) {}

    fn get_reserved_width (&self) -> f64 {
        0.0
    }
}

impl NoClock {
    pub fn new () -> Self {
        NoClock {}
    }
}

#[enum_dispatch(ClockTrait)]
pub enum ClockWrapper {
    Clock1,
    Clock2,
    NoClock
}
/*
pub enum ClockWrapper {
    Clock1(crate::clock1::Clock1),
    Clock2(crate::clock2::Clock2),
    NoClock(crate::clock::NoClock),
}

impl ClockTrait for ClockWrapper {
    fn draw(& mut self, cr: Context, wheight: i32, right: u32, battery_integrated: Option<crate::battery::BatteryStats>) {
        match self {
            ClockWrapper::Clock1(c) => c.draw(cr, wheight, right, battery_integrated),
            ClockWrapper::Clock2(c) => c.draw(cr, wheight, right, battery_integrated),
            ClockWrapper::NoClock(c) => c.draw(cr, wheight, right, battery_integrated),
        }
    }
    
    fn get_reserved_width (&self) -> f64 {
        match self {
            ClockWrapper::Clock1(c) => c.get_reserved_width(),
            ClockWrapper::Clock2(c) => c.get_reserved_width(),
            ClockWrapper::NoClock(c) => c.get_reserved_width()
        }
    }
}
*/