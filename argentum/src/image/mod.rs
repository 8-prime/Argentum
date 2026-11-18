use std::ops::{Index, IndexMut};

use crate::image::colors::{RgbColorType, linear_to_srgb, srgb_to_linear};

pub mod colors;
pub mod load;
pub mod save;

pub struct PixelData {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
}

impl PixelData {
    fn sum(&self) -> f32 {
        self.red + self.green + self.blue
    }

    pub fn normalize(&mut self) {
        let sum = &self.sum();
        self.red = self.red / sum;
        self.green = self.green / sum;
        self.blue = self.blue / sum;
    }

    fn linearize(&self) -> PixelData {
        PixelData {
            red: srgb_to_linear(self.red),
            green: srgb_to_linear(self.green),
            blue: srgb_to_linear(self.blue),
        }
    }

    fn gamma(&self) -> PixelData {
        PixelData {
            red: linear_to_srgb(self.red),
            green: linear_to_srgb(self.green),
            blue: linear_to_srgb(self.blue),
        }
    }

    fn as_slice(&self) -> [f32; 3] {
        [self.red, self.green, self.blue]
    }
}

impl IndexMut<RgbColorType> for PixelData {
    fn index_mut(&mut self, index: RgbColorType) -> &mut Self::Output {
        match index {
            RgbColorType::Red => &mut self.red,
            RgbColorType::Green => &mut self.green,
            RgbColorType::Blue => &mut self.blue,
        }
    }
}

impl Index<RgbColorType> for PixelData {
    type Output = f32;

    fn index(&self, index: RgbColorType) -> &Self::Output {
        match index {
            RgbColorType::Red => &self.red,
            RgbColorType::Green => &self.green,
            RgbColorType::Blue => &self.blue,
        }
    }
}

pub struct LinearImage {
    pub width: u32,
    pub height: u32,
    pub data: Vec<PixelData>,
}
