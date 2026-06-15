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
    /// 实际匹配上的 binary 名 (从 `bin_candidates` 里挑出来的那一个,
    /// 通常 == 文件名)。install.rs 用这个写 .cargo/bin/<name>。
    pub binary_name: String,
}

/// 解压 `archive_path` (本身在另一个临时目录里) 到一个新临时目录,
/// 然后在里面挨个尝试 `bin_candidates` 里的 binary 名, 首个找到的胜出。
///
/// 候选列表设计动机: 包名 != binary 名 (ripgrep -> rg, tauri-cli -> cargo-tauri)
/// 时仅传 spec.name 会找不到文件 → 失败回 cargo install。
/// 这里接收 .crates2.json 的 bins[] 整段, 容忍这类不一致。
pub fn extract(
    archive_path: &Path,
    fmt: ArchiveFmt,
    bin_candidates: &[String],
) -> Result<ExtractResult, DownloaderError> {
    if bin_candidates.is_empty() {
        return Err(DownloaderError::Unsupported(UnsupportedReason::UnknownArchiveFormat));
    }
    let temp_dir = tempfile::tempdir()
        .map_err(|e| failed_extract(anyhow!(e).context("mkdir tempdir for extract")))?;

    match fmt {
        ArchiveFmt::TarGz => extract_targz(archive_path, temp_dir.path())
            .map_err(|e| failed_extract(e.context("extract tar.gz")))?,
        ArchiveFmt::Zip => extract_zip(archive_path, temp_dir.path())
            .map_err(|e| failed_extract(e.context("extract zip")))?,
        ArchiveFmt::Bin => {
            // 裸二进制: 直接拷过去, 第一个候选当文件名
            let dest = temp_dir.path().join(&bin_candidates[0]);
            std::fs::copy(archive_path, &dest)
                .map_err(|e| failed_extract(anyhow!(e).context("copy raw bin")))?;
            #[cfg(unix)]
            set_executable(&dest)
                .map_err(|e| failed_extract(e.context("chmod +x bin")))?;
        }
    }

    for name in bin_candidates {
        if let Some(binary_path) = find_binary(temp_dir.path(), name) {
            return Ok(ExtractResult {
                temp_dir,
                binary_path,
                binary_name: name.clone(),
            });
        }
    }
    Err(DownloaderError::Unsupported(UnsupportedReason::UnknownArchiveFormat))
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
            if p.is_file() && p.file_name().is_some_and(|f| f == name) {
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
        let r = extract(&fixture("ripgrep-like.tar.gz"), ArchiveFmt::TarGz, &["rg".into()])
            .expect("extract ok");
        assert!(r.binary_path.exists());
        assert!(r.binary_path.ends_with("rg"));
    }

    #[test]
    fn extract_mdbook_like_targz_root_binary() {
        let r = extract(&fixture("mdbook-like.tar.gz"), ArchiveFmt::TarGz, &["mdbook".into()])
            .expect("extract ok");
        assert!(r.binary_path.exists());
        assert!(r.binary_path.ends_with("mdbook"));
    }

    #[test]
    fn extract_cargo_deny_like_zip() {
        let r = extract(
            &fixture("cargo-deny-like.zip"),
            ArchiveFmt::Zip,
            &["cargo-deny".into()],
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
            &["no-such-binary".into()],
        )
        .unwrap_err();
        assert!(matches!(
            err,
            DownloaderError::Unsupported(UnsupportedReason::UnknownArchiveFormat)
        ));
    }

    #[test]
    fn temp_dir_cleaned_up_on_drop() {
        let r = extract(&fixture("mdbook-like.tar.gz"), ArchiveFmt::TarGz, &["mdbook".into()])
            .expect("extract ok");
        let dir_path = r.temp_dir.path().to_owned();
        assert!(dir_path.exists());
        drop(r);
        assert!(!dir_path.exists(), "temp_dir should be cleaned on Drop");
    }
}
