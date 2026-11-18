use std::ops::Index;

use serde::Deserialize;
use strum::IntoEnumIterator;

use crate::{
    image::{
        PixelData,
        colors::{CmyColorType, RgbColorType},
    },
    ssc::csm::{CrossSensitivityMatrix, integrate_color_channel},
};

pub mod csm;
pub mod load;

#[derive(Deserialize)]
pub struct SpectralPoint {
    wavelength: f32, // in nanometers
    log_sensitivity: f32,
}

#[derive(Deserialize)]
pub struct SpectralSensitivityCurve {
    cyan: Vec<SpectralPoint>,
    magenta: Vec<SpectralPoint>,
    yellow: Vec<SpectralPoint>,
}

impl Index<CmyColorType> for SpectralSensitivityCurve {
    type Output = Vec<SpectralPoint>;
    fn index(&self, color: CmyColorType) -> &Self::Output {
        match color {
            CmyColorType::Cyan => &self.cyan,
            CmyColorType::Magenta => &self.magenta,
            CmyColorType::Yellow => &self.yellow,
        }
    }
}

pub fn derive_cross_sensetivity_matrix_for_color(
    ssc: &SpectralSensitivityCurve,
    color: RgbColorType,
) -> PixelData {
    let layer = &ssc[RgbColorType::to_cmy(&color)];

    let mut pixelData = PixelData {
        blue: 0.0,
        green: 0.0,
        red: 0.0,
    };

    for matrix_color in RgbColorType::iter() {
        let (lower, upper) = matrix_color.range();
        pixelData[matrix_color] = integrate_color_channel(&layer, lower, upper)
    }
    pixelData.normalize();
    pixelData
}

pub fn derive_cross_sensetivity_matrix(ssc: &SpectralSensitivityCurve) -> CrossSensitivityMatrix {
    CrossSensitivityMatrix {
        red: derive_cross_sensetivity_matrix_for_color(ssc, RgbColorType::Red),
        green: derive_cross_sensetivity_matrix_for_color(ssc, RgbColorType::Green),
        blue: derive_cross_sensetivity_matrix_for_color(ssc, RgbColorType::Blue),
    }
}
