use anyhow::{Result, anyhow};
use clap::Parser;
use rawler::imgop::develop::{Intermediate, ProcessingStep, RawDevelop};
use serde::Deserialize;
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
            .map(|&[r, g, b]| {
                let to_log = |v: f32| {
                    if v <= 0.0 {
                        f32::NEG_INFINITY
                    } else {
                        v.log10()
                    }
                };
                [
                    interpolate_hd(&self.r, to_log(r)),
                    interpolate_hd(&self.g, to_log(g)),
                    interpolate_hd(&self.b, to_log(b)),
                ]
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

struct CrossSensitivityMatrix {
    // derived from SpectralSensitivityCurve at load time
    values: [[f32; 3]; 3],
}

impl CrossSensitivityMatrix {
    fn apply(&self, image: &LinearImage) -> LinearImage {
        let data = image
            .data
            .iter()
            .map(|&[r, g, b]| {
                let v = &self.values;
                [
                    v[0][0] * b + v[0][1] * g + v[0][2] * r, // yellow layer
                    v[1][0] * b + v[1][1] * g + v[1][2] * r, // magenta layer
                    v[2][0] * b + v[2][1] * g + v[2][2] * r, // cyan layer
                ]
            })
            .collect();
        LinearImage {
            width: image.width,
            height: image.height,
            data,
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

    let hd_curve = load_hd_curve(Path::new("kodak_gold_200_hd.json"))?;
    let ssc = load_ssc(Path::new("kodak_gold_200_ssc.json"))?;
    let cross_sensitivity = derive_cross_sensetivity_matrix(&ssc);

    let image = match is_raw(&args.input) {
        true => load_raw(&args.input),
        false => load_standard(&args.input),
    }?;

    let image = cross_sensitivity.apply(&image);
    let image = hd_curve.apply(&image);

    let output_path = args.input.with_extension("tiff");
    let flat: Vec<f32> = image.data.iter().flat_map(|&[r, g, b]| [r, g, b]).collect();
    image::ImageBuffer::<image::Rgb<f32>, _>::from_raw(image.width, image.height, flat)
        .ok_or_else(|| anyhow!("failed to construct output image buffer"))?
        .save(&output_path)?;

    println!("Saved to {}", output_path.display());
    Ok(())
}
