//! Control-side content pinning. No metadata is trusted as evidence of a real VST3 factory.
use crate::{
    wire::{Policy, Source},
    Error, Result,
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

pub fn verify(source: &Source) -> Result<(PathBuf, String)> {
    if fs::symlink_metadata(&source.bundle_path)
        .map_err(asset)?
        .file_type()
        .is_symlink()
    {
        return Err(asset("VST3 bundle symlinks are unsupported"));
    }
    let path = fs::canonicalize(&source.bundle_path).map_err(asset)?;
    if path.extension().is_none_or(|v| v != "vst3") || !path.is_dir() {
        return Err(asset("expected a .vst3 bundle directory"));
    }
    let hash = bundle_hash(&path)?;
    if source
        .expected_hash
        .as_ref()
        .is_some_and(|expected| !expected.eq_ignore_ascii_case(&hash))
    {
        return Err(Error::new(
            "PluginManifestMismatch",
            "VST3 bundle hash mismatch",
        ));
    }
    if source.allow_plugins == Policy::SignedOnly {
        let verified = std::process::Command::new("/usr/bin/codesign")
            .args(["--verify", "--strict", "-R", "anchor apple generic"])
            .arg(&path)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        if !verified.is_ok_and(|result| result.success()) {
            return Err(Error::new(
                "PluginManifestMismatch",
                "signed-only requires an Apple-issued signature",
            ));
        }
    }
    Ok((path, hash))
}
pub fn file_hash(path: &Path) -> Result<String> {
    let mut hash = Sha256::new();
    let mut file = fs::File::open(path).map_err(asset)?;
    let mut buffer = [0u8; 65536];
    loop {
        let count = file.read(&mut buffer).map_err(asset)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
pub fn bundle_hash(root: &Path) -> Result<String> {
    let mut hash = Sha256::new();
    hash.update(b"oxitone-vst3-bundle-v1\0");
    let mut pending = vec![(root.to_path_buf(), 0)];
    let (mut count, mut bytes) = (0usize, 0u64);
    while let Some((path, depth)) = pending.pop() {
        count += 1;
        if count > 65536 || depth > 64 {
            return Err(Error::new(
                "BudgetExceeded",
                "VST3 bundle entry/depth budget exceeded",
            ));
        }
        let metadata = fs::symlink_metadata(&path).map_err(asset)?;
        if metadata.file_type().is_symlink() {
            return Err(asset("VST3 bundle symlinks are unsupported"));
        }
        let relative = path
            .strip_prefix(root)
            .unwrap()
            .to_str()
            .ok_or_else(|| asset("non-UTF8 bundle path"))?;
        hash.update((relative.len() as u64).to_le_bytes());
        hash.update(relative.as_bytes());
        if metadata.is_dir() {
            hash.update(b"D");
            let mut children = Vec::new();
            for child in fs::read_dir(&path).map_err(asset)? {
                if children.len() + pending.len() + count >= 65536 {
                    return Err(Error::new(
                        "BudgetExceeded",
                        "VST3 bundle entry budget exceeded",
                    ));
                }
                children.push(child.map_err(asset)?.path());
            }
            children.sort();
            pending.extend(children.into_iter().rev().map(|path| (path, depth + 1)));
        } else if metadata.is_file() {
            hash.update(b"F");
            hash.update(metadata.len().to_le_bytes());
            bytes = bytes
                .checked_add(metadata.len())
                .ok_or_else(|| asset("bundle size overflow"))?;
            if bytes > 1024 * 1024 * 1024 {
                return Err(Error::new("BudgetExceeded", "VST3 bundle exceeds 1 GiB"));
            }
            let mut file = fs::File::open(&path).map_err(asset)?;
            let mut buffer = [0u8; 65536];
            let mut read = 0u64;
            loop {
                let n = file.read(&mut buffer).map_err(asset)?;
                if n == 0 {
                    break;
                }
                read += n as u64;
                if read > metadata.len() {
                    return Err(Error::new("SourceChanged", "bundle grew during hash"));
                }
                hash.update(&buffer[..n]);
            }
            let after = fs::symlink_metadata(&path).map_err(asset)?;
            if read != metadata.len()
                || after.modified().ok() != metadata.modified().ok()
                || !after.is_file()
            {
                return Err(Error::new("SourceChanged", "bundle changed during hash"));
            }
        } else {
            return Err(asset("non-regular bundle entry"));
        }
    }
    Ok(format!("{:x}", hash.finalize()))
}
pub fn asset(error: impl ToString) -> Error {
    Error::new("AssetUnavailable", error)
}
