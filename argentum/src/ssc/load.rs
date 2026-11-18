use anyhow::Result;
use std::path::Path;

use crate::ssc::SpectralSensitivityCurve;

pub fn load_ssc(path: &Path) -> Result<SpectralSensitivityCurve> {
    let file = std::fs::File::open(path)?;
    let ssc = serde_json::from_reader(std::io::BufReader::new(file))?;
    Ok(ssc)
}
