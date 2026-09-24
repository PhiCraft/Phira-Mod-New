//! Configuration module of the playing environment.\
//! e.g. player name, volume, speed, autoplay, etc.

use bitflags::bitflags;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

pub static TIPS: Lazy<Vec<String>> = Lazy::new(|| include_str!("tips.txt").split('\n').map(str::to_owned).collect());

bitflags! {
    #[derive(Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq, Debug)]
    #[serde(transparent)]
    pub struct Mods: i32 {
        const AUTOPLAY = 0x0001;
        const FLIP_X = 0x0002;
        const FADE_OUT = 0x0004;
        const FADE_IN = 0x0008;
        const NIGHTCORE = 0x0010;
        const RAINBOW = 0x0020;
        const NO_SHADER = 0x0040;
        const INSTANT_DEATH_AP = 0x0080;
        const INSTANT_DEATH_FC = 0x0100;
        /// 去连击分：分数不再计入最大连击，`score = accuracy * 1_000_000`。
        const NO_COMBO_SCORE = 0x0200;
        /// 全屏判定：整个屏幕都在判定范围内（横向不再限制），
        /// 并且距离对判定优先级的影响大幅降低。
        const FULL_SCREEN_JUDGE = 0x0400;
        /// 血条模式：Perfect / Good 回血，Bad / Miss 扣血，血量归零即失败。
        const HEALTH_MODE = 0x0800;

        const UNRATED = Self::AUTOPLAY.bits() | Self::NO_SHADER.bits() | Self::NO_COMBO_SCORE.bits() | Self::FULL_SCREEN_JUDGE.bits() | Self::HEALTH_MODE.bits();
    }
}

impl Mods {
    pub fn toggle_mod(&mut self, flag: Mods) {
        if self.contains(flag) {
            self.remove(flag);
        } else {
            for &conflict in Mods::conflicts(flag) {
                self.remove(conflict);
            }
            self.insert(flag);
        }
    }
    fn conflicts(flag: Mods) -> &'static [Mods] {
        match flag {
            Mods::FADE_IN => &[Mods::FADE_OUT],
            Mods::FADE_OUT => &[Mods::FADE_IN],
            Mods::INSTANT_DEATH_AP => &[Mods::INSTANT_DEATH_FC],
            Mods::INSTANT_DEATH_FC => &[Mods::INSTANT_DEATH_AP],
            _ => &[],
        }
    }
}

/// The color of the challenge mode badge shown in the ending scene.
#[derive(Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "camelCase")]
pub enum ChallengeColor {
    White,
    Green,
    Blue,
    Red,
    #[default]
    Golden,
    Rainbow,
}

impl ChallengeColor {
    pub const ALL: [ChallengeColor; 6] = [
        ChallengeColor::White,
        ChallengeColor::Green,
        ChallengeColor::Blue,
        ChallengeColor::Red,
        ChallengeColor::Golden,
        ChallengeColor::Rainbow,
    ];

    /// 下一个颜色（设置界面里点一下换一种）。
    pub fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|it| *it == self).unwrap_or(Self::ALL.len() - 2);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }
}

/// 血条模式里血量条的颜色。
#[derive(Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "camelCase")]
pub enum HealthBarColor {
    #[default]
    White,
    Green,
    Blue,
    Red,
    Gold,
    Purple,
}

impl HealthBarColor {
    pub const ALL: [HealthBarColor; 6] = [
        HealthBarColor::White,
        HealthBarColor::Green,
        HealthBarColor::Blue,
        HealthBarColor::Red,
        HealthBarColor::Gold,
        HealthBarColor::Purple,
    ];

    /// 下一个颜色（设置界面里点一下换一种）。
    pub fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|it| *it == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    /// RGB（0~1）；这里不直接依赖渲染层的类型。
    pub fn rgb(self) -> (f32, f32, f32) {
        match self {
            HealthBarColor::White => (0.95, 0.95, 0.95),
            HealthBarColor::Green => (0.42, 0.92, 0.55),
            HealthBarColor::Blue => (0.45, 0.75, 1.0),
            HealthBarColor::Red => (1.0, 0.42, 0.42),
            HealthBarColor::Gold => (1.0, 0.85, 0.4),
            HealthBarColor::Purple => (0.78, 0.55, 1.0),
        }
    }
}

/// 判定文字（PERFECT / GOOD / BAD / MISS）的出现动画样式。
#[derive(Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "camelCase")]
pub enum JudgeTextStyle {
    /// 原地淡出。
    Fade,
    /// 向上飘并淡出。
    Rise,
    /// 由小放大弹出后淡出。
    Pop,
    /// 上浮 + 弹出（默认）。
    #[default]
    RisePop,
}

impl JudgeTextStyle {
    pub const ALL: [JudgeTextStyle; 4] = [JudgeTextStyle::Fade, JudgeTextStyle::Rise, JudgeTextStyle::Pop, JudgeTextStyle::RisePop];

    pub fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|it| *it == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    /// 上浮距离 / 弹出幅度（0 表示该效果不参与）。
    pub fn rise_ratio(self) -> f32 {
        match self {
            JudgeTextStyle::Fade => 0.,
            JudgeTextStyle::Rise => 1.,
            JudgeTextStyle::Pop => 0.,
            JudgeTextStyle::RisePop => 0.6,
        }
    }

    pub fn pop_ratio(self) -> f32 {
        match self {
            JudgeTextStyle::Fade => 0.,
            JudgeTextStyle::Rise => 0.,
            JudgeTextStyle::Pop => 1.,
            JudgeTextStyle::RisePop => 1.,
        }
    }
}

/// 触点标记（touch_debug / 暂停调偏移时显示的手指点）用什么画。
#[derive(Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "camelCase")]
pub enum TouchMarkerStyle {
    /// 实心圆点
    #[default]
    Circle,
    /// 命中特效贴图（资源包里的 hit_fx）
    HitFx,
    /// 玩家头像
    Avatar,
}

impl TouchMarkerStyle {
    pub const ALL: [TouchMarkerStyle; 3] = [TouchMarkerStyle::Circle, TouchMarkerStyle::HitFx, TouchMarkerStyle::Avatar];

    pub fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|it| *it == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }
}

/// 触点标记的颜色。
#[derive(Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "camelCase")]
pub enum TouchMarkerColor {
    White,
    #[default]
    Red,
    Green,
    Blue,
    Gold,
    Purple,
}

impl TouchMarkerColor {
    pub const ALL: [TouchMarkerColor; 6] = [
        TouchMarkerColor::White,
        TouchMarkerColor::Red,
        TouchMarkerColor::Green,
        TouchMarkerColor::Blue,
        TouchMarkerColor::Gold,
        TouchMarkerColor::Purple,
    ];

    pub fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|it| *it == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    /// RGB（0~1）。
    pub fn rgb(self) -> (f32, f32, f32) {
        match self {
            TouchMarkerColor::White => (1.0, 1.0, 1.0),
            TouchMarkerColor::Red => (1.0, 0.35, 0.35),
            TouchMarkerColor::Green => (0.42, 0.95, 0.55),
            TouchMarkerColor::Blue => (0.45, 0.75, 1.0),
            TouchMarkerColor::Gold => (1.0, 0.85, 0.4),
            TouchMarkerColor::Purple => (0.8, 0.55, 1.0),
        }
    }
}
/// 实时 HUD 的九宫格位置。
#[derive(Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "camelCase")]
pub enum HudCorner {
    TopLeft,
    TopCenter,
    TopRight,
    #[default]
    LeftCenter,
    Center,
    RightCenter,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl HudCorner {
    pub const ALL: [HudCorner; 9] = [
        HudCorner::TopLeft,
        HudCorner::TopCenter,
        HudCorner::TopRight,
        HudCorner::LeftCenter,
        HudCorner::Center,
        HudCorner::RightCenter,
        HudCorner::BottomLeft,
        HudCorner::BottomCenter,
        HudCorner::BottomRight,
    ];

    pub fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|it| *it == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    /// 返回 (锚点 x, 锚点 y)，取值 -1 / 0 / 1；文字块以此为基准对齐。
    pub fn anchor(self) -> (f32, f32) {
        match self {
            HudCorner::TopLeft => (-1., 1.),
            HudCorner::TopCenter => (0., 1.),
            HudCorner::TopRight => (1., 1.),
            HudCorner::LeftCenter => (-1., 0.),
            HudCorner::Center => (0., 0.),
            HudCorner::RightCenter => (1., 0.),
            HudCorner::BottomLeft => (-1., -1.),
            HudCorner::BottomCenter => (0., -1.),
            HudCorner::BottomRight => (1., -1.),
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(default)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[serde(rename = "adjust_time_new")]
    pub adjust_time: bool,
    pub aggressive: bool,
    pub ap_fc_indicator: bool,
    pub aspect_ratio: Option<f32>,
    pub audio_buffer_size: Option<u32>,
    pub challenge_color: ChallengeColor,
    pub challenge_rank: u32,
    /// 游戏内连击数下面那行文字。
    /// 默认值和参考实现一致（Phi-Recorder / phire 的 `config.combo` = "RECALL"）；
    /// 留空才会回退到 AUTOPLAY / COMBO。
    pub combo_text: String,
    pub chart_debug: bool,
    /// 判定线调试：把本该隐藏/淡出的判定线以淡影保留，并在线旁显示编号 / 线高 / z / 类型。
    pub chart_debug_line: bool,
    /// 音符调试：把本该隐藏/淡出的音符以淡影保留，并在音符旁显示线号 / 时间 / 高度，
    /// 同时画出该音符的横向判定范围。
    pub chart_debug_note: bool,
    pub disable_effect: bool,
    pub double_click_to_pause: bool,
    pub double_hint: bool,
    /// 黄键保护：点击（蓝键）不会被叠在附近的 Drag（黄键）吃掉。
    pub drag_protect: bool,
    /// 红键保护：点击（蓝键）不会被叠在附近的 Flick（红键）吃掉。
    pub flick_protect: bool,
    pub fullscreen_mode: bool,
    pub fxaa: bool,
    pub interactive: bool,
    /// Half of the `Perfect` judgement window, in milliseconds. `Good` and `Bad`
    /// windows default to 2× and 2.75× of it, but each of them can be adjusted
    /// separately (see [`Config::judge_window_good`] and
    /// [`Config::judge_window_bad`]).
    pub judge_window: f32,
    /// `Good` 判定窗口（正负毫秒），可单独调整；`None` 表示跟随 Perfect 的 2 倍。
    pub judge_window_good: Option<f32>,
    /// `Bad` 判定窗口（正负毫秒），可单独调整；`None` 表示跟随 Perfect 的 2.75 倍。
    pub judge_window_bad: Option<f32>,
    /// 判定线以 Y 轴为参考（线长与厚度按屏幕纵横比换算），与参考实现一致。
    pub line_ref_y_axis: bool,
    pub mods: Mods,
    pub mp_address: String,
    pub mp_enabled: bool,
    pub note_scale: f32,
    /// 结算画面是否画判定时间分布图（Early / Late 直方图）。
    pub ending_judge_chart: bool,
    /// 游戏内是否实时提示「太早 / 太晚」。
    pub early_late_hint: bool,
    /// 暂停时是否显示判定偏移微调面板（±1ms / ±5ms）。
    pub pause_offset_adjust: bool,
    /// 血条模式的血量上限。
    pub health_max: f32,
    /// 血条模式的回血 / 扣血倍率。
    pub health_scale: f32,
    /// 血量条的颜色。
    pub health_bar_color: HealthBarColor,
    /// 血量条的长度（占可用高度的比例）。
    pub health_bar_len: f32,
    /// 摇一摇再玩：设备静止一段时间自动暂停，摇动超过阈值自动继续。
    pub motion_pause: bool,
    /// 摇动的幅度阈值（单位 g，线性加速度；越小越灵敏）。
    pub motion_threshold: f32,
    /// 静止多久算「静止」（秒）。
    pub motion_still_time: f32,

    // ---- 实时 HUD ----
    /// 实时 HUD 总开关（下面的子项只在打开它之后生效）。
    pub hud: bool,
    /// HUD：实时准确率。
    pub hud_acc: bool,
    /// HUD：各判定计数。
    pub hud_counts: bool,
    /// HUD：最大连击。
    pub hud_max_combo: bool,
    /// HUD：当前时间 / 总时长 / 进度条。
    pub hud_time: bool,
    /// HUD：最近一次判定的偏差（±xx ms）。
    pub hud_delta: bool,
    /// HUD：预估 RKS（本地近似值，见 [`Config::estimate_rks`]）。
    pub hud_rks: bool,
    /// HUD：当前 / 平均 FPS。
    pub hud_fps: bool,
    /// HUD 的九宫格位置。
    pub hud_corner: HudCorner,
    /// HUD 字号倍率。
    pub hud_size: f32,
    /// HUD 不透明度。
    pub hud_alpha: f32,
    /// HUD 是否加描边（黑描边 + 亮色字，背景亮时也看得清）。
    pub hud_outline: bool,

    /// **晚按补偿**（毫秒）：判定时把「按晚了」的误差减掉这么多。
    ///
    /// 上游 prpr 里这是写死的 70ms，而且只作用在晚按一侧，于是实际窗口变成
    /// Perfect ≤150 / Good ≤230 / Bad ≤290，早按一侧还是 80/160/220 —— 也就是
    /// 「late 判定太松」。默认 0 = 完全对称、严格按三个窗口判；
    /// 如果设备有音频输出延迟导致你总是偏晚，可以调大一点补回来。
    pub late_leniency_ms: f32,

    // ---- 打歌界面字体 ----
    /// 打歌界面的文字改用 Phi-Recorder 渲染视频时用的那套字体。
    ///
    /// 默认关 = 原版 phira 行为：分数 / 连击 / 准度用 `assets/phigros.ttf`，
    /// 曲名 / 难度用界面字体。打开后整块打歌文字（分数、连击、准度、曲名、难度、
    /// 血条数字、HUD、判定文字）统一走 `assets/font.ttf`
    /// （Source Han Sans + Saira + Noto），和 Phi-Recorder 导出的视频一致。
    pub phi_recorder_font: bool,

    // ---- 打击表现 ----
    /// 判定文字（PERFECT / GOOD / BAD / MISS）。
    pub judge_text: bool,
    /// 判定文字的出现动画样式。
    pub judge_text_style: JudgeTextStyle,
    /// 判定文字字号。
    pub judge_text_size: f32,
    /// 漏键标记：Bad / Miss 的位置在判定线上留痕。
    pub miss_marker: bool,
    /// 漏键标记停留时间（秒）。
    pub miss_marker_time: f32,
    /// 音符拖影。
    pub note_trail: bool,
    /// 音符拖影的残影数量（每个音符，1~255）。
    pub note_trail_count: u32,
    /// 音符拖影长度（0~1，越大越长）。
    pub note_trail_len: f32,
    /// 音符拖影不透明度（0~1）。
    pub note_trail_alpha: f32,
    /// 拖影对蓝键（Click）生效。
    pub note_trail_click: bool,
    /// 拖影对黄键（Drag）生效。
    pub note_trail_drag: bool,
    /// 拖影对红键（Flick）生效。
    pub note_trail_flick: bool,
    /// 判定线残影。
    pub line_afterimage: bool,
    /// 判定线残影条数（1~255）。
    pub line_afterimage_count: u32,
    /// 判定线残影不透明度（0~1）。
    pub line_afterimage_alpha: f32,
    /// 判定线发光。
    pub line_glow: bool,
    /// 判定线发光强度（0~1）。
    pub line_glow_strength: f32,

    /// 触点标记的样式（圆点 / 命中特效贴图 / 头像）。
    pub touch_marker_style: TouchMarkerStyle,
    /// 触点标记的颜色。
    pub touch_marker_color: TouchMarkerColor,
    /// 触点标记的不透明度（0.05~1）。
    pub touch_marker_alpha: f32,

    /// 音乐可视化：在判定线上画频谱条（对音频做 FFT）。
    pub music_spectrum: bool,
    /// 频谱条的放大系数（0.2~4）。
    pub music_spectrum_gain: f32,

    /// **上传成绩**（默认关）：打完官方谱面后是否把本局成绩上传到 Phira 官方服务器。
    ///
    /// 关掉时成绩只存在本机（本地「成绩历史」照常记录），不上排行榜、不更新云端 RKS。
    /// 打开前必须先阅读并同意成绩上传协议（见 `upload_agreed`）。
    pub upload_record: bool,
    /// 是否已经读过并同意「成绩上传知情同意与免责声明」。
    pub upload_agreed: bool,

    pub offline_mode: bool,
    pub offset: f32,
    pub particle: bool,
    pub player_name: String,
    pub player_rks: f32,
    pub preferred_sample_rate: Option<u32>,
    pub res_pack_path: Option<String>,
    /// 结算画面显示的 rks；`None` 表示跟随账号真实 rks。
    pub rks_override: Option<f32>,
    pub sample_count: u32,
    pub show_acc: bool,
    pub show_avg_fps: bool,
    pub speed: f32,
    pub touch_debug: bool,
    pub use_keyboard: bool,
    pub volume_bgm: f32,
    pub volume_music: f32,
    pub volume_sfx: f32,

    // for compatibility
    autoplay: Option<bool>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            adjust_time: false,
            aggressive: true,
            ap_fc_indicator: true,
            aspect_ratio: None,
            audio_buffer_size: None,
            challenge_color: ChallengeColor::Golden,
            challenge_rank: 45,
            combo_text: "RECALL".to_owned(),
            chart_debug: false,
            chart_debug_line: false,
            chart_debug_note: false,
            disable_effect: false,
            double_click_to_pause: true,
            double_hint: true,
            drag_protect: false,
            flick_protect: false,
            fxaa: false,
            interactive: true,
            judge_window: Self::DEFAULT_JUDGE_WINDOW,
            judge_window_good: None,
            judge_window_bad: None,
            line_ref_y_axis: false,
            mods: Mods::default(),
            mp_address: "mp2.phira.cn:12345".to_owned(),
            mp_enabled: false,
            note_scale: 1.0,
            ending_judge_chart: false,
            early_late_hint: false,
            pause_offset_adjust: false,
            health_max: 100.,
            health_scale: 1.,
            health_bar_color: HealthBarColor::White,
            health_bar_len: 0.6,
            motion_pause: false,
            motion_threshold: 0.15,
            motion_still_time: 1.2,

            hud: false,
            hud_acc: false,
            hud_counts: false,
            hud_max_combo: false,
            hud_time: false,
            hud_delta: false,
            hud_rks: false,
            hud_fps: false,
            hud_corner: HudCorner::LeftCenter,
            hud_size: 1.,
            hud_alpha: 0.85,
            hud_outline: true,

            late_leniency_ms: 0.,

            phi_recorder_font: false,

            judge_text: false,
            judge_text_style: JudgeTextStyle::RisePop,
            judge_text_size: 0.62,
            miss_marker: false,
            miss_marker_time: 1.5,
            note_trail: false,
            note_trail_count: 8,
            note_trail_len: 0.5,
            note_trail_alpha: 0.35,
            note_trail_click: false,
            note_trail_drag: false,
            note_trail_flick: false,
            line_afterimage: false,
            line_afterimage_count: 4,
            line_afterimage_alpha: 0.35,
            line_glow: false,
            line_glow_strength: 0.4,

            touch_marker_style: TouchMarkerStyle::Circle,
            touch_marker_color: TouchMarkerColor::Red,
            touch_marker_alpha: 0.4,

            music_spectrum: false,
            music_spectrum_gain: 1.,

            upload_record: false,
            upload_agreed: false,

            offline_mode: false,
            fullscreen_mode: false,
            offset: 0.,
            particle: true,
            player_name: "Mivik".to_string(),
            player_rks: 15.,
            preferred_sample_rate: None,
            res_pack_path: None,
            rks_override: None,
            sample_count: 1,
            show_acc: false,
            show_avg_fps: false,
            speed: 1.,
            touch_debug: false,
            use_keyboard: false,
            volume_music: 1.,
            volume_sfx: 1.,
            volume_bgm: 1.,

            autoplay: None,
        }
    }
}

impl Config {
    /// The default half-width of the `Perfect` judgement window, in milliseconds.
    pub const DEFAULT_JUDGE_WINDOW: f32 = 80.;
    /// The narrowest judge window that can be configured, in milliseconds.
    pub const JUDGE_WINDOW_MIN: f32 = 20.;
    /// The widest judge window that can be configured, in milliseconds.
    pub const JUDGE_WINDOW_MAX: f32 = 200.;
    /// `Good` 判定窗口的上限（毫秒）。
    pub const GOOD_WINDOW_MAX: f32 = 400.;
    /// `Bad` 判定窗口的上限（毫秒）。
    pub const BAD_WINDOW_MAX: f32 = 600.;

    pub fn init(&mut self) {
        if let Some(flag) = self.autoplay {
            self.mods.set(Mods::AUTOPLAY, flag);
        }
        #[cfg(target_env = "ohos")]
        {
            // Due to the fucking poor performance of the Maloon GPU, the sample count must be set to 1.
            self.sample_count = 1;
        }
    }

    #[inline]
    pub fn has_mod(&self, m: Mods) -> bool {
        self.mods.contains(m)
    }

    #[inline]
    pub fn autoplay(&self) -> bool {
        self.has_mod(Mods::AUTOPLAY)
    }

    #[inline]
    pub fn flip_x(&self) -> bool {
        self.has_mod(Mods::FLIP_X)
    }

    /// 全屏判定：任意位置的点按/滑动都能判定到音符（与参考的 `full_scrrn_judge` 一致）。
    #[inline]
    pub fn full_screen_judge(&self) -> bool {
        self.has_mod(Mods::FULL_SCREEN_JUDGE)
    }

    /// 血条模式。
    #[inline]
    pub fn health_mode(&self) -> bool {
        self.has_mod(Mods::HEALTH_MODE)
    }

    /// 连击数下面那行显示的文案。
    /// 参考实现（phire）是「Autoplay 时显示 AUTOPLAY，否则显示自定义文案」，
    /// 所以这里也把 Autoplay 放在最前面判断，而不是只在文案为空时才显示 AUTOPLAY。
    #[inline]
    pub fn combo_label(&self) -> &str {
        if self.autoplay() {
            "AUTOPLAY"
        } else if self.combo_text.is_empty() {
            "COMBO"
        } else {
            &self.combo_text
        }
    }

    /// 自定义连击文案的长度上限（太长会画出屏幕）。
    pub const COMBO_TEXT_MAX_CHARS: usize = 16;

    /// The judge window, in milliseconds, clamped into the configurable range.
    #[inline]
    fn judge_window_ms(&self) -> f32 {
        if self.judge_window.is_finite() {
            self.judge_window.clamp(Self::JUDGE_WINDOW_MIN, Self::JUDGE_WINDOW_MAX)
        } else {
            Self::DEFAULT_JUDGE_WINDOW
        }
    }

    /// Half-width of the `Perfect` judgement window, in seconds (chart time).
    #[inline]
    pub fn limit_perfect(&self) -> f64 {
        self.judge_window_ms() as f64 / 1000.
    }

    /// 血量条长度限制在可配置范围内（占可用高度的比例）。
    #[inline]
    pub fn health_bar_len_ratio(&self) -> f32 {
        if self.health_bar_len.is_finite() {
            self.health_bar_len.clamp(Self::HEALTH_BAR_LEN_MIN, Self::HEALTH_BAR_LEN_MAX)
        } else {
            Self::HEALTH_BAR_LEN_MAX
        }
    }

    /// 血量条长度上限。
    pub const HEALTH_BAR_LEN_MAX: f32 = 1.;
    /// 血量条长度下限。
    pub const HEALTH_BAR_LEN_MIN: f32 = 0.2;

    /// 摇动幅度阈值（g），限制在可配置范围内。
    #[inline]
    pub fn motion_threshold_g(&self) -> f32 {
        if self.motion_threshold.is_finite() {
            self.motion_threshold.clamp(Self::MOTION_THRESHOLD_MIN, Self::MOTION_THRESHOLD_MAX)
        } else {
            0.15
        }
    }

    /// 静止判定时间（秒），限制在可配置范围内。
    #[inline]
    pub fn motion_still_secs(&self) -> f32 {
        if self.motion_still_time.is_finite() {
            self.motion_still_time.clamp(Self::MOTION_STILL_MIN, Self::MOTION_STILL_MAX)
        } else {
            1.2
        }
    }

    /// 摇动幅度阈值范围（g）。
    pub const MOTION_THRESHOLD_MIN: f32 = 0.02;
    pub const MOTION_THRESHOLD_MAX: f32 = 1.;
    /// 静止判定时间范围（秒）。
    pub const MOTION_STILL_MIN: f32 = 0.3;
    pub const MOTION_STILL_MAX: f32 = 10.;

    /// 横向判定范围（世界坐标，屏幕横向为 ±1）：
    /// 默认 `0.21 / (16/9) * 2`，开了全屏判定就是整个屏幕宽。
    #[inline]
    pub fn x_diff_max(&self) -> f64 {
        if self.full_screen_judge() {
            2.
        } else {
            0.21 / (16. / 9.) * 2.
        }
    }

    /// `Good` 判定窗口，毫秒，限制在可配置范围内；`None` 时跟随 Perfect 的 2 倍。
    #[inline]
    pub fn judge_window_good_ms(&self) -> f32 {
        match self.judge_window_good {
            Some(v) if v.is_finite() => v.clamp(Self::JUDGE_WINDOW_MIN, Self::GOOD_WINDOW_MAX),
            _ => self.judge_window_ms() * 2.,
        }
    }

    /// `Bad` 判定窗口，毫秒，限制在可配置范围内；`None` 时跟随 Perfect 的 2.75 倍。
    #[inline]
    pub fn judge_window_bad_ms(&self) -> f32 {
        match self.judge_window_bad {
            Some(v) if v.is_finite() => v.clamp(Self::JUDGE_WINDOW_MIN, Self::BAD_WINDOW_MAX),
            _ => self.judge_window_ms() * 2.75,
        }
    }

    /// Half-width of the `Good` judgement window, in seconds (chart time).
    /// 不会小于 Perfect 窗口。
    #[inline]
    pub fn limit_good(&self) -> f64 {
        (self.judge_window_good_ms().max(self.judge_window_ms()) as f64) / 1000.
    }

    /// Half-width of the `Bad` judgement window, in seconds (chart time).
    /// 不会小于 Good / Perfect 窗口。
    #[inline]
    pub fn limit_bad(&self) -> f64 {
        (self.judge_window_bad_ms().max(self.judge_window_good_ms()).max(self.judge_window_ms()) as f64) / 1000.
    }

    /// 晚按补偿（秒），限制在可配置范围内；默认 0 = 早按晚按完全对称。
    #[inline]
    pub fn late_leniency(&self) -> f64 {
        const MAX_MS: f32 = 200.;
        if self.late_leniency_ms.is_finite() {
            (self.late_leniency_ms.clamp(0., MAX_MS) as f64) / 1000.
        } else {
            0.
        }
    }

    /// 晚按补偿的上限（毫秒）。
    pub const LATE_LENIENCY_MAX: f32 = 200.;

    // ---------------- 实时 HUD / 打击表现：把各滑块收进合理范围 ----------------

    fn finite_or(v: f32, def: f32) -> f32 {
        if v.is_finite() {
            v
        } else {
            def
        }
    }

    /// HUD 字号倍率（0.5~2.0）。
    #[inline]
    pub fn hud_size_scale(&self) -> f32 {
        Self::finite_or(self.hud_size, 1.).clamp(0.5, 2.)
    }

    /// HUD 不透明度（0.1~1.0）。
    #[inline]
    pub fn hud_alpha_value(&self) -> f32 {
        Self::finite_or(self.hud_alpha, 0.85).clamp(0.1, 1.)
    }

    /// 判定文字字号（0.2~1.5）。
    #[inline]
    pub fn judge_text_size_value(&self) -> f32 {
        Self::finite_or(self.judge_text_size, 0.62).clamp(0.2, 1.5)
    }

    /// 漏键标记停留时间，秒（0.2~10）。
    #[inline]
    pub fn miss_marker_secs(&self) -> f32 {
        Self::finite_or(self.miss_marker_time, 1.5).clamp(0.2, 10.)
    }

    /// 音符拖影长度（0~1）。
    #[inline]
    pub fn note_trail_len_ratio(&self) -> f32 {
        Self::finite_or(self.note_trail_len, 0.5).clamp(0., 1.)
    }

    /// 音符拖影不透明度（0~1）。
    #[inline]
    pub fn note_trail_alpha_value(&self) -> f32 {
        Self::finite_or(self.note_trail_alpha, 0.35).clamp(0., 1.)
    }

    /// 判定线残影条数（1~255）。
    #[inline]
    pub fn line_afterimage_count_value(&self) -> u32 {
        self.line_afterimage_count.clamp(1, Self::AFTERIMAGE_COUNT_MAX)
    }

    /// 音符拖影的残影数量（每个音符，1~255）。
    #[inline]
    pub fn note_trail_count_value(&self) -> u32 {
        self.note_trail_count.clamp(1, Self::AFTERIMAGE_COUNT_MAX)
    }

    /// 残影数量上限（每个键 / 每条判定线）。
    pub const AFTERIMAGE_COUNT_MAX: u32 = 255;

    /// 触点标记不透明度（0.05~1）。
    #[inline]
    pub fn touch_marker_alpha_value(&self) -> f32 {
        Self::finite_or(self.touch_marker_alpha, 0.4).clamp(0.05, 1.)
    }

    /// 频谱条放大系数（0.2~4）。
    #[inline]
    pub fn music_spectrum_gain_value(&self) -> f32 {
        Self::finite_or(self.music_spectrum_gain, 1.).clamp(0.2, 4.)
    }

    /// 判定线残影不透明度（0~1）。
    #[inline]
    pub fn line_afterimage_alpha_value(&self) -> f32 {
        Self::finite_or(self.line_afterimage_alpha, 0.35).clamp(0., 1.)
    }

    /// 判定线发光强度（0~1）。
    #[inline]
    pub fn line_glow_strength_value(&self) -> f32 {
        Self::finite_or(self.line_glow_strength, 0.4).clamp(0., 1.)
    }

    /// 预估 RKS（本地近似值）。
    ///
    /// 服务端的 RKS 是按「最好的一批成绩」算的，本地拿不到那批数据，所以这里用一个
    /// 单调的近似：`本曲难度 × 当前分数占比`。打满就是本曲难度值本身，用来在打歌时
    /// 大致判断「这把大概能给自己加多少」。设置界面里也标了「近似」。
    #[inline]
    pub fn estimate_rks(&self, difficulty: f32, score: u32) -> f32 {
        (difficulty.max(0.) * (score as f32 / 1_000_000.).clamp(0., 1.)).max(0.)
    }
}
