use std::ops::Index;

use strum_macros::EnumIter;

use crate::{
    image::{LinearImage, PixelData, colors::RgbColorType},
    ssc::SpectralPoint,
};

pub struct CrossSensitivityMatrix {
    //use pixel data to allow for accessing the different matrix values by their color represenattion.
    // TODO: theoretically this should be its own struct.
    pub red: PixelData,
    pub green: PixelData,
    pub blue: PixelData,
}

impl Index<RgbColorType> for CrossSensitivityMatrix {
    type Output = PixelData;
    fn index(&self, color: RgbColorType) -> &Self::Output {
        match color {
            RgbColorType::Red => &self.red,
            RgbColorType::Green => &self.green,
            RgbColorType::Blue => &self.blue,
        }
    }
}

impl CrossSensitivityMatrix {
    pub fn apply(&self, image: &LinearImage) -> LinearImage {
        let data = image
            .data
            .iter()
            .map(|p| PixelData {
                red: &self[RgbColorType::Red].red * p.red
                    + &self[RgbColorType::Red].green * p.green
                    + &self[RgbColorType::Red].blue * p.blue,
                green: &self[RgbColorType::Green].red * p.red
                    + &self[RgbColorType::Green].green * p.green
                    + &self[RgbColorType::Green].blue * p.blue,
                blue: &self[RgbColorType::Blue].red * p.red
                    + &self[RgbColorType::Blue].green * p.green
                    + &self[RgbColorType::Blue].blue * p.blue,
            })
            .collect();
        LinearImage {
            width: image.width,
            height: image.height,
            data,
        }
    }
}

pub fn integrate_color_channel(
    spectral_points: &Vec<SpectralPoint>,
    color_start: f32,
    color_end: f32,
) -> f32 {
    let points: Vec<(f32, f32)> = spectral_points
        .iter()
        .filter(|p| p.wavelength >= color_start && p.wavelength <= color_end)
        .map(|p| (p.wavelength, 10f32.powf(p.log_sensitivity)))
        .collect();

    points
        .windows(2)
        .map(|w| {
            let (w1, l1) = w[0];
            let (w2, l2) = w[1];
            (w2 - w1) * (l1 + l2) / 2.0
        })
        .sum()
}
