//! Reviewed read-only source access.

use std::path::{Path, PathBuf};

#[cfg(not(target_arch = "wasm32"))]
use std::fs::File;
#[cfg(not(target_arch = "wasm32"))]
use memmap2::{Mmap, MmapOptions};

use thiserror::Error;

const NESSTAR_MAGIC: &[u8; 8] = b"NESSTART";

#[derive(Debug, Error)]
pub enum SourceError {
    #[error("invalid source `{path}`: {reason}")]
    Invalid { path: PathBuf, reason: String },
    #[error("source `{path}` {context}: range {start}..{end} exceeds {length} bytes")]
    OutOfBounds {
        path: PathBuf,
        context: &'static str,
        start: usize,
        end: usize,
        length: usize,
    },
}

enum SourceBacking {
    #[cfg(not(target_arch = "wasm32"))]
    Mmap {
        _file: File,
        mmap: Mmap,
    },
    Bytes(Vec<u8>),
}

/// A read-only source held for the conversion job's lifetime.
pub struct ReadOnlySource {
    path: PathBuf,
    backing: SourceBacking,
}

impl ReadOnlySource {
    #[cfg(not(target_arch = "wasm32"))]
    #[allow(unsafe_code)]
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SourceError> {
        let path = path.as_ref().to_path_buf();
        if path.as_os_str().is_empty() {
            return Err(SourceError::Invalid {
                path,
                reason: "empty path".into(),
            });
        }
        let file = File::open(&path).map_err(|error| SourceError::Invalid {
            path: path.clone(),
            reason: format!("cannot open: {error}"),
        })?;
        let mmap =
            unsafe { MmapOptions::new().map(&file) }.map_err(|error| SourceError::Invalid {
                path: path.clone(),
                reason: format!("cannot map read-only: {error}"),
            })?;
        let source = Self {
            path,
            backing: SourceBacking::Mmap { _file: file, mmap },
        };
        source.validate_magic()?;
        Ok(source)
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, SourceError> {
        let source = Self {
            path: PathBuf::from("in-memory.Nesstar"),
            backing: SourceBacking::Bytes(bytes),
        };
        source.validate_magic()?;
        Ok(source)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn len(&self) -> usize {
        self.bytes().len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes().is_empty()
    }

    pub fn bytes(&self) -> &[u8] {
        match &self.backing {
            #[cfg(not(target_arch = "wasm32"))]
            SourceBacking::Mmap { mmap, .. } => mmap,
            SourceBacking::Bytes(b) => b,
        }
    }

    pub fn slice(
        &self,
        start: usize,
        length: usize,
        context: &'static str,
    ) -> Result<&[u8], SourceError> {
        let end = start
            .checked_add(length)
            .ok_or_else(|| SourceError::OutOfBounds {
                path: self.path.clone(),
                context,
                start,
                end: usize::MAX,
                length: self.len(),
            })?;
        self.bytes()
            .get(start..end)
            .ok_or_else(|| SourceError::OutOfBounds {
                path: self.path.clone(),
                context,
                start,
                end,
                length: self.len(),
            })
    }

    fn validate_magic(&self) -> Result<(), SourceError> {
        if self.len() < NESSTAR_MAGIC.len() {
            return Err(SourceError::Invalid {
                path: self.path.clone(),
                reason: "file is shorter than the NESSTART header".into(),
            });
        }
        if self.bytes().get(..NESSTAR_MAGIC.len()) != Some(NESSTAR_MAGIC) {
            return Err(SourceError::Invalid {
                path: self.path.clone(),
                reason: "first eight bytes are not NESSTART".into(),
            });
        }
        Ok(())
    }
}
