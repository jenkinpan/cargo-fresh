//! 把下载的归档解压到临时目录, 定位 binary 路径。
//!
//! 支持 tar.gz / zip / 裸二进制三种格式。tar 流式读取避免一次性
//! 读入大文件内存。返回临时目录 + binary 在里面的相对路径。

use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};

use crate::downloader::events::{DownloaderError, FailureKind, UnsupportedReason};
use crate::downloader::resolve::ArchiveFmt;

/// 解压结果。`_temp_dir` 通过 RAII 保活整段调用——caller 用完调 `Drop`
/// (即丢弃 ExtractResult) 时整个临时目录会被 rm -rf。
#[derive(Debug)]
pub struct ExtractResult {
    /// 持有它确保 Drop 时清理临时目录
    pub temp_dir: tempfile::TempDir,
    /// 绝对路径到解压出来的可执行文件
    pub binary_path: PathBuf,
}

/// 解压 `archive_path` (本身在另一个临时目录里) 到一个新临时目录,
/// 在里面找到名为 `binary_name` 的可执行文件并返回路径。
pub fn extract(
    archive_path: &Path,
    fmt: ArchiveFmt,
    binary_name: &str,
) -> Result<ExtractResult, DownloaderError> {
    let temp_dir = tempfile::tempdir()
        .map_err(|e| failed_extract(anyhow!(e).context("mkdir tempdir for extract")))?;

    match fmt {
        ArchiveFmt::TarGz => extract_targz(archive_path, temp_dir.path())
            .map_err(|e| failed_extract(e.context("extract tar.gz")))?,
        ArchiveFmt::Zip => extract_zip(archive_path, temp_dir.path())
            .map_err(|e| failed_extract(e.context("extract zip")))?,
        ArchiveFmt::Bin => {
            // 裸二进制: 直接拷过去, binary_name 就是文件名
            let dest = temp_dir.path().join(binary_name);
            std::fs::copy(archive_path, &dest)
                .map_err(|e| failed_extract(anyhow!(e).context("copy raw bin")))?;
            #[cfg(unix)]
            set_executable(&dest)
                .map_err(|e| failed_extract(e.context("chmod +x bin")))?;
        }
    }

    let binary_path = find_binary(temp_dir.path(), binary_name).ok_or_else(|| {
        DownloaderError::Unsupported(UnsupportedReason::UnknownArchiveFormat)
    })?;

    Ok(ExtractResult {
        temp_dir,
        binary_path,
    })
}

fn extract_targz(archive: &Path, into: &Path) -> Result<()> {
    let f = std::fs::File::open(archive).context("open archive")?;
    let gz = flate2::read::GzDecoder::new(f);
    let mut tar = tar::Archive::new(gz);
    tar.unpack(into).context("tar.unpack")?;
    Ok(())
}

fn extract_zip(archive: &Path, into: &Path) -> Result<()> {
    let f = std::fs::File::open(archive).context("open archive")?;
    let mut z = zip::ZipArchive::new(f).context("read zip")?;
    z.extract(into).context("zip.extract")?;
    Ok(())
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path).context("stat")?.permissions();
    perms.set_mode(perms.mode() | 0o111);
    std::fs::set_permissions(path, perms).context("set_permissions")?;
    Ok(())
}

/// 递归找名为 `name` 的文件 (优先匹配可执行位)。先广度优先扫一层,
/// 再下一层——大多数 release tarball 把 binary 放第 1-2 层。
fn find_binary(root: &Path, name: &str) -> Option<PathBuf> {
    fn walk(dir: &Path, name: &str, depth: usize) -> Option<PathBuf> {
        if depth > 3 {
            return None;
        }
        let entries = std::fs::read_dir(dir).ok()?;
        let mut subdirs = Vec::new();
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() && p.file_name().map(|f| f == name).unwrap_or(false) {
                return Some(p);
            }
            if p.is_dir() {
                subdirs.push(p);
            }
        }
        for sub in subdirs {
            if let Some(found) = walk(&sub, name, depth + 1) {
                return Some(found);
            }
        }
        None
    }
    walk(root, name, 0)
}

fn failed_extract(e: anyhow::Error) -> DownloaderError {
    DownloaderError::Failed {
        kind: FailureKind::ExtractFailed,
        source: e,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    #[test]
    fn extract_ripgrep_like_targz() {
        let r = extract(&fixture("ripgrep-like.tar.gz"), ArchiveFmt::TarGz, "rg")
            .expect("extract ok");
        assert!(r.binary_path.exists());
        assert!(r.binary_path.ends_with("rg"));
    }

    #[test]
    fn extract_mdbook_like_targz_root_binary() {
        let r = extract(&fixture("mdbook-like.tar.gz"), ArchiveFmt::TarGz, "mdbook")
            .expect("extract ok");
        assert!(r.binary_path.exists());
        assert!(r.binary_path.ends_with("mdbook"));
    }

    #[test]
    fn extract_cargo_deny_like_zip() {
        let r = extract(
            &fixture("cargo-deny-like.zip"),
            ArchiveFmt::Zip,
            "cargo-deny",
        )
        .expect("extract ok");
        assert!(r.binary_path.exists());
        assert!(r.binary_path.ends_with("cargo-deny"));
    }

    #[test]
    fn missing_binary_in_archive_returns_unsupported() {
        let err = extract(
            &fixture("ripgrep-like.tar.gz"),
            ArchiveFmt::TarGz,
            "no-such-binary",
        )
        .unwrap_err();
        assert!(matches!(
            err,
            DownloaderError::Unsupported(UnsupportedReason::UnknownArchiveFormat)
        ));
    }

    #[test]
    fn temp_dir_cleaned_up_on_drop() {
        let r = extract(&fixture("mdbook-like.tar.gz"), ArchiveFmt::TarGz, "mdbook")
            .expect("extract ok");
        let dir_path = r.temp_dir.path().to_owned();
        assert!(dir_path.exists());
        drop(r);
        assert!(!dir_path.exists(), "temp_dir should be cleaned on Drop");
    }
}
