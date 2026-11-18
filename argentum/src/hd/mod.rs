use serde::Deserialize;

use crate::image::{LinearImage, PixelData};

pub mod load;

#[derive(Deserialize)]
struct HDPoint {
    exposure: f32,
    density: f32,
}

#[derive(Deserialize)]
pub struct HDCurve {
    // List of control points that describe the hd curve per color channel
    r: Vec<HDPoint>,
    g: Vec<HDPoint>,
    b: Vec<HDPoint>,
}

impl HDCurve {
    pub fn apply(&self, image: &LinearImage) -> LinearImage {
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
