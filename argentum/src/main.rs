use anyhow::{Result, anyhow};
use clap::Parser;
use rawler::{
    formats::bmff::vmhd::RgbColor,
    imgop::develop::{Intermediate, ProcessingStep, RawDevelop},
};
use serde::Deserialize;
use std::{
    ops::{Index, IndexMut},
    path::{Path, PathBuf},
};
use strum::IntoEnumIterator; // 0.17.1
use strum_macros::EnumIter;

use crate::{
    hd::load::load_hd_curve,
    image::{PixelData, load::load_image, save::save_jpeg},
    ssc::{derive_cross_sensetivity_matrix, load::load_ssc},
}; // 0.17.1

mod hd;
mod image;
mod ssc;

#[derive(Parser)]
struct Args {
    input: std::path::PathBuf,
    output: Option<std::path::PathBuf>,
}

fn determine_output(args: &Args) -> PathBuf {
    if let Some(out) = &args.output {
        return out.clone();
    }
    return args.input.with_extension("jpg");
}

fn main() -> Result<()> {
    let args = Args::parse();

    let hd_curve = load_hd_curve(Path::new("kodak_gold_200_hd.json"))?;
    let ssc = load_ssc(Path::new("kodak_gold_200_ssc.json"))?;
    let cross_sensitivity = derive_cross_sensetivity_matrix(&ssc);

    let image = load_image(&args.input)?;
    println!("Loaded image with w {} and h {}", image.width, image.height);

    let image = cross_sensitivity.apply(&image);
    let image = hd_curve.apply(&image);

    // save_tiff(&image, &args.input)?;
    save_jpeg(&image, &determine_output(&args))?;

    Ok(())
}
