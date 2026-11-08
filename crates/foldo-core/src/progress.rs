//! 扫描进度和取消：界面在扫描大文件夹时显示进度，也可以中途取消。

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// 一次扫描的进度。可以在另一个线程里读取进度、请求取消。
#[derive(Debug, Default)]
pub struct ScanControl {
    found: AtomicUsize,
    checked: AtomicUsize,
    cancel: AtomicBool,
}

impl ScanControl {
    /// 开始新的一次扫描前清零。
    pub fn reset(&self) {
        self.found.store(0, Ordering::Relaxed);
        self.checked.store(0, Ordering::Relaxed);
        self.cancel.store(false, Ordering::Relaxed);
    }

    /// 请求取消，扫描会在下一个检查点停下。
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// (找到的文件数, 已按规则检查的文件数)
    pub fn progress(&self) -> (usize, usize) {
        (
            self.found.load(Ordering::Relaxed),
            self.checked.load(Ordering::Relaxed),
        )
    }

    pub(crate) fn add_found(&self) {
        self.found.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_checked(&self) {
        self.checked.fetch_add(1, Ordering::Relaxed);
    }
}
