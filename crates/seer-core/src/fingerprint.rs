//! A cheap identity for a file's contents, used as a cache key.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::time::UNIX_EPOCH;
use xxhash_rust::xxh3::Xxh3;

const EDGE: u64 = 8 * 1024;

/// Hash of size, modification time and the first and last 8 KB.
///
/// Reads at most 16 KB whatever the file size. This is not a content hash: it
/// answers "is this the file I analysed before?", nothing stronger.
pub fn fingerprint(path: &Path) -> std::io::Result<String> {
    let mut file = File::open(path)?;
    let meta = file.metadata()?;
    let len = meta.len();
    let mtime = meta
        .modified()?
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    let mut hasher = Xxh3::new();
    hasher.update(&len.to_le_bytes());
    hasher.update(&mtime.to_le_bytes());

    let mut buf = vec![0u8; EDGE as usize];
    let head = read_up_to(&mut file, &mut buf)?;
    hasher.update(&buf[..head]);
    if len > EDGE {
        file.seek(SeekFrom::Start(len - EDGE.min(len - EDGE)))?;
        let tail = read_up_to(&mut file, &mut buf)?;
        hasher.update(&buf[..tail]);
    }
    Ok(format!("{:032x}", hasher.digest128()))
}

fn read_up_to(file: &mut File, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match file.read(&mut buf[filled..])? {
            0 => break,
            n => filled += n,
        }
    }
    Ok(filled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changes_with_content_and_is_stable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f.bin");
        std::fs::write(&path, vec![1u8; 40_000]).unwrap();
        let a = fingerprint(&path).unwrap();
        assert_eq!(a, fingerprint(&path).unwrap());
        assert_eq!(a.len(), 32);

        let mut data = vec![1u8; 40_000];
        *data.last_mut().unwrap() = 2;
        std::fs::write(&path, data).unwrap();
        assert_ne!(a, fingerprint(&path).unwrap());
    }

    #[test]
    fn handles_small_and_empty_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f.bin");
        std::fs::write(&path, []).unwrap();
        fingerprint(&path).unwrap();
        std::fs::write(&path, [7u8; 9000]).unwrap();
        fingerprint(&path).unwrap();
    }
}
