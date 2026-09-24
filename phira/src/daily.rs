//! 每日挑战：用日期当种子，每天固定一首谱面 + 一个约束，本地记录完成情况。
//!
//! 种子必须**稳定**：不能用 `rand` 的默认熵（同一个种子，随机数发生器的初始化是确定的，
//! 但用系统熵就会每次重启都换题），所以这里用「Unix 天数」直接做哈希。
//!
//! 约束通过 `PENDING_OVERRIDE` 传给 `SongScene`：开始挑战时放一份改好的 `Config`，
//! song.rs 读一次就清掉 —— 这样挑战参数不会写进你保存的配置里。

use anyhow::Result;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

use crate::{dir, get_data};

/// 每日挑战的约束类型。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Constraint {
    /// 谱面速度 1.1x
    SpeedUp,
    /// 判定窗口收窄到 ±60ms
    StrictJudge,
    /// 血条模式
    HealthMode,
    /// 关掉所有提示（太早太晚 / HUD / 判定文字）
    NoHints,
}

impl Constraint {
    pub const ALL: [Constraint; 4] = [Constraint::SpeedUp, Constraint::StrictJudge, Constraint::HealthMode, Constraint::NoHints];

    /// 把这套约束套到一份配置上（不写回保存的配置）。
    pub fn apply(&self, config: &mut prpr::config::Config) {
        match self {
            Constraint::SpeedUp => config.speed = 1.1,
            Constraint::StrictJudge => {
                config.judge_window = 60.;
                config.judge_window_good = None;
                config.judge_window_bad = None;
            }
            Constraint::HealthMode => config.mods.insert(prpr::config::Mods::HEALTH_MODE),
            Constraint::NoHints => {
                config.early_late_hint = false;
                config.hud = false;
                config.judge_text = false;
            }
        }
    }
}

/// 今天的挑战。
#[derive(Clone, Debug)]
pub struct Challenge {
    /// Unix 天数（种子 / 存储键都靠它）。
    pub day: i64,
    /// 谱面在本地列表里的下标。
    pub chart_index: usize,
    pub constraint: Constraint,
}

/// 一天的完成情况。
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DayRecord {
    pub done: bool,
    pub score: u32,
    pub accuracy: f64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct Store {
    days: HashMap<String, DayRecord>,
}

static STORE: Lazy<Mutex<Option<Store>>> = Lazy::new(|| Mutex::new(None));
/// 正在进行的挑战是哪一天（用于结算时标记完成）。
static ACTIVE: Lazy<Mutex<Option<i64>>> = Lazy::new(|| Mutex::new(None));
/// 待应用的挑战配置覆盖。
static PENDING_OVERRIDE: Lazy<Mutex<Option<prpr::config::Config>>> = Lazy::new(|| Mutex::new(None));

fn path() -> Result<String> {
    Ok(format!("{}/daily.json", dir::root()?))
}

fn load_inner() -> Store {
    (|| -> Result<Store> {
        let path = path()?;
        if !std::path::Path::new(&path).exists() {
            return Ok(Store::default());
        }
        let text = std::fs::read_to_string(&path)?;
        if text.trim().is_empty() {
            return Ok(Store::default());
        }
        Ok(serde_json::from_str(&text)?)
    })()
    .unwrap_or_default()
}

fn with_store<T>(f: impl FnOnce(&mut Store) -> T) -> T {
    let mut guard = STORE.lock().unwrap();
    let store = guard.get_or_insert_with(load_inner);
    f(store)
}

fn save(store: &Store) -> Result<()> {
    std::fs::write(path()?, serde_json::to_string(store)?)?;
    Ok(())
}

/// 今天（本地时区）的 Unix 天数。
pub fn today() -> i64 {
    chrono::Local::now().timestamp().div_euclid(86_400)
}

/// 由天数派生的稳定哈希（SplitMix64，够用且不引入依赖）。
fn hash(day: i64, salt: u64) -> u64 {
    let mut z = (day as u64)
        .wrapping_add(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(salt.wrapping_mul(0xBF58_476D_1CE4_E5B9));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// 生成某一天的挑战（谱面数量为 0 时返回 None）。
pub fn challenge_for(day: i64, chart_count: usize) -> Option<Challenge> {
    if chart_count == 0 {
        return None;
    }
    Some(Challenge {
        day,
        chart_index: (hash(day, 1) % chart_count as u64) as usize,
        constraint: Constraint::ALL[(hash(day, 2) % Constraint::ALL.len() as u64) as usize],
    })
}

/// 今天的挑战。
pub fn today_challenge(chart_count: usize) -> Option<Challenge> {
    challenge_for(today(), chart_count)
}

/// 某一天的完成情况。
pub fn record_of(day: i64) -> DayRecord {
    with_store(|it| it.days.get(&day.to_string()).cloned().unwrap_or_default())
}

/// 开始挑战：暂存一份改好的配置，并记住「正在进行的是哪一天」。
pub fn start(challenge: &Challenge) {
    let mut config = get_data().config.clone();
    challenge.constraint.apply(&mut config);
    *PENDING_OVERRIDE.lock().unwrap() = Some(config);
    *ACTIVE.lock().unwrap() = Some(challenge.day);
}

/// `song.rs` 取走挑战用的配置覆盖（只取一次）。
pub fn take_pending_override() -> Option<prpr::config::Config> {
    PENDING_OVERRIDE.lock().unwrap().take()
}

/// 结算时调用：如果这一局是每日挑战，记下完成情况。
pub fn on_finished(score: u32, accuracy: f64) {
    let Some(day) = ACTIVE.lock().unwrap().take() else {
        return;
    };
    with_store(|store| {
        let rec = store.days.entry(day.to_string()).or_default();
        rec.done = true;
        rec.score = rec.score.max(score);
        rec.accuracy = rec.accuracy.max(accuracy);
        let _ = save(store);
    });
}
