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
