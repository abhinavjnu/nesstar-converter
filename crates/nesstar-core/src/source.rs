//! Reviewed read-only source access.

use std::{
    fs::File,
    path::{Path, PathBuf},
};

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

/// A read-only mapping held for the conversion job's lifetime.
///
/// `memmap2` requires `unsafe` because the caller must ensure that the mapped
/// file is not truncated while the mapping is live. Nesstar inputs are opened
/// read-only and this wrapper keeps the file handle private, so no code in the
/// converter can mutate the mapped file through this API.
pub struct ReadOnlySource {
    path: PathBuf,
    _file: File,
    mmap: Mmap,
}

impl ReadOnlySource {
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
        // SAFETY: `file` is retained in this struct for at least as long as
        // `mmap`; this API exposes no mutable mapping or file handle.
        let mmap =
            unsafe { MmapOptions::new().map(&file) }.map_err(|error| SourceError::Invalid {
                path: path.clone(),
                reason: format!("cannot map read-only: {error}"),
            })?;
        let source = Self {
            path,
            _file: file,
            mmap,
        };
        source.validate_magic()?;
        Ok(source)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn len(&self) -> usize {
        self.mmap.len()
    }
    pub fn is_empty(&self) -> bool {
        self.mmap.is_empty()
    }
    pub fn bytes(&self) -> &[u8] {
        &self.mmap
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
        self.mmap
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
