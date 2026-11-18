use anyhow::{Result, anyhow};

use crate::image::LinearImage;

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

pub fn save_jpeg(image: &LinearImage, input: &std::path::Path) -> Result<()> {
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
