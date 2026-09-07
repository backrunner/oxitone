//! Immutable, content-addressed cache publication; no overwrites of existing assets.
use oxitone_core::{codes, OxitoneError};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempFile(PathBuf);
impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

struct HashWriter<W> {
    writer: W,
    hash: Sha256,
}
impl<W: Write> Write for HashWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = self.writer.write(bytes)?;
        self.hash.update(&bytes[..count]);
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

fn create_temp(directory: &Path) -> io::Result<(TempFile, File)> {
    loop {
        let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = directory.join(format!(
            ".oxitone-cache-{}-{sequence}.tmp",
            std::process::id()
        ));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((TempFile(path), file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
}

fn verify_existing(path: &Path, expected: &str) -> io::Result<()> {
    if !fs::symlink_metadata(path)?.file_type().is_file() {
        return Err(io::Error::other(
            "cache asset is not a regular file (symlinks are rejected)",
        ));
    }
    let mut file = File::open(path)?;
    let mut hash = Sha256::new();
    let mut block = [0u8; 65536];
    loop {
        let count = file.read(&mut block)?;
        if count == 0 {
            break;
        }
        hash.update(&block[..count]);
    }
    if format!("{:x}", hash.finalize()) != expected {
        return Err(io::Error::other("cache asset content hash mismatch"));
    }
    Ok(())
}

pub(crate) fn publish(
    directory: &Path,
    encode: impl FnOnce(&mut dyn Write) -> io::Result<()>,
) -> Result<(PathBuf, String), OxitoneError> {
    let mut error_path = directory.to_path_buf();
    let result = (|| {
        let directory = std::path::absolute(directory)?;
        fs::create_dir_all(&directory)?;
        let (temp, file) = create_temp(&directory)?;
        let mut writer = HashWriter {
            writer: BufWriter::new(&file),
            hash: Sha256::new(),
        };
        encode(&mut writer)?;
        writer.flush()?;
        let hash = format!("{:x}", writer.hash.finalize());
        file.sync_all()?;
        let path = directory.join(format!("{hash}.wav"));
        error_path = path.clone();
        match fs::hard_link(&temp.0, &path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                verify_existing(&path, &hash)?
            }
            Err(error) => return Err(error),
        }
        fs::remove_file(&temp.0)?;
        File::open(&directory)?.sync_all()?;
        Ok((path, hash))
    })();
    result.map_err(|error: io::Error| {
        OxitoneError::with_path(
            codes::ASSET_UNAVAILABLE,
            format!("failed to publish sample cache: {error}"),
            error_path.display().to_string(),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interrupted_encoding_never_publishes_partial_asset() {
        let directory =
            std::env::temp_dir().join(format!("oxitone-cache-failure-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let error = publish(&directory, |writer| {
            writer.write_all(b"partial")?;
            Err(io::Error::other("injected write failure"))
        })
        .unwrap_err();
        assert_eq!(error.code, codes::ASSET_UNAVAILABLE);
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 0);
        fs::remove_dir(&directory).unwrap();
    }
}
