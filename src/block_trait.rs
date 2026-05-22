use cairo::{Context, Format, ImageSurface};

pub trait BlockTrait {
    type DataType;

    // fn new () -> Self;
    fn update_data (&mut self, data: Self::DataType) -> Result<bool, Box<dyn std::error::Error>>;
    fn draw (&mut self, cr: Context, x: f64, y: f64, w: f64, h: f64) -> (f64, f64);
    fn get_width (&self) -> f64;
    fn get_height (&self) -> f64;
}

fn compute_pango_layout (cr: &Context, text: &str, font_size: f64) -> (pango::Layout, f64, f64) {
    let layout = pangocairo::functions::create_layout(cr);
    let mut font_desc = pango::FontDescription::new();
    font_desc.set_family(""); // O "Iosevka", o lasci il default
    font_desc.set_absolute_size(font_size * pango::SCALE as f64);
    layout.set_font_description(Some(&font_desc));
    layout.set_text(text);
    let (ink_rect, logical_rect) = layout.extents();
    let w = ink_rect.width() as f64 / pango::SCALE as f64;
    let h = ink_rect.height() as f64 / pango::SCALE as f64;
    (layout, w, h)
}

pub struct ClockBlock {
    surface_cache: Option<ImageSurface>,
    width: i32,
    height: i32
}

impl ClockBlock {
    pub fn new() -> Self {
        Self {
            surface_cache: None,
            width: 0,
            height: 0
        }
    }
}

impl BlockTrait for ClockBlock {
    type DataType = ();

    fn update_data (&mut self, _data: Self::DataType) -> Result<bool, Box<dyn std::error::Error>> {

        let dummy_surface = ImageSurface::create(Format::ARgb32, 1, 1)?;
        let dummy_ctx = Context::new(&dummy_surface)?;
        let (layout, width, height) = compute_pango_layout(&dummy_ctx, "00:00", 16.0);
        let wi = width.ceil() as i32;
        let hi = height.ceil() as i32;

        let need_reallocate = match &self.surface_cache {
            None => true,
            Some(s) => s.width() != wi || s.height() != hi,
        };

        if need_reallocate {
            self.surface_cache = Some(ImageSurface::create(Format::ARgb32, wi, hi).unwrap());
            self.width = wi;
            self.height = hi;
        }

        let s = self.surface_cache.as_ref().unwrap();
        let cr = Context::new(s).unwrap();

        cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);

        cr.move_to(0.0, 0.0);
        pangocairo::functions::show_layout(&cr, &layout);

        Ok(true)
    }

    fn draw (&mut self, cr: Context, x: f64, y: f64, _w: f64, _h: f64) -> (f64, f64) {
        if let Some(ref local_surface) = self.surface_cache {
            cr.save().unwrap();
            cr.translate(x, y);
            cr.set_source_surface(local_surface, 0.0, 0.0).unwrap();
            cr.paint().unwrap();
            cr.restore().unwrap();
        }
        (0.0, 0.0)
    }

    fn get_width (&self) -> f64 {
        self.width as f64
    }

    fn get_height (&self) -> f64 {
        self.height as f64
    }
}


pub struct MainBatteryBlock {
    pub(crate) battery: crate::battery::BatteryStats
}

impl BlockTrait for MainBatteryBlock {
    type DataType = crate::battery::BatteryStats;

    fn update_data (&mut self, data: Self::DataType) -> Result<bool, Box<dyn std::error::Error>> {
        self.battery = data;
        Ok(true)
    }

    fn draw (&mut self, _cr: Context, _x: f64, _y: f64, _w: f64, _h: f64) -> (f64, f64) {
        (0.0, 0.0)
    }

    fn get_width (&self) -> f64 {
        0.0
    }

    fn get_height (&self) -> f64 {
        0.0
    }
}