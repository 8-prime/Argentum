use strum_macros::EnumIter;

#[derive(Debug, EnumIter)]
pub enum RgbColorType {
    Red,
    Green,
    Blue,
}

impl RgbColorType {
    pub const fn range(&self) -> (f32, f32) {
        match self {
            RgbColorType::Red => (600.0, 700.0),
            RgbColorType::Green => (500.0, 600.0),
            RgbColorType::Blue => (400.0, 500.0),
        }
    }

    pub const fn to_cmy(&self) -> CmyColorType {
        match self {
            RgbColorType::Red => CmyColorType::Cyan,
            RgbColorType::Green => CmyColorType::Magenta,
            RgbColorType::Blue => CmyColorType::Yellow,
        }
    }
}

#[derive(Debug, EnumIter)]
pub enum CmyColorType {
    Cyan,
    Magenta,
    Yellow,
}

impl CmyColorType {
    pub const fn to_rgb(&self) -> RgbColorType {
        match self {
            CmyColorType::Cyan => RgbColorType::Red,
            CmyColorType::Magenta => RgbColorType::Green,
            CmyColorType::Yellow => RgbColorType::Blue,
        }
    }
}

pub fn srgb_to_linear(v: f32) -> f32 {
    //Transfer function according to
    //https://wikimedia.org/api/rest_v1/media/math/render/svg/9403ccced1e836dcb55cea7aae09b70aee141f89
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055f32).powf(2.4)
    }
}

pub fn linear_to_srgb(v: f32) -> f32 {
    if (v <= 0.0031308) {
        return 12.92 * v;
    }
    1.055 * v.powf(1.0 / 2.4) - 0.055
}
