//! 重复文件：全部整理文件夹里内容完全相同的文件（拖入的文件之间也会比较）。
//!
//! 先按大小分组，只有大小相同的文件才读内容比较，所以几乎没有额外开销。
//! 每组相同的文件里，修改时间最早的一份（相同时按路径排序取第一个）算“原件”，其余算“重复”。
//! 这样“重复文件 → 移到回收站”这样的规则总会留下一份。

use chrono::{DateTime, Local};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::File;
use std::hash::Hasher;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::SystemTime;

/// 文件的“版本”：大小和修改时间都没变，就认为内容没变。
type Stamp = (u64, Option<SystemTime>);

fn stamp(path: &Path) -> Option<Stamp> {
    let md = std::fs::metadata(path).ok()?;
    Some((md.len(), md.modified().ok()))
}

/// 跨扫描的缓存（后台每次检查都会重新扫描，内容没变的文件不必重新读）。
static DIGESTS: LazyLock<Mutex<HashMap<PathBuf, (Stamp, u64)>>> = LazyLock::new(Default::default);
/// (文件 A, 文件 B) → (A 的版本, B 的版本, 内容是否相同)
type SameCache = HashMap<(PathBuf, PathBuf), (Stamp, Stamp, bool)>;
static SAME: LazyLock<Mutex<SameCache>> = LazyLock::new(Default::default);
/// 缓存最多记这么多项，超过就清空重来
const CACHE_LIMIT: usize = 50_000;
thread_local! {
    /// 本线程实际读文件内容的次数（测试缓存用；扫描在一个线程里完成）
    static READS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// 本线程为判断重复实际读取文件内容的次数。
pub fn content_reads() -> u64 {
    READS.with(|r| r.get())
}

fn count_read() {
    READS.with(|r| r.set(r.get() + 1));
}

fn cached_digest(path: &Path) -> Option<u64> {
    let st = stamp(path)?;
    if let Some((s, d)) = DIGESTS.lock().ok()?.get(path)
        && *s == st
    {
        return Some(*d);
    }
    let d = digest(path)?;
    if let Ok(mut m) = DIGESTS.lock() {
        if m.len() > CACHE_LIMIT {
            m.clear();
        }
        m.insert(path.to_path_buf(), (st, d));
    }
    Some(d)
}

fn cached_same(a: &Path, b: &Path) -> bool {
    let (Some(sa), Some(sb)) = (stamp(a), stamp(b)) else {
        return false;
    };
    let key = (a.to_path_buf(), b.to_path_buf());
    if let Ok(m) = SAME.lock()
        && let Some((x, y, same)) = m.get(&key)
        && *x == sa
        && *y == sb
    {
        return *same;
    }
    let same = same_content(a, b);
    if let Ok(mut m) = SAME.lock() {
        if m.len() > CACHE_LIMIT {
            m.clear();
        }
        m.insert(key, (sa, sb, same));
    }
    same
}

#[derive(Debug)]
struct Peer {
    path: PathBuf,
    size: u64,
    modified: Option<DateTime<Local>>,
}

/// 一次扫描里的全部文件，供判断重复用。
#[derive(Debug, Default)]
pub struct Peers {
    files: Vec<Peer>,
    /// 内容摘要缓存；读不了的文件为 None
    digests: RefCell<HashMap<PathBuf, Option<u64>>>,
}

impl Peers {
    pub fn new<'a>(
        files: impl IntoIterator<Item = (&'a Path, u64, Option<DateTime<Local>>)>,
    ) -> Self {
        Self {
            files: files
                .into_iter()
                .map(|(p, size, modified)| Peer {
                    path: p.to_path_buf(),
                    size,
                    modified,
                })
                .collect(),
            digests: RefCell::default(),
        }
    }

    fn digest(&self, path: &Path) -> Option<u64> {
        if let Some(d) = self.digests.borrow().get(path) {
            return *d;
        }
        let d = cached_digest(path);
        self.digests.borrow_mut().insert(path.to_path_buf(), d);
        d
    }

    /// 与 `path` 内容相同、且算作原件的文件。`path` 本身是原件或没有重复时返回 None。
    pub fn original_of(&self, path: &Path, size: u64) -> Option<PathBuf> {
        if size == 0 {
            // 空文件都“相同”，没有意义
            return None;
        }
        let me = self.files.iter().find(|p| p.path == path)?;
        let rank = |p: &Peer| (p.modified, p.path.clone());
        let mut same: Vec<&Peer> = self
            .files
            .iter()
            .filter(|p| p.size == size && p.path != path)
            .collect();
        if same.is_empty() {
            return None;
        }
        let mine = self.digest(path)?;
        same.retain(|p| self.digest(&p.path) == Some(mine) && cached_same(path, &p.path));
        let first = same.into_iter().min_by_key(|p| rank(p))?;
        (rank(first) < rank(me)).then(|| first.path.clone())
    }
}

fn digest(path: &Path) -> Option<u64> {
    count_read();
    let mut r = BufReader::with_capacity(1 << 16, File::open(path).ok()?);
    let mut h = std::hash::DefaultHasher::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = r.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        h.write(&buf[..n]);
    }
    Some(h.finish())
}

/// 摘要相同后再逐字节确认，避免哈希碰撞误判。
fn same_content(a: &Path, b: &Path) -> bool {
    count_read();
    let (Ok(fa), Ok(fb)) = (File::open(a), File::open(b)) else {
        return false;
    };
    let (mut ra, mut rb) = (BufReader::new(fa), BufReader::new(fb));
    let (mut ba, mut bb) = (vec![0u8; 1 << 16], vec![0u8; 1 << 16]);
    loop {
        let n = match ra.read(&mut ba) {
            Ok(n) => n,
            Err(_) => return false,
        };
        if rb.read_exact(&mut bb[..n]).is_err() || ba[..n] != bb[..n] {
            return false;
        }
        if n == 0 {
            return rb.read(&mut bb[..1]).is_ok_and(|m| m == 0);
        }
    }
}
