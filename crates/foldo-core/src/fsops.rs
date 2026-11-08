//! 底层文件操作：跨磁盘移动、带校验的复制、移入回收站。

use filetime::FileTime;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// 路径上是否已经有东西（不跟随符号链接）。
pub fn exists(p: &Path) -> bool {
    p.symlink_metadata().is_ok()
}

/// 移动文件或文件夹。同一磁盘内直接 rename；跨磁盘时退回到“复制 → 校验 → 删除源”。
pub fn move_file(src: &Path, dst: &Path) -> io::Result<()> {
    match fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::CrossesDevices => {
            if src.is_dir() {
                // 整个文件夹复制完成后才删除源文件夹；复制失败时删掉复制了一半的目标
                if let Err(e) = copy_tree(src, dst) {
                    let _ = fs::remove_dir_all(dst);
                    return Err(e);
                }
                fs::remove_dir_all(src)
            } else {
                copy_file(src, dst)?;
                fs::remove_file(src)
            }
        }
        Err(e) => Err(e),
    }
}

/// 复制整个文件夹（不跟随符号链接，链接本身照样复制）。`dst` 不能已存在。
pub fn copy_tree(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir(dst)?;
    for entry in walkdir::WalkDir::new(src).min_depth(1).follow_links(false) {
        let entry = entry.map_err(io::Error::other)?;
        let rel = entry.path().strip_prefix(src).map_err(io::Error::other)?;
        let target = dst.join(rel);
        let ft = entry.file_type();
        if ft.is_dir() {
            fs::create_dir_all(&target)?;
        } else if ft.is_symlink() {
            let link = fs::read_link(entry.path())?;
            #[cfg(unix)]
            std::os::unix::fs::symlink(link, &target)?;
            #[cfg(windows)]
            {
                let _ = link;
                fs::copy(entry.path(), &target)?;
            }
        } else {
            copy_file(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// 复制文件并校验大小，保留修改时间。
pub fn copy_file(src: &Path, dst: &Path) -> io::Result<()> {
    let md = fs::metadata(src)?;
    let written = fs::copy(src, dst)?;
    if written != md.len() {
        let _ = fs::remove_file(dst);
        return Err(io::Error::other(crate::t!(
            "复制不完整：写入 {written} 字节，源文件 {a0} 字节",
            written = written,
            a0 = md.len()
        )));
    }
    let _ = filetime::set_file_mtime(dst, FileTime::from_last_modification_time(&md));
    Ok(())
}

/// 移入系统回收站。
pub fn trash(p: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        // NSFileManager 方式不需要“自动化控制 Finder”的权限，也不会播放音效。
        use trash::macos::{DeleteMethod, TrashContextExtMacos};
        let mut ctx = trash::TrashContext::default();
        ctx.set_delete_method(DeleteMethod::NsFileManager);
        ctx.delete(p).map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        trash::delete(p).map_err(|e| e.to_string())
    }
}

/// 确保 `dst` 的父目录存在，返回本次新建的目录（从外到内）。
pub fn ensure_parent(dst: &Path) -> io::Result<Vec<PathBuf>> {
    let Some(parent) = dst.parent() else {
        return Ok(Vec::new());
    };
    let mut missing = Vec::new();
    let mut cur = Some(parent);
    while let Some(d) = cur {
        if d.as_os_str().is_empty() || exists(d) {
            break;
        }
        missing.push(d.to_path_buf());
        cur = d.parent();
    }
    if !missing.is_empty() {
        fs::create_dir_all(parent)?;
        missing.reverse();
    }
    Ok(missing)
}

/// 能解压的压缩包格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveKind {
    Zip,
    SevenZ,
    Rar,
    Tar,
    TarGz,
}

/// 按文件名判断压缩包格式；不支持的返回 None。
pub fn archive_kind(path: &Path) -> Option<ArchiveKind> {
    let name = path.file_name()?.to_string_lossy().to_lowercase();
    Some(if name.ends_with(".zip") {
        ArchiveKind::Zip
    } else if name.ends_with(".7z") {
        ArchiveKind::SevenZ
    } else if name.ends_with(".rar") {
        ArchiveKind::Rar
    } else if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        ArchiveKind::TarGz
    } else if name.ends_with(".tar") {
        ArchiveKind::Tar
    } else {
        return None;
    })
}

/// 把压缩包里的相对路径接到 `dir` 下；绝对路径和 `..` 这类会越出目标文件夹的项目返回 None（跳过）。
fn safe_join(dir: &Path, name: &Path) -> Option<PathBuf> {
    use std::path::Component;
    let mut out = dir.to_path_buf();
    let mut any = false;
    for c in name.components() {
        match c {
            Component::Normal(p) => {
                out.push(p);
                any = true;
            }
            Component::CurDir => {}
            _ => return None,
        }
    }
    any.then_some(out)
}

fn write_entry(path: &Path, reader: &mut dyn io::Read) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut out = fs::File::create(path)?;
    io::copy(reader, &mut out)?;
    Ok(())
}

/// 解压到 `dir`（`dir` 不能已存在）：支持 zip、7z、rar、tar、tar.gz。
/// 越出目标文件夹的项目会被跳过；失败时删掉解压了一半的文件夹。
pub fn extract_archive(archive: &Path, dir: &Path) -> io::Result<()> {
    let kind =
        archive_kind(archive).ok_or_else(|| io::Error::other(crate::t!("不支持的压缩包格式")))?;
    if exists(dir) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            crate::t!("目标文件夹已存在"),
        ));
    }
    fs::create_dir_all(dir)?;
    let result = match kind {
        ArchiveKind::Zip => extract_zip(archive, dir),
        ArchiveKind::SevenZ => extract_7z(archive, dir),
        ArchiveKind::Rar => extract_rar(archive, dir),
        ArchiveKind::Tar => extract_tar(fs::File::open(archive)?, dir),
        ArchiveKind::TarGz => {
            extract_tar(flate2::read::GzDecoder::new(fs::File::open(archive)?), dir)
        }
    };
    if result.is_err() {
        let _ = fs::remove_dir_all(dir);
    }
    result
}

fn extract_zip(archive: &Path, dir: &Path) -> io::Result<()> {
    let file = fs::File::open(archive)?;
    let mut zip = zip::ZipArchive::new(io::BufReader::new(file)).map_err(io::Error::other)?;
    // extract 会拒绝 ../ 之类越出目标文件夹的路径
    zip.extract(dir).map_err(io::Error::other)
}

fn extract_7z(archive: &Path, dir: &Path) -> io::Result<()> {
    let mut reader = sevenz_rust2::ArchiveReader::open(archive, sevenz_rust2::Password::empty())
        .map_err(io::Error::other)?;
    reader
        .for_each_entries(|entry, data| {
            let Some(path) = safe_join(dir, Path::new(entry.name())) else {
                return Ok(true);
            };
            if entry.is_directory() {
                fs::create_dir_all(&path)?;
            } else {
                write_entry(&path, data)?;
            }
            Ok(true)
        })
        .map_err(io::Error::other)
}

fn extract_rar(archive: &Path, dir: &Path) -> io::Result<()> {
    let mut open = unrar::Archive::new(archive)
        .open_for_processing()
        .map_err(io::Error::other)?;
    while let Some(header) = open.read_header().map_err(io::Error::other)? {
        let entry = header.entry();
        let target = safe_join(dir, &entry.filename);
        open = match target {
            Some(path) if entry.is_file() => {
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)?;
                }
                header.extract_to(&path).map_err(io::Error::other)?
            }
            Some(path) => {
                if entry.is_directory() {
                    fs::create_dir_all(&path)?;
                }
                header.skip().map_err(io::Error::other)?
            }
            None => header.skip().map_err(io::Error::other)?,
        };
    }
    Ok(())
}

fn extract_tar(reader: impl io::Read, dir: &Path) -> io::Result<()> {
    let mut tar = tar::Archive::new(reader);
    for entry in tar.entries()? {
        // unpack_in 会跳过越出目标文件夹的项目
        entry?.unpack_in(dir)?;
    }
    Ok(())
}

/// 把整个文件夹压缩成 zip，压缩包里保留文件夹本身这一层（`dst` 不能已存在）。
fn compress_dir(src: &Path, dst: &Path) -> io::Result<()> {
    use std::io::Write;
    let root = src
        .file_name()
        .ok_or_else(|| io::Error::other(crate::t!("无法获取文件夹名")))?
        .to_string_lossy()
        .into_owned();
    let out = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dst)?;
    let result = (|| {
        let mut zip = zip::ZipWriter::new(io::BufWriter::new(out));
        for entry in walkdir::WalkDir::new(src)
            .follow_links(false)
            .sort_by_file_name()
        {
            let entry = entry.map_err(io::Error::other)?;
            let rel = entry.path().strip_prefix(src).map_err(io::Error::other)?;
            let mut name = root.clone();
            for c in rel.components() {
                name.push('/');
                name.push_str(&c.as_os_str().to_string_lossy());
            }
            let md = entry.metadata().map_err(io::Error::other)?;
            let opts = zip_options(&md);
            if entry.file_type().is_dir() {
                zip.add_directory(format!("{name}/"), opts)
                    .map_err(io::Error::other)?;
            } else if entry.file_type().is_file() {
                zip.start_file(name, opts.large_file(md.len() >= u32::MAX as u64))
                    .map_err(io::Error::other)?;
                io::copy(
                    &mut io::BufReader::new(fs::File::open(entry.path())?),
                    &mut zip,
                )?;
            }
        }
        zip.finish().map_err(io::Error::other)?.flush()
    })();
    if result.is_err() {
        let _ = fs::remove_file(dst);
    }
    result
}

/// 压缩选项：Deflate，并保留修改时间（zip 用本地时间）。
fn zip_options(md: &fs::Metadata) -> zip::write::SimpleFileOptions {
    let mut opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    if let Ok(t) = md.modified() {
        use chrono::{Datelike, Timelike};
        let t: chrono::DateTime<chrono::Local> = t.into();
        if let Ok(dt) = zip::DateTime::from_date_and_time(
            t.year().clamp(1980, 2107) as u16,
            t.month() as u8,
            t.day() as u8,
            t.hour() as u8,
            t.minute() as u8,
            t.second() as u8,
        ) {
            opts = opts.last_modified_time(dt);
        }
    }
    opts
}

/// 把文件或文件夹压缩成 zip（`dst` 不能已存在）。
pub fn compress(src: &Path, dst: &Path) -> io::Result<()> {
    if src.is_dir() {
        compress_dir(src, dst)
    } else {
        compress_file(src, dst)
    }
}

/// 把单个文件压缩成 zip（`dst` 不能已存在）。
fn compress_file(src: &Path, dst: &Path) -> io::Result<()> {
    use std::io::Write;
    let name = src
        .file_name()
        .ok_or_else(|| io::Error::other(crate::t!("无法获取文件名")))?
        .to_string_lossy()
        .into_owned();
    let out = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dst)?;
    let result = (|| {
        let mut zip = zip::ZipWriter::new(io::BufWriter::new(out));
        let md = fs::metadata(src)?;
        let opts = zip_options(&md).large_file(md.len() >= u32::MAX as u64);
        zip.start_file(name, opts).map_err(io::Error::other)?;
        io::copy(&mut io::BufReader::new(fs::File::open(src)?), &mut zip)?;
        zip.finish().map_err(io::Error::other)?.flush()
    })();
    if result.is_err() {
        let _ = fs::remove_file(dst);
    }
    result
}

/// 创建指向 `target` 的快捷方式：Windows 为 .lnk，其他系统为符号链接。
pub fn create_shortcut(target: &Path, link: &Path) -> io::Result<()> {
    if exists(link) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            crate::t!("已有同名文件"),
        ));
    }
    #[cfg(windows)]
    {
        let lnk = mslnk::ShellLink::new(target).map_err(io::Error::other)?;
        lnk.create_lnk(link).map_err(io::Error::other)
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)
    }
}
