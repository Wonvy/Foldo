//! 底层文件操作：跨磁盘移动、带校验的复制、移入回收站。

use filetime::FileTime;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// 路径上是否已经有东西（不跟随符号链接）。
pub fn exists(p: &Path) -> bool {
    p.symlink_metadata().is_ok()
}

/// 移动文件。同一磁盘内直接 rename；跨磁盘时退回到“复制 → 校验 → 删除源文件”。
pub fn move_file(src: &Path, dst: &Path) -> io::Result<()> {
    match fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::CrossesDevices => {
            copy_file(src, dst)?;
            fs::remove_file(src)
        }
        Err(e) => Err(e),
    }
}

/// 复制文件并校验大小，保留修改时间。
pub fn copy_file(src: &Path, dst: &Path) -> io::Result<()> {
    let md = fs::metadata(src)?;
    let written = fs::copy(src, dst)?;
    if written != md.len() {
        let _ = fs::remove_file(dst);
        return Err(io::Error::other(format!(
            "复制不完整：写入 {written} 字节，源文件 {} 字节",
            md.len()
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

/// 把 zip 解压到 `dir`（`dir` 不能已存在）。失败时删掉解压了一半的文件夹。
pub fn extract_zip(archive: &Path, dir: &Path) -> io::Result<()> {
    if exists(dir) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "目标文件夹已存在",
        ));
    }
    let file = fs::File::open(archive)?;
    let mut zip = zip::ZipArchive::new(io::BufReader::new(file)).map_err(io::Error::other)?;
    fs::create_dir_all(dir)?;
    // extract 会拒绝 ../ 之类越出目标文件夹的路径
    zip.extract(dir).map_err(|e| {
        let _ = fs::remove_dir_all(dir);
        io::Error::other(e)
    })
}

/// 把单个文件压缩成 zip（`dst` 不能已存在）。
pub fn compress_file(src: &Path, dst: &Path) -> io::Result<()> {
    use std::io::Write;
    let name = src
        .file_name()
        .ok_or_else(|| io::Error::other("无法获取文件名"))?
        .to_string_lossy()
        .into_owned();
    let out = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dst)?;
    let result = (|| {
        let mut zip = zip::ZipWriter::new(io::BufWriter::new(out));
        let md = fs::metadata(src)?;
        let mut opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .large_file(md.len() >= u32::MAX as u64);
        // 压缩包里保留文件原来的修改时间（zip 用本地时间）
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
        return Err(io::Error::new(io::ErrorKind::AlreadyExists, "已有同名文件"));
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
