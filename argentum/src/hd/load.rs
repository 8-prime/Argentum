use anyhow::Result;
use std::path::Path;

use crate::hd::HDCurve;

pub fn load_hd_curve(path: &Path) -> Result<HDCurve> {
    let file = std::fs::File::open(path)?;
    let curve = serde_json::from_reader(std::io::BufReader::new(file))?;
    Ok(curve)
}
