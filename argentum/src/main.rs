use anyhow::{Result, anyhow};
use clap::Parser;
use rawler::{
    formats::bmff::vmhd::RgbColor,
    imgop::develop::{Intermediate, ProcessingStep, RawDevelop},
};
use serde::Deserialize;
use std::{
    ops::{Index, IndexMut},
    path::Path,
};
use strum::IntoEnumIterator; // 0.17.1
use strum_macros::EnumIter; // 0.17.1

#[derive(Parser)]
struct Args {
    input: std::path::PathBuf,
}

struct PixelData {
    red: f32,
    green: f32,
    blue: f32,
}

impl PixelData {
    fn sum(&self) -> f32 {
        self.red + self.green + self.blue
    }

    fn normalize(&mut self) {
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

struct LinearImage {
    width: u32,
    height: u32,
    data: Vec<PixelData>,
}

#[derive(Deserialize)]
struct HDPoint {
    exposure: f32,
    density: f32,
}

#[derive(Deserialize)]
struct HDCurve {
    // List of control points that describe the hd curve per color channel
    r: Vec<HDPoint>,
    g: Vec<HDPoint>,
    b: Vec<HDPoint>,
}

impl HDCurve {
    fn apply(&self, image: &LinearImage) -> LinearImage {
        let data = image
            .data
            .iter()
            .map(|p| {
                let to_log = |v: f32| {
                    if v <= 0.0 {
                        f32::NEG_INFINITY
                    } else {
                        v.log10()
                    }
                };
                PixelData {
                    red: interpolate_hd(&self.r, to_log(p.red)),
                    green: interpolate_hd(&self.g, to_log(p.green)),
                    blue: interpolate_hd(&self.b, to_log(p.blue)),
                }
            })
            .collect();
        LinearImage {
            width: image.width,
            height: image.height,
            data,
        }
    }
}

#[derive(Deserialize)]
struct SpectralPoint {
    wavelength: f32, // in nanometers
    log_sensitivity: f32,
}

#[derive(Deserialize)]
struct SpectralSensitivityCurve {
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

struct CrossSensitivityMatrix {
    //use pixel data to allow for accessing the different matrix values by their color represenattion.
    // TODO: theoretically this should be its own struct.
    red: PixelData,
    green: PixelData,
    blue: PixelData,
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
    fn apply(&self, image: &LinearImage) -> LinearImage {
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

#[derive(Debug, EnumIter)]
enum RgbColorType {
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
enum CmyColorType {
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

fn interpolate_hd(points: &[HDPoint], exposure: f32) -> f32 {
    if exposure <= points[0].exposure {
        return points[0].density;
    }
    let last = points.len() - 1;
    if exposure >= points[last].exposure {
        return points[last].density;
    }
    for i in 0..last {
        if exposure <= points[i + 1].exposure {
            let dx = points[i + 1].exposure - points[i].exposure;
            if dx.abs() < f32::EPSILON {
                return points[i].density;
            }
            let t = (exposure - points[i].exposure) / dx;
            return points[i].density + t * (points[i + 1].density - points[i].density);
        }
    }
    points[last].density
}

fn integrate_color_channel(
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

fn derive_cross_sensetivity_matrix_for_color(
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

fn derive_cross_sensetivity_matrix(ssc: &SpectralSensitivityCurve) -> CrossSensitivityMatrix {
    CrossSensitivityMatrix {
        red: derive_cross_sensetivity_matrix_for_color(ssc, RgbColorType::Red),
        green: derive_cross_sensetivity_matrix_for_color(ssc, RgbColorType::Green),
        blue: derive_cross_sensetivity_matrix_for_color(ssc, RgbColorType::Blue),
    }
}

fn load_hd_curve(path: &Path) -> Result<HDCurve> {
    let file = std::fs::File::open(path)?;
    let curve = serde_json::from_reader(std::io::BufReader::new(file))?;
    Ok(curve)
}

fn load_ssc(path: &Path) -> Result<SpectralSensitivityCurve> {
    let file = std::fs::File::open(path)?;
    let ssc = serde_json::from_reader(std::io::BufReader::new(file))?;
    Ok(ssc)
}

fn is_raw(path: &std::path::Path) -> bool {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .as_deref()
    {
        Some("cr2" | "cr3" | "nef" | "arw" | "dng" | "raf" | "rw2") => true,
        _ => false,
    }
}

fn load_raw(path: &Path) -> Result<LinearImage> {
    let mut raw_image = rawler::decode_file(path)?;
    let original_white_level = raw_image
        .whitelevel
        .0
        .first()
        .cloned()
        .unwrap_or(u16::MAX as u32) as f32;
    let original_black_level = raw_image
        .blacklevel
        .levels
        .first()
        .map(|r| r.as_f32())
        .unwrap_or(0.0);

    for level in raw_image.whitelevel.0.iter_mut() {
        *level = u32::MAX;
    }

    let mut developer = RawDevelop::default();
    developer.steps.retain(|&step| step != ProcessingStep::SRgb);
    let developed_intermediate = developer.develop_intermediate(&raw_image)?;
    let denominator = (original_white_level - original_black_level).max(1.0);
    let rescale_factor = (u32::MAX as f32 - original_black_level) / denominator;
    let dim = developed_intermediate.dim();
    let width = dim.w as u32;
    let height = dim.h as u32;

    let data = match developed_intermediate {
        Intermediate::ThreeColor(pixels) => pixels
            .data
            .iter()
            .map(|p| PixelData {
                red: (p[0] * rescale_factor).max(0.0),
                green: (p[1] * rescale_factor).max(0.0),
                blue: (p[2] * rescale_factor).max(0.0),
            })
            .collect(),
        Intermediate::Monochrome(pixels) => pixels
            .data
            .iter()
            .map(|p| {
                let v = (p * rescale_factor).max(0.0);
                PixelData {
                    red: v,
                    green: v,
                    blue: v,
                }
            })
            .collect(),
        _ => return Err(anyhow!("unsupported intermediate foramt")),
    };

    Ok(LinearImage {
        width,
        height,
        data,
    })
}

fn srgb_to_linear(v: f32) -> f32 {
    //Transfer function according to
    //https://wikimedia.org/api/rest_v1/media/math/render/svg/9403ccced1e836dcb55cea7aae09b70aee141f89
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055f32).powf(2.4)
    }
}

fn linear_to_srgb(v: f32) -> f32 {
    if (v <= 0.0031308) {
        return 12.92 * v;
    }
    1.055 * v.powf(1.0 / 2.4) - 0.055
}

fn load_standard(path: &Path) -> Result<LinearImage> {
    let img = image::open(path)?.into_rgb32f();
    let (width, height) = img.dimensions();
    let data = img
        .pixels()
        .map(|p| {
            let [r, g, b] = p.0;
            PixelData {
                red: srgb_to_linear(r),
                green: srgb_to_linear(g),
                blue: srgb_to_linear(b),
            }
        })
        .collect();
    Ok(LinearImage {
        width,
        height,
        data,
    })
}

fn main() -> Result<()> {
    let args = Args::parse();

    let hd_curve = load_hd_curve(Path::new("kodak_gold_200_hd.json"))?;
    let ssc = load_ssc(Path::new("kodak_gold_200_ssc.json"))?;
    let cross_sensitivity = derive_cross_sensetivity_matrix(&ssc);

    let image = match is_raw(&args.input) {
        true => load_raw(&args.input),
        false => load_standard(&args.input),
    }?;

    let image = cross_sensitivity.apply(&image);
    // let image = hd_curve.apply(&image);

    save_tiff(&image, &args.input)?;
    save_jpeg(&image, &args.input)?;

    Ok(())
}

fn save_tiff(image: &LinearImage, input: &std::path::Path) -> Result<()> {
    let output_path = input.with_extension("tiff");
    let flat: Vec<f32> = image
        .data
        .iter()
        .flat_map(|p| p.gamma().as_slice())
        .collect();
    image::ImageBuffer::<image::Rgb<f32>, _>::from_raw(image.width, image.height, flat)
        .ok_or_else(|| anyhow!("failed to construct output image buffer"))?
        .save(&output_path)?;
    println!("Saved to {}", output_path.display());
    Ok(())
}

fn save_jpeg(image: &LinearImage, input: &std::path::Path) -> Result<()> {
    let output_path = input.with_extension("jpg");
    let flat: Vec<u8> = image
        .data
        .iter()
        .flat_map(|p| {
            let g = p.gamma();
            [
                (g.red.clamp(0.0, 1.0) * 255.0).round() as u8,
                (g.green.clamp(0.0, 1.0) * 255.0).round() as u8,
                (g.blue.clamp(0.0, 1.0) * 255.0).round() as u8,
            ]
        })
        .collect();
    image::ImageBuffer::<image::Rgb<u8>, _>::from_raw(image.width, image.height, flat)
        .ok_or_else(|| anyhow!("failed to construct output image buffer"))?
        .save(&output_path)?;
    println!("Saved to {}", output_path.display());
    Ok(())
}
