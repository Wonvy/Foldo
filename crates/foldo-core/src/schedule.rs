//! 定时整理：每天或每周固定时间整理一次。这里只负责算“最近一次该整理的时刻”和“下一次”。

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, NaiveTime, TimeZone};
use serde::{Deserialize, Serialize};

/// 配置里的写法：`schedule: { every: daily, at: "09:00" }` 或 `{ every: weekly, weekday: 1, at: "18:30" }`。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScheduleDef {
    pub every: Every,
    /// 时间，24 小时制，如 "09:00"
    pub at: String,
    /// 每周的哪一天：1 = 周一 … 7 = 周日（每周时必填）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weekday: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Every {
    Daily,
    Weekly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Schedule {
    pub every: Every,
    pub time: NaiveTime,
    /// 1 = 周一 … 7 = 周日
    pub weekday: u32,
}

impl Schedule {
    pub fn compile(def: &ScheduleDef) -> Result<Self, String> {
        let time = NaiveTime::parse_from_str(def.at.trim(), "%H:%M")
            .map_err(|_| crate::t!("定时整理的时间「{at}」应写成 09:00 这样", at = def.at))?;
        let weekday = match (def.every, def.weekday) {
            (Every::Weekly, Some(d @ 1..=7)) => d as u32,
            (Every::Weekly, _) => {
                return Err(crate::t!(
                    "每周定时整理需要写 weekday（1 = 周一 … 7 = 周日）"
                ));
            }
            (Every::Daily, _) => 1,
        };
        Ok(Self {
            every: def.every,
            time,
            weekday,
        })
    }

    fn at(&self, day: NaiveDate) -> DateTime<Local> {
        let naive = day.and_time(self.time);
        // 夏令时切换时这个时刻可能不存在或出现两次：取最早的那个，不存在时往后推一小时
        Local
            .from_local_datetime(&naive)
            .earliest()
            .or_else(|| {
                Local
                    .from_local_datetime(&(naive + Duration::hours(1)))
                    .earliest()
            })
            .unwrap_or_else(Local::now)
    }

    fn period(&self) -> Duration {
        match self.every {
            Every::Daily => Duration::days(1),
            Every::Weekly => Duration::days(7),
        }
    }

    /// 不晚于 `now` 的最近一次整理时刻。
    pub fn last_due(&self, now: DateTime<Local>) -> DateTime<Local> {
        let today = now.date_naive();
        let back = match self.every {
            Every::Daily => 0,
            Every::Weekly => (now.weekday().number_from_monday() + 7 - self.weekday) % 7,
        };
        let day = today - Duration::days(back as i64);
        let t = self.at(day);
        if t <= now {
            t
        } else {
            self.at(day - self.period())
        }
    }

    /// `now` 之后的下一次整理时刻。
    pub fn next_due(&self, now: DateTime<Local>) -> DateTime<Local> {
        let last = self.last_due(now);
        self.at(last.date_naive() + self.period())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(y, m, d, h, min, 0).unwrap()
    }

    #[test]
    fn daily() {
        let s = Schedule::compile(&ScheduleDef {
            every: Every::Daily,
            at: "09:00".into(),
            weekday: None,
        })
        .unwrap();
        // 2026-11-04 是周三
        assert_eq!(s.last_due(at(2026, 11, 4, 10, 0)), at(2026, 11, 4, 9, 0));
        assert_eq!(s.last_due(at(2026, 11, 4, 8, 0)), at(2026, 11, 3, 9, 0));
        assert_eq!(s.next_due(at(2026, 11, 4, 8, 0)), at(2026, 11, 4, 9, 0));
        assert_eq!(s.next_due(at(2026, 11, 4, 10, 0)), at(2026, 11, 5, 9, 0));
    }

    #[test]
    fn weekly() {
        // 每周一 18:30
        let s = Schedule::compile(&ScheduleDef {
            every: Every::Weekly,
            at: "18:30".into(),
            weekday: Some(1),
        })
        .unwrap();
        // 周三：上一次是本周一，下一次是下周一
        assert_eq!(s.last_due(at(2026, 11, 4, 10, 0)), at(2026, 11, 2, 18, 30));
        assert_eq!(s.next_due(at(2026, 11, 4, 10, 0)), at(2026, 11, 9, 18, 30));
        // 周一 18:00（还没到点）：上一次是上周一
        assert_eq!(s.last_due(at(2026, 11, 2, 18, 0)), at(2026, 10, 26, 18, 30));
    }

    #[test]
    fn invalid() {
        let bad = |every, at: &str, weekday| {
            Schedule::compile(&ScheduleDef {
                every,
                at: at.into(),
                weekday,
            })
            .is_err()
        };
        assert!(bad(Every::Daily, "9点", None));
        assert!(bad(Every::Weekly, "09:00", None));
        assert!(bad(Every::Weekly, "09:00", Some(8)));
    }
}
