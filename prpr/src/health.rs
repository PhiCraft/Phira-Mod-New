//! 血条模式。
//!
//! 参考 phire 的 `health.rs`，这里按「判定加减血 + 连击回血」的经典模式简化实现：
//! Perfect / Good 回血，Bad / Miss 扣血，每满一定连击额外回血，血量掉到 0 即失败。

use crate::{config::Config, judge::Judgement};

/// 每多少连击回一次血。
pub const COMBO_HEAL_INTERVAL: u32 = 10;
/// 连击回血的量（再乘倍率）。
pub const COMBO_HEAL_AMOUNT: f32 = 2.;
/// 开局血量占上限的比例。
pub const INITIAL_RATIO: f32 = 0.7;

#[derive(Clone, Copy, Debug)]
pub struct Health {
    /// 当前血量
    pub hp: f32,
    /// 血量上限
    pub max: f32,
    /// 回血 / 扣血倍率
    pub scale: f32,
    /// 上一次触发连击回血时的连击数
    last_combo_heal: u32,
    /// 血量是否已经掉到 0
    pub failed: bool,
}

impl Health {
    pub fn new(config: &Config) -> Self {
        let max = if config.health_max.is_finite() { config.health_max.clamp(10., 10000.) } else { 100. };
        let scale = if config.health_scale.is_finite() { config.health_scale.clamp(0.1, 10.) } else { 1. };
        let mut it = Self {
            hp: max,
            max,
            scale,
            last_combo_heal: 0,
            failed: false,
        };
        it.reset();
        it
    }

    pub fn reset(&mut self) {
        self.hp = self.max * INITIAL_RATIO;
        self.last_combo_heal = 0;
        self.failed = false;
    }

    /// 当前血量比例（0 ~ 1），画血条用。
    pub fn ratio(&self) -> f32 {
        if self.max <= 0. {
            0.
        } else {
            (self.hp / self.max).clamp(0., 1.)
        }
    }

    /// 每次判定调用一次，返回这次判定后是否失败（血量归零）。
    pub fn on_judge(&mut self, what: Judgement, combo: u32) -> bool {
        if self.failed {
            return true;
        }
        let delta = match what {
            Judgement::Perfect => 1.,
            Judgement::Good => 0.5,
            Judgement::Bad => {
                // 断连后回到当前里程碑，重新连满 10 连击还能再回血
                self.last_combo_heal = combo / COMBO_HEAL_INTERVAL * COMBO_HEAL_INTERVAL;
                -5.
            }
            Judgement::Miss => {
                self.last_combo_heal = 0;
                -10.
            }
        } * self.scale;
        self.hp += delta;
        // 连击回血：每满 COMBO_HEAL_INTERVAL 连击回一次
        if combo >= COMBO_HEAL_INTERVAL && combo / COMBO_HEAL_INTERVAL > self.last_combo_heal / COMBO_HEAL_INTERVAL {
            self.hp += COMBO_HEAL_AMOUNT * self.scale;
            self.last_combo_heal = combo;
        }
        self.hp = self.hp.clamp(0., self.max);
        if self.hp <= 0. {
            self.failed = true;
        }
        self.failed
    }
}
