use chrono::{DateTime, Local};
use std::{
    fs, io,
    path::{Path, PathBuf},
    time::SystemTime,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotationCadence {
    Never,
    Hourly,
    Daily,
}
#[derive(Debug, Clone)]
pub struct RotationPolicy {
    pub max_file_size: Option<u64>,
    pub max_files: usize,
    pub cadence: RotationCadence,
}
impl Default for RotationPolicy {
    fn default() -> Self {
        Self {
            max_file_size: None,
            max_files: 5,
            cadence: RotationCadence::Never,
        }
    }
}
#[derive(Debug, Clone)]
pub struct RotationState {
    pub policy: RotationPolicy,
}
impl RotationState {
    pub fn new(policy: RotationPolicy) -> Self {
        Self { policy }
    }
    pub fn should_rotate(&self, path: &Path, incoming: usize) -> io::Result<bool> {
        if !path.exists() {
            return Ok(false);
        }
        if let Some(max) = self.policy.max_file_size {
            if fs::metadata(path)?.len().saturating_add(incoming as u64) > max {
                return Ok(true);
            }
        }
        let m = fs::metadata(path)?.modified().unwrap_or(SystemTime::now());
        let then: DateTime<Local> = m.into();
        let now = Local::now();
        Ok(match self.policy.cadence {
            RotationCadence::Never => false,
            RotationCadence::Hourly => {
                then.format("%Y%m%d%H").to_string() != now.format("%Y%m%d%H").to_string()
            }
            RotationCadence::Daily => {
                then.format("%Y%m%d").to_string() != now.format("%Y%m%d").to_string()
            }
        })
    }
    pub fn rotate(&self, path: &Path) -> io::Result<Option<PathBuf>> {
        if !path.exists() {
            return Ok(None);
        }
        if self.policy.max_files == 0 {
            fs::write(path, "")?;
            return Ok(None);
        }
        let stamp = Local::now().format("%Y%m%d-%H%M%S");
        let mut archive = path.as_os_str().to_owned();
        archive.push(format!(".{stamp}"));
        let archive = PathBuf::from(archive);
        fs::rename(path, &archive)?;
        self.cleanup(path)?;
        Ok(Some(archive))
    }
    fn cleanup(&self, path: &Path) -> io::Result<()> {
        let Some(parent) = path.parent() else {
            return Ok(());
        };
        let base = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
            + ".";
        let mut v: Vec<_> = fs::read_dir(parent)?
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().starts_with(&base))
            .collect();
        v.sort_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());
        while v.len() > self.policy.max_files {
            let e = v.remove(0);
            let _ = fs::remove_file(e.path());
        }
        Ok(())
    }
}
