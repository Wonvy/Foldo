//! 跨平台路径工具：`~` 展开、显示、大小写不敏感比较、危险目录检查。

use std::path::{MAIN_SEPARATOR, Path, PathBuf};

/// 展开开头的 `~`。
pub fn expand_home(s: &str) -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        if s == "~" {
            return home;
        }
        if let Some(rest) = s.strip_prefix("~/").or_else(|| s.strip_prefix("~\\")) {
            return home.join(native_separators(rest).as_ref());
        }
    }
    PathBuf::from(native_separators(s).as_ref())
}

/// Windows 上把 `/` 统一成 `\\`，避免同一个路径因为分隔符写法不同而对不上。
fn native_separators(s: &str) -> std::borrow::Cow<'_, str> {
    if cfg!(windows) && s.contains('/') {
        s.replace('/', "\\").into()
    } else {
        s.into()
    }
}

/// 用于展示：把主目录缩写成 `~`。
pub fn display(p: &Path) -> String {
    if let Some(home) = dirs::home_dir()
        && let Ok(rest) = p.strip_prefix(&home)
    {
        return if rest.as_os_str().is_empty() {
            "~".to_string()
        } else {
            format!("~{MAIN_SEPARATOR}{}", rest.display())
        };
    }
    p.display().to_string()
}

/// 用于判断“是不是同一个路径”的键。macOS 和 Windows 默认文件系统不区分大小写。
/// Windows 上还会把 `/` 统一成 `\\`。
pub fn path_key(p: &Path) -> String {
    let s = native_separators(&p.to_string_lossy()).into_owned();
    if cfg!(any(target_os = "macos", target_os = "windows")) {
        s.to_lowercase()
    } else {
        s
    }
}

/// 两个路径是否只差大小写（在不区分大小写的系统上，这种重命名是合法的）。
pub fn is_case_only_change(a: &Path, b: &Path) -> bool {
    a != b && path_key(a) == path_key(b)
}

/// 默认的规则文件路径：`<配置目录>/foldo/rules.yaml`。
/// 设置了环境变量 `FOLDO_RULES` 时以它为准。
pub fn default_rules_path() -> PathBuf {
    if let Some(p) = std::env::var_os("FOLDO_RULES").filter(|v| !v.is_empty()) {
        return PathBuf::from(p);
    }
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("foldo")
        .join("rules.yaml")
}

/// 默认的数据目录：`<数据目录>/foldo`，存放操作日志。
/// 设置了环境变量 `FOLDO_HOME` 时以它为准。
pub fn default_data_dir() -> PathBuf {
    if let Some(p) = std::env::var_os("FOLDO_HOME").filter(|v| !v.is_empty()) {
        return PathBuf::from(p);
    }
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("foldo")
}

/// 检查一个目录能不能作为规则的扫描位置，拒绝系统目录和过大的范围。
pub fn check_location(p: &Path, recursive: bool) -> Result<(), String> {
    if !p.is_absolute() {
        return Err(crate::t!(
            "位置「{a0}」必须是绝对路径或以 ~ 开头",
            a0 = p.display()
        ));
    }
    if p.parent().is_none() {
        return Err(crate::t!("不能把磁盘根目录作为整理位置"));
    }
    // 如果路径存在，用真实路径再检查一遍（处理符号链接以及 macOS 的 /etc → /private/etc）。
    let real = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    let home = dirs::home_dir();
    if recursive
        && let Some(h) = &home
        && (p == h.as_path() || real == *h)
    {
        return Err(crate::t!("不能递归整理整个用户主目录"));
    }

    let mut forbidden: Vec<PathBuf> = Vec::new();
    #[cfg(unix)]
    forbidden.extend(
        [
            "/System",
            "/Library",
            "/Applications",
            "/usr",
            "/bin",
            "/sbin",
            "/etc",
            "/private/etc",
            "/boot",
            "/dev",
            "/proc",
            "/sys",
        ]
        .map(PathBuf::from),
    );
    #[cfg(windows)]
    {
        let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
        forbidden.push(PathBuf::from(root));
        for var in ["ProgramFiles", "ProgramFiles(x86)", "ProgramData"] {
            if let Ok(v) = std::env::var(var) {
                forbidden.push(PathBuf::from(v));
            }
        }
    }
    if let Some(h) = &home {
        forbidden.extend(["Library", "AppData", ".config", ".local", ".ssh"].map(|d| h.join(d)));
    }

    for f in &forbidden {
        if p.starts_with(f) || real.starts_with(f) {
            return Err(crate::t!(
                "为了安全，不允许整理系统或应用数据目录「{a0}」",
                a0 = display(f)
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_and_display_roundtrip() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(expand_home("~"), home);
        assert_eq!(expand_home("~/Downloads"), home.join("Downloads"));
        assert_eq!(expand_home("/tmp/x"), PathBuf::from("/tmp/x"));
        assert_eq!(display(&home.join("a")), format!("~{MAIN_SEPARATOR}a"));
    }

    #[test]
    fn mixed_separators_are_the_same_path() {
        let home = dirs::home_dir().unwrap();
        let a = expand_home("~/Downloads/项目A");
        let b = home.join("Downloads").join("项目A");
        assert_eq!(path_key(&a), path_key(&b));
    }

    #[cfg(unix)]
    #[test]
    fn forbidden_locations() {
        assert!(check_location(Path::new("/"), false).is_err());
        assert!(check_location(Path::new("/usr/local/bin"), false).is_err());
        assert!(check_location(Path::new("relative/dir"), false).is_err());
        assert!(check_location(&expand_home("~/Library/Caches"), false).is_err());
        assert!(check_location(&expand_home("~"), true).is_err());
        assert!(check_location(&expand_home("~"), false).is_ok());
        assert!(check_location(&expand_home("~/Downloads"), true).is_ok());
    }
}
