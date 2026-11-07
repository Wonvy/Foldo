//! 规则里的大小（`50KB`）和时长（`3d`、`1d12h`）解析。

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::fmt;
use std::str::FromStr;
use std::time::Duration;

/// 字节数。`KB/MB/GB/TB` 按 1000 进制，`KiB/MiB/GiB/TiB` 按 1024 进制。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ByteSize(pub u64);

impl FromStr for ByteSize {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        let s = s.trim();
        let split = s
            .find(|c: char| !(c.is_ascii_digit() || c == '.'))
            .unwrap_or(s.len());
        let (num, unit) = s.split_at(split);
        let n: f64 = num
            .parse()
            .map_err(|_| format!("无法解析大小「{s}」，示例：500KB、1.5MB、2GiB"))?;
        let mul: u64 = match unit.trim().to_ascii_lowercase().as_str() {
            "" | "b" => 1,
            "k" | "kb" => 1_000,
            "kib" => 1 << 10,
            "m" | "mb" => 1_000_000,
            "mib" => 1 << 20,
            "g" | "gb" => 1_000_000_000,
            "gib" => 1 << 30,
            "t" | "tb" => 1_000_000_000_000,
            "tib" => 1 << 40,
            other => return Err(format!("未知的大小单位「{other}」")),
        };
        Ok(ByteSize((n * mul as f64).round() as u64))
    }
}

/// 时长。支持 `s` 秒、`m` 分、`h` 时、`d` 天、`w` 周、`mo` 月（按 30 天）、`y` 年（按 365 天），可组合如 `1d12h`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span(pub Duration);

impl FromStr for Span {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        let src = s.trim();
        let bad = || format!("无法解析时长「{src}」，示例：30m、12h、3d、2w、6mo、1d12h");
        let mut rest = src;
        let mut total: u64 = 0;
        if rest.is_empty() {
            return Err(bad());
        }
        while !rest.is_empty() {
            let num_end = rest.find(|c: char| !c.is_ascii_digit()).ok_or_else(bad)?;
            if num_end == 0 {
                return Err(bad());
            }
            let n: u64 = rest[..num_end].parse().map_err(|_| bad())?;
            rest = &rest[num_end..];
            let unit_end = rest
                .find(|c: char| !c.is_ascii_alphabetic())
                .unwrap_or(rest.len());
            let secs: u64 = match rest[..unit_end].to_ascii_lowercase().as_str() {
                "s" => 1,
                "m" | "min" => 60,
                "h" => 3_600,
                "d" => 86_400,
                "w" => 7 * 86_400,
                "mo" => 30 * 86_400,
                "y" => 365 * 86_400,
                _ => return Err(bad()),
            };
            total = total.saturating_add(n.saturating_mul(secs));
            rest = rest[unit_end..].trim_start();
        }
        Ok(Span(Duration::from_secs(total)))
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum NumOrStr {
    Num(u64),
    Str(String),
}

impl<'de> Deserialize<'de> for ByteSize {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        match NumOrStr::deserialize(d)? {
            NumOrStr::Num(n) => Ok(ByteSize(n)),
            NumOrStr::Str(s) => s.parse().map_err(de::Error::custom),
        }
    }
}

impl<'de> Deserialize<'de> for Span {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        match NumOrStr::deserialize(d)? {
            NumOrStr::Num(_) => Err(de::Error::custom("时长需要带单位，例如 3d、12h、30m")),
            NumOrStr::Str(s) => s.parse().map_err(de::Error::custom),
        }
    }
}

/// 写回时用能整除的最大单位：`10MB`、`512KiB`、`1500`。
impl fmt::Display for ByteSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let n = self.0;
        const UNITS: &[(u64, &str)] = &[
            (1_000_000_000_000, "TB"),
            (1 << 40, "TiB"),
            (1_000_000_000, "GB"),
            (1 << 30, "GiB"),
            (1_000_000, "MB"),
            (1 << 20, "MiB"),
            (1_000, "KB"),
            (1 << 10, "KiB"),
        ];
        for &(size, unit) in UNITS {
            if n >= size && n.is_multiple_of(size) {
                return write!(f, "{}{unit}", n / size);
            }
        }
        write!(f, "{n}")
    }
}

/// 写回时用能整除的最大单位：`2w`、`3d`、`36h`、`90m`、`45s`。
impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = self.0.as_secs();
        const UNITS: &[(u64, &str)] = &[(7 * 86_400, "w"), (86_400, "d"), (3_600, "h"), (60, "m")];
        for &(size, unit) in UNITS {
            if s >= size && s.is_multiple_of(size) {
                return write!(f, "{}{unit}", s / size);
            }
        }
        write!(f, "{s}s")
    }
}

impl Serialize for ByteSize {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        if self.0.is_multiple_of(1000) || self.0.is_multiple_of(1024) {
            s.serialize_str(&self.to_string())
        } else {
            s.serialize_u64(self.0)
        }
    }
}

impl Serialize for Span {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_roundtrip() {
        for s in ["10MB", "512KiB", "3GB", "1500"] {
            assert_eq!(s.parse::<ByteSize>().unwrap().to_string(), s);
        }
        for s in ["2w", "3d", "36h", "90m", "45s"] {
            assert_eq!(s.parse::<Span>().unwrap().to_string(), s);
        }
        assert_eq!("1d12h".parse::<Span>().unwrap().to_string(), "36h");
    }

    #[test]
    fn sizes() {
        assert_eq!("500".parse::<ByteSize>().unwrap().0, 500);
        assert_eq!("50KB".parse::<ByteSize>().unwrap().0, 50_000);
        assert_eq!("1.5 MB".parse::<ByteSize>().unwrap().0, 1_500_000);
        assert_eq!("2GiB".parse::<ByteSize>().unwrap().0, 2 << 30);
        assert!("12XB".parse::<ByteSize>().is_err());
        assert!("MB".parse::<ByteSize>().is_err());
    }

    #[test]
    fn spans() {
        let secs = |s: &str| s.parse::<Span>().unwrap().0.as_secs();
        assert_eq!(secs("30s"), 30);
        assert_eq!(secs("10m"), 600);
        assert_eq!(secs("3d"), 3 * 86_400);
        assert_eq!(secs("1d12h"), 86_400 + 12 * 3_600);
        assert_eq!(secs("6mo"), 180 * 86_400);
        assert!("3".parse::<Span>().is_err());
        assert!("d".parse::<Span>().is_err());
        assert!("3x".parse::<Span>().is_err());
    }
}
