use anyhow::{Result, anyhow};
use rawler::imgop::develop::{Intermediate, ProcessingStep, RawDevelop};
use std::path::{Path, PathBuf};

use crate::image::{LinearImage, PixelData, colors::srgb_to_linear};

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

pub fn load_image(input: &Path) -> Result<LinearImage> {
    let img = match is_raw(&input) {
        true => load_raw(&input),
        false => load_standard(&input),
    };

    return img;
}
