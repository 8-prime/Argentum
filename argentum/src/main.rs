use anyhow::{Result, anyhow};
use clap::Parser;
use rawler::imgop::develop::{Intermediate, ProcessingStep, RawDevelop};
use std::path::Path;

#[derive(Parser)]
struct Args {
    input: std::path::PathBuf,
}

struct LinearImage {
    width: u32,
    height: u32,
    data: Vec<[f32; 3]>,
}

struct HDPoint {
    exposure: f32,
    density: f32,
}

struct HDCurve {
    // List of control points that describe the hd curve per color channel
    r: Vec<HDPoint>,
    g: Vec<HDPoint>,
    b: Vec<HDPoint>,
}

struct SpectralPoint {
    wavelength: f32, // in nanometers
    log_sensitivity: f32,
}

struct SpectralSensitivityCurve {
    cyan: Vec<SpectralPoint>,
    magenta: Vec<SpectralPoint>,
    yellow: Vec<SpectralPoint>,
}

struct CrossSensitivityMatrix {
    // derived from SpectralSensitivityCurve at load time
    values: [[f32; 3]; 3],
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

fn derive_cross_sensetivity_matrix(ssc: &SpectralSensitivityCurve) -> CrossSensitivityMatrix {
    let colors = [(400.0, 500.0), (500.0, 600.0), (600.0, 700.0)]; //B, G, R
    let mut matrix_values = [[0.0f32; 3]; 3];

    for (layer_idx, layer) in [&ssc.yellow, &ssc.magenta, &ssc.cyan].iter().enumerate() {
        let integrals: [f32; 3] = std::array::from_fn(|i| {
            let (lower, upper) = colors[i];
            integrate_color_channel(layer, lower, upper)
        });
        let sum: f32 = integrals.iter().sum();
        for (i, &v) in integrals.iter().enumerate() {
            matrix_values[layer_idx][i] = v / sum;
        }
    }

    CrossSensitivityMatrix {
        values: matrix_values,
    }
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
            .map(|p| {
                [
                    (p[0] * rescale_factor).max(0.0),
                    (p[1] * rescale_factor).max(0.0),
                    (p[2] * rescale_factor).max(0.0),
                ]
            })
            .collect(),
        Intermediate::Monochrome(pixels) => pixels
            .data
            .iter()
            .map(|p| {
                let v = (p * rescale_factor).max(0.0);
                [v, v, v]
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

fn load_standard(path: &Path) -> Result<LinearImage> {
    let img = image::open(path)?.into_rgb32f();
    let (width, height) = img.dimensions();
    let data = img
        .pixels()
        .map(|p| {
            let [r, g, b] = p.0;
            [srgb_to_linear(r), srgb_to_linear(g), srgb_to_linear(b)]
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
    let image = match is_raw(&args.input) {
        true => load_raw(&args.input),
        false => load_standard(&args.input),
    }?;

    println!("Image with w {} and h {}", image.width, image.height);
    Ok(())
}
