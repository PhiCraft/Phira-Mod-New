#![allow(unused)]

prpr_l10n::tl_file!("game");

use super::{
    draw_background,
    ending::RecordUpdateState,
    loading::{BasicPlayer, SaveFn, UpdateFn, UploadFn},
    request_input, return_input, show_message, take_input, EndingScene, NextScene, Scene,
};
use crate::{
    bin::BinaryReader,
    config::{Config, Mods},
    core::{copy_fbo, BadNote, Chart, ChartExtra, Effect, JudgeLineKind, NoteKind, Point, Resource, UIElement, Vector, GAME_FONT, PGR_FONT},
    ext::{draw_text_aligned_opt_width_using, draw_text_aligned_using, parse_time, screen_aspect, semi_white, RectExt, SafeTexture, ScaleType},
    fs::FileSystem,
    info::{ChartFormat, ChartInfo},
    judge::{Judge, Judgement},
    parse::{parse_extra, parse_pec, parse_phigros, parse_rpe},
    task::Task,
    time::TimeManager,
    ui::{RectButton, TextPainter, Ui},
};
use anyhow::{bail, Context, Result};
use concat_string::concat_string;
use inputbox::InputBox;
use lyon::path::Path;
use macroquad::{prelude::*, window::InternalGlContext};
use sasa::{Music, MusicParams};
use serde::{Deserialize, Serialize};
use std::{
    any::Any,
    cell::{Cell, RefCell},
    fs::File,
    io::{Cursor, ErrorKind},
    ops::{Deref, DerefMut, Range},
    path::PathBuf,
    process::{Command, Stdio},
    rc::Rc,
    sync::{Arc, Mutex},
    thread::LocalKey,
    time::Duration,
};
use tracing::{debug, warn};

const PAUSE_CLICK_INTERVAL: f32 = 0.7;

/// 打歌界面里 `size()` 和实际字高的换算。
///
/// 文字渲染的缩放是 `0.04 * size * 视口宽(px)`，画到世界坐标里又被 `2 / 视口宽`
/// 除回来，所以一行字的**字高（世界坐标）≈ 0.08 × 字体 cap 高比例 × size**。
/// 打歌界面用的 `assets/phigros.ttf` cap 高是 0.688 em，于是：
///
/// * `size 1.0` → 字高 ≈ 0.055（世界坐标）；
/// * 16:9 时屏幕高 = `2 / 1.7778` = 1.125，所以 `size 1.0` ≈ **屏幕高的 4.9%**。
///
/// 对照 Phi-Recorder（`phire`）和原生 phira 的打歌界面：
/// COMBO 数字 1.0、分数 0.8、偏移面板标题 0.7 / 数值 0.6、曲名难度 0.5、
/// 按钮文字 0.42（`Ui::button`）、ACC 与连击文字 0.4。
/// 自己加的 UI 一律按这套尺度写，不要再凭感觉填「零点几」。
const IN_GAME_TEXT_H: f32 = 0.055;

/// 打歌界面文字用的字体。
///
/// 默认（`phi_recorder_font = false`）是原生 phira 的行为：打歌文字用
/// `assets/phigros.ttf`（[`PGR_FONT`]）。打开后切到 `assets/font.ttf`
/// （[`GAME_FONT`]，Source Han Sans + Saira + Noto），也就是 Phi-Recorder
/// 渲染视频时唯一的那套字体，分数 / 连击 / 准度的字形会和它一致。
fn text_font(config: &Config) -> &'static LocalKey<RefCell<Option<TextPainter>>> {
    if config.phi_recorder_font {
        &GAME_FONT
    } else {
        &PGR_FONT
    }
}

/// 曲名 / 难度这类原生用界面字体画的文字，开了「Phi-Recorder 字体」后一并统一。
fn name_font(config: &Config) -> Option<&'static LocalKey<RefCell<Option<TextPainter>>>> {
    config.phi_recorder_font.then_some(&GAME_FONT)
}

thread_local! {
    /// 暂停界面上调过的判定偏移（秒），客户端每帧取走并写回配置。
    static PENDING_OFFSET: Cell<Option<f32>> = const { Cell::new(None) };
    /// 暂停界面上调过的音量（音乐, 音效），客户端每帧取走并写回配置。
    static PENDING_VOLUME: Cell<Option<(f32, f32)>> = const { Cell::new(None) };
    /// 设备当前的摇动强度（线性加速度，单位 g）。平台层（Android 的 SensorManager /
    /// iOS 的 CoreMotion）负责写进来，引擎只关心数值；负数表示没有传感器。
    static MOTION_LEVEL: Cell<f32> = const { Cell::new(-1.) };
}

/// 平台层写入当前摇动强度（单位 g，≥ 0）。负数表示这台设备没有传感器（模拟器等），
/// 此时「摇一摇再玩」不会自动暂停/恢复。
pub fn set_motion_level(level: f32) {
    MOTION_LEVEL.with(|it| it.set(if level.is_finite() { level } else { -1. }));
}

/// 当前摇动强度（单位 g）；负数表示传感器不可用。
pub fn motion_level() -> f32 {
    MOTION_LEVEL.with(|it| it.get())
}

/// 取走暂停界面上调整过的判定偏移（秒）；没有调整过时返回 `None`。
pub fn take_pending_offset() -> Option<f32> {
    PENDING_OFFSET.with(|it| it.take())
}

/// 取走暂停界面上调整过的音量 `(音乐, 音效)`；没有调整过时返回 `None`。
pub fn take_pending_volume() -> Option<(f32, f32)> {
    PENDING_VOLUME.with(|it| it.take())
}

#[rustfmt::skip]
#[cfg(closed)]
mod inner;
#[cfg(closed)]
use inner::*;

const WAIT_TIME: f64 = 0.5;
const AFTER_TIME: f64 = 0.7;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SimpleRecord {
    pub score: i32,
    pub accuracy: f32,
    pub full_combo: bool,
    /// 下面这些是给「成绩历史」用的：服务端的最好成绩接口只返回上面三个，
    /// 所以它们必须能缺省（`#[serde(default)]`），缺省就是 0 / 空。
    pub counts: [u32; 4],
    pub max_combo: u32,
    pub num_of_notes: u32,
    /// 判定误差分布（早 ← → 晚），和结算图的 `hist` 同源
    pub hist: Vec<u32>,
}

impl SimpleRecord {
    pub fn update(&mut self, other: &SimpleRecord) -> bool {
        let mut changed = false;
        if other.score > self.score {
            self.score = other.score;
            changed = true;
        }
        if other.accuracy > self.accuracy {
            self.accuracy = other.accuracy;
            changed = true;
        }
        if other.full_combo & !self.full_combo {
            self.full_combo = other.full_combo;
            changed = true;
        }
        changed
    }
}

fn fmt_time(t: f32) -> String {
    let f = t < 0.;
    let t = t.abs();
    let secs = t % 60.;
    let mut t = (t / 60.) as u64;
    let mins = t % 60;
    t /= 60;
    let hrs = t % 100;
    format!("{}{hrs:02}:{mins:02}:{secs:05.2}", if f { "-" } else { "" })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
extern "C" {
    fn on_game_start();
}

#[derive(PartialEq, Eq)]
pub enum GameMode {
    Normal,
    TweakOffset,
    Exercise,
    NoRetry,
    View,
}

#[derive(Clone)]
enum State {
    Starting,
    BeforeMusic,
    Playing,
    Ending,
}

pub struct GameScene {
    should_exit: bool,
    next_scene: Option<NextScene>,

    pub mode: GameMode,
    pub res: Resource,
    pub chart: Chart,
    pub judge: Judge,
    pub gl: InternalGlContext<'static>,
    player: Option<BasicPlayer>,
    chart_bytes: Vec<u8>,
    chart_format: ChartFormat,
    info_offset: f32,
    effects: Vec<Effect>,

    first_in: bool,
    exercise_range: Range<f64>,
    exercise_press: Option<(i8, u64)>,
    exercise_btns: (RectButton, RectButton),

    pub music: Music,

    state: State,
    pub last_update_time: f64,
    pause_rewind: Option<f64>,
    pause_first_time: f32,
    /// 摇一摇再玩：是否为「因为静止而自动暂停」（只自动恢复自己暂停的）
    paused_by_motion: bool,
    /// 持续静止的累计秒数
    still_time: f32,
    /// 上一次采样摇动强度的真实时间
    motion_last: f64,

    pub bad_notes: Vec<BadNote>,

    upload_fn: Option<UploadFn>,
    update_fn: Option<UpdateFn>,
    save_fn: Option<SaveFn>,

    best_record: Option<SimpleRecord>,

    pub touch_points: Vec<(f32, f32)>,
    fps_frame_count: u32,
    fps_total_time: f64,
    fps_last_frame_time: f64,

    dead: bool,
}

macro_rules! reset {
    ($self:ident, $res:expr, $tm:ident) => {{
        $self.bad_notes.clear();
        $self.judge.reset();
        $self.chart.reset();
        $res.health.reset();
        $res.judge_line_color = $res.res_pack.info.color_perfect();
        $self.music.pause()?;
        $self.music.seek_to(0.)?;
        $tm.speed = $res.config.speed as _;
        $tm.reset();
        $self.last_update_time = $tm.now();
        $self.state = State::Starting;
        $self.fps_frame_count = 0;
        $self.fps_total_time = 0.0;
        $self.fps_last_frame_time = $tm.real_time();
        $self.dead = false;
    }};
}

impl GameScene {
    pub const BEFORE_TIME: f64 = 0.7;
    pub const FADEOUT_TIME: f64 = WAIT_TIME + AFTER_TIME + 0.3;

    pub async fn load_chart_bytes(fs: &mut dyn FileSystem, info: &ChartInfo) -> Result<Vec<u8>> {
        if let Ok(bytes) = fs.load_file(&info.chart).await {
            return Ok(bytes);
        }
        if let Some(name) = info.chart.strip_suffix(".pec") {
            if let Ok(bytes) = fs.load_file(&concat_string!(name, ".json")).await {
                return Ok(bytes);
            }
        }
        bail!("Cannot find chart file")
    }

    pub fn infer_chart_format(info: &ChartInfo, bytes: &[u8]) -> ChartFormat {
        info.format.clone().unwrap_or_else(|| {
            if let Ok(text) = String::from_utf8(bytes.to_vec()) {
                if text.starts_with('{') {
                    if text.contains("\"META\"") {
                        ChartFormat::Rpe
                    } else {
                        ChartFormat::Pgr
                    }
                } else {
                    ChartFormat::Pec
                }
            } else {
                ChartFormat::Pbc
            }
        })
    }

    pub async fn load_chart(fs: &mut dyn FileSystem, info: &ChartInfo) -> Result<(Chart, Vec<u8>, ChartFormat)> {
        let extra = fs.load_file("extra.json").await.ok().map(String::from_utf8).transpose()?;
        let extra = if let Some(extra) = extra {
            parse_extra(&extra, fs).await.context("Failed to parse extra")?
        } else {
            ChartExtra::default()
        };
        let bytes = Self::load_chart_bytes(fs, info).await.context("Failed to load chart")?;
        let format = Self::infer_chart_format(info, &bytes);
        let mut chart = match format {
            ChartFormat::Rpe => parse_rpe(&String::from_utf8_lossy(&bytes), fs, extra, info.use_rpe_170_speed.unwrap_or_default()).await,
            ChartFormat::Pgr => parse_phigros(&String::from_utf8_lossy(&bytes), extra),
            ChartFormat::Pec => parse_pec(&String::from_utf8_lossy(&bytes), extra),
            ChartFormat::Pbc => {
                let mut r = BinaryReader::new(Cursor::new(&bytes));
                r.read()
            }
        }?;
        chart.load_textures(fs).await?;
        chart.settings.hold_partial_cover = info.hold_partial_cover;
        Ok((chart, bytes, format))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        mode: GameMode,
        info: ChartInfo,
        mut config: Config,
        mut fs: Box<dyn FileSystem>,
        player: Option<BasicPlayer>,
        background: SafeTexture,
        illustration: SafeTexture,
        upload_fn: Option<UploadFn>,
        update_fn: Option<UpdateFn>,
        save_fn: Option<SaveFn>,
    ) -> Result<Self> {
        match mode {
            GameMode::TweakOffset => {
                config.mods.insert(Mods::AUTOPLAY);
            }
            GameMode::Exercise => {
                config.mods.remove(Mods::AUTOPLAY);
            }
            _ => {}
        }

        let (mut chart, chart_bytes, chart_format) = Self::load_chart(fs.deref_mut(), &info).await?;
        if config.mods.contains(Mods::NO_SHADER) {
            chart.extra.effects.clear();
            chart.extra.global_effects.clear();
        }
        let effects = std::mem::take(&mut chart.extra.global_effects);
        if config.fxaa {
            chart
                .extra
                .effects
                .push(Effect::new(0.0..f64::INFINITY, include_str!("fxaa.glsl"), Vec::new(), false).unwrap());
        }

        if config.has_mod(Mods::NIGHTCORE) {
            config.speed *= 1.5;
        }

        if config.has_mod(Mods::RAINBOW) {
            chart
                .extra
                .effects
                .push(Effect::new(0.0..f64::INFINITY, include_str!("rainbow.glsl"), Vec::new(), false).unwrap());
        }

        let info_offset = info.offset;
        let mut res = Resource::new(
            config,
            info,
            fs,
            player.as_ref().and_then(|it| it.avatar.clone()),
            background,
            illustration,
            chart.extra.effects.is_empty() && effects.is_empty(),
        )
        .await
        .context("Failed to load resources")?;

        // Prepare extra sfx from chart.hitsounds
        chart.hitsounds.drain().for_each(|(name, clip)| {
            if let Ok(clip) = res.create_sfx(clip) {
                res.extra_sfxs.insert(name, clip);
            }
        });

        let exercise_range = (chart.offset + info_offset + res.config.offset) as f64..res.track_length;

        let judge = Judge::new(&chart);

        let music = Self::new_music(&mut res)?;
        Ok(Self {
            should_exit: false,
            next_scene: None,

            mode,
            res,
            chart,
            judge,
            gl: unsafe { get_internal_gl() },
            player,
            chart_bytes,
            chart_format,
            effects,
            info_offset,

            first_in: false,
            exercise_range,
            exercise_press: None,
            exercise_btns: (RectButton::new(), RectButton::new()),

            music,

            state: State::Starting,
            last_update_time: 0.,
            pause_rewind: None,
            pause_first_time: f32::NEG_INFINITY,
            paused_by_motion: false,
            still_time: 0.,
            motion_last: 0.,

            bad_notes: Vec::new(),

            upload_fn,
            update_fn,
            save_fn,

            best_record: None,

            touch_points: Vec::new(),

            fps_frame_count: 0,
            fps_total_time: 0.0,
            fps_last_frame_time: 0.0,

            dead: false,
        })
    }

    fn new_music(res: &mut Resource) -> Result<Music> {
        res.audio.create_music(
            res.music.clone(),
            MusicParams {
                amplifier: res.config.volume_music as _,
                playback_rate: res.config.speed as _,
                ..Default::default()
            },
        )
    }

    fn touch_scale(&self) -> f32 {
        (screen_width() / screen_height()) / self.res.aspect_ratio
    }

    fn ui(&mut self, ui: &mut Ui, tm: &mut TimeManager) -> Result<()> {
        let time = tm.now();
        let p = match self.state {
            State::Starting => {
                if time <= Self::BEFORE_TIME {
                    1. - (1. - time / Self::BEFORE_TIME).powi(3)
                } else {
                    1.
                }
            }
            State::BeforeMusic => 1.,
            State::Playing => 1.,
            State::Ending => {
                let t = time - self.res.track_length - WAIT_TIME;
                1. - (t / (AFTER_TIME + 0.3)).min(1.).powi(2)
            }
        } as f32;
        let res = &mut self.res;
        let eps = 2e-2 / res.aspect_ratio;
        let top = -1. / res.aspect_ratio;
        let pause_w = 0.015;
        let pause_h = pause_w * 3.2;
        let pause_center = Point::new(pause_w * 4.0 - 1., top + eps * 3.5 - (1. - p) * 0.4 + pause_h / 2.);
        if res.config.interactive
            && !tm.paused()
            && self.pause_rewind.is_none()
            && Judge::get_touches().iter().any(|touch| {
                touch.phase == TouchPhase::Started && {
                    let p = touch.position;
                    let p = Point::new(p.x, p.y);
                    (pause_center - p).norm() < 0.05
                }
            })
        {
            let t = tm.now() as f32;
            if t - self.pause_first_time > PAUSE_CLICK_INTERVAL && res.config.double_click_to_pause {
                self.pause_first_time = t;
            } else {
                self.pause_first_time = f32::NEG_INFINITY;
                if !self.music.paused() {
                    self.music.pause()?;
                }
                tm.pause();
                #[cfg(target_env = "ohos")]
                miniquad::native::set_interceptor_state(false);
            }
        }
        ui.alpha(res.alpha, |ui| {
            ui.text("MAGIC BUGFIX TEXT").color(Color::new(0., 0., 0., 0.)).draw();
            if tm.now() as f32 - self.pause_first_time <= PAUSE_CLICK_INTERVAL {
                ui.fill_circle(pause_center.x, pause_center.y, 0.05, Color::new(1., 1., 1., 0.5));
            }

            let margin = 0.03;
            let font = text_font(&res.config);

            let legacy_aui = !res.info.use_attach_ui_fix.unwrap_or_default();
            let unit_h = if legacy_aui { ui.text("0").measure_using(font).h } else { 0. };

            // score
            let h = 0.07;
            let score_top = top + eps * 2.2 - (1. - p) * 0.4;
            let score_right = 1. - margin;
            let score = format!("{:07}", self.judge.score(res.config.has_mod(Mods::NO_COMBO_SCORE)));
            // 和 Phi-Recorder 一样留一个宽度上限（0.55 * 谱面宽高比），超了就缩字号而不是溢出屏幕。
            // 正常 7 位数分数远不到这个宽度，所以实际是原版行为。
            let max_width = 0.55 * res.aspect_ratio;
            let mut text_size = 0.8;
            let w = ui.text(&score).size(text_size).measure_using(font).w;
            if w > max_width {
                text_size *= max_width / w;
            }
            let scale_point = legacy_aui.then(|| {
                let ct = ui.text(&score).size(text_size).measure_using(font).center();
                (score_right - ct.x, score_top + ct.y)
            });
            self.chart
                .with_element(ui, res, UIElement::Score, scale_point, (score_right, score_top), |ui, c| {
                    ui.text(&score)
                        .pos(score_right, score_top)
                        .anchor(1., 0.)
                        .size(text_size)
                        .color(c)
                        .draw_using(font);
                    if res.config.show_acc {
                        ui.text(format!("{:05.2}%", self.judge.real_time_accuracy() * 100.))
                            .pos(1. - margin, score_top + h)
                            .anchor(1., 0.)
                            .size(0.4)
                            .color(Color { a: c.a * 0.7, ..c })
                            .draw_using(font);
                    }
                });

            self.chart.with_element(
                ui,
                res,
                UIElement::Pause,
                legacy_aui.then(|| (pause_center.x, pause_center.y)),
                (pause_center.x - pause_w * 1.5, pause_center.y - pause_h / 2.),
                |ui, c| {
                    let mut r = Rect::new(pause_center.x - pause_w * 1.5, pause_center.y - pause_h / 2., pause_w, pause_h);
                    ui.fill_rect(r, c);
                    r.x += pause_w * 2.;
                    ui.fill_rect(r, c);
                },
            );
            if self.judge.combo() >= 3 {
                let combo = self.judge.combo().to_string();
                let combo_label = res.config.combo_label();
                // 连击数字同样留宽度上限；连击文字（可以自定义成任意长）超了自动缩字号，
                // 不然会直接画到屏幕外面去。
                let mut combo_size = 1.0;
                let w = ui.text(&combo).size(combo_size).measure_using(font).w;
                if w > max_width {
                    combo_size *= max_width / w;
                }
                if legacy_aui {
                    let combo_top = top + eps * 2. - (1. - p) * 0.4;
                    let btm = self
                        .chart
                        .with_element(ui, res, UIElement::ComboNumber, None, (0., combo_top + unit_h / 2.), |ui, c| {
                            ui.text(&combo)
                                .pos(0., combo_top)
                                .anchor(0.5, 0.)
                                .size(combo_size)
                                .color(c)
                                .draw_using(font)
                                .bottom()
                        });
                    let combo_top = btm + 0.01;
                    self.chart
                        .with_element(ui, res, UIElement::Combo, None, (0., combo_top + unit_h * 0.2), |ui, c| {
                            draw_text_aligned_opt_width_using(ui, font, &combo_label, 0., combo_top, (0.5, 0.), 0.4, c, max_width);
                        });
                } else {
                    let ct = ui.text(&combo).size(combo_size).measure_using(font).center();
                    let combo_y = top + eps * 2. - (1. - p) * 0.4 + ct.y;
                    let btm = self.chart.with_element(ui, res, UIElement::ComboNumber, None, (0., combo_y), |ui, c| {
                        ui.text(&combo)
                            .pos(0., combo_y)
                            .anchor(0.5, 0.5)
                            .size(combo_size)
                            .color(c)
                            .draw_using(font)
                            .bottom()
                    });
                    let ct = ui.text(&*combo_label).size(0.4).measure_using(font).center();
                    let combo_top = btm + 0.01 + ct.y;
                    self.chart.with_element(ui, res, UIElement::Combo, None, (0., combo_top), |ui, c| {
                        draw_text_aligned_opt_width_using(ui, font, &combo_label, 0., combo_top, (0.5, 0.5), 0.4, c, max_width);
                    });
                }
            }
            // 血条模式：左侧竖直血条 + 血量数字（颜色与长度可在设置里调）
            if res.config.health_mode() && matches!(self.mode, GameMode::Normal | GameMode::NoRetry | GameMode::View) {
                let (cr, cg, cb) = res.config.health_bar_color.rgb();
                let x = -1. + margin * 0.8;
                let w = 0.028;
                let full = 1. / res.aspect_ratio - eps * 3.5 - (top + eps * 3.5);
                let h = full * res.config.health_bar_len_ratio();
                // 竖直居中
                let y0 = top + eps * 3.5 + (full - h) / 2.;
                let y1 = y0 + h;
                let filled = h * res.health.ratio();
                ui.fill_rect(Rect::new(x, y0, w, h), Color::new(0.28, 0.28, 0.28, 0.7));
                ui.fill_rect(Rect::new(x, y1 - filled, w, filled), Color::new(cr, cg, cb, 0.95));
                draw_text_aligned_opt_width_using(
                    ui,
                    font,
                    &format!("{:.0}", res.health.hp),
                    x + w / 2.,
                    y1 - filled - 0.012,
                    (0.5, 1.),
                    // Phi-Recorder 的血量数字是 0.4（`0.4 * scale_ratio`）对应到我们的
                    // 「字宽归一化」坐标系就是 0.4，别自己另取一个值。
                    0.4,
                    semi_white(0.9),
                    0.9,
                );
            }
            // 摇一摇再玩：左上角实时显示当前摇动强度，方便确认传感器有没有在报数
            // （负数 = 这台设备没有传感器/没读到，此时不会自动暂停）
            if res.config.motion_pause {
                let level = motion_level();
                let text = if level < 0. {
                    "MOTION n/a".to_owned()
                } else {
                    format!("MOTION {level:.2}g / {:.2}g", res.config.motion_threshold_g())
                };
                ui.text(text)
                    .pos(-1. + margin, top + eps * 1.2)
                    .anchor(0., 0.)
                    .size(0.4)
                    .color(if level >= 0. && level >= res.config.motion_threshold_g() {
                        Color::new(0.5, 1., 0.6, 0.9)
                    } else {
                        semi_white(0.7)
                    })
                    .draw_using(font);
            }
            // Early / Late 实时提示：最近 0.8 秒内的判定按时间误差淡出。
            // 字号对齐 ACC（0.4），行距按字高（0.055 * 0.4 ≈ 0.022）的 2 倍留，最多 5 行。
            if res.config.early_late_hint {
                let now = res.time;
                let hint_size = 0.4;
                let hint_line = IN_GAME_TEXT_H * hint_size * 2.;
                let mut y = top + eps * 9.5;
                for (what, diff, at) in self.judge.feedback.iter().rev().take(5) {
                    let age = now - at;
                    if !(0. ..0.8).contains(&age) {
                        continue;
                    }
                    let alpha = (1. - age / 0.8) as f32;
                    let ms = diff * 1000.;
                    let color = match what {
                        Judgement::Perfect => Color::new(1., 1., 1., alpha),
                        Judgement::Good => Color::new(0.62, 0.85, 1., alpha),
                        _ => Color::new(1., 0.5, 0.5, alpha),
                    };
                    let text = if ms < 0. {
                        format!("EARLY {ms:.0}ms")
                    } else {
                        format!("LATE +{ms:.0}ms")
                    };
                    ui.text(text).pos(0., y).anchor(0.5, 0.).size(hint_size).color(color).draw_using(font);
                    y += hint_line;
                }
            }
            // magic to make score visible, refer to phira/src/rate.rs#L219
            ui.text("").draw_using(font);
            let lf = -1. + margin;
            let bt = -top - eps * 2.8 + (1. - p) * 0.4;
            // 曲名 / 难度原生用界面字体画（中文字形靠它）。开了 Phi-Recorder 字体后
            // 一并换成 assets/font.ttf，和 Phi-Recorder 里「整块打歌文字只有一套字体」一致。
            let name_painter = name_font(&res.config);
            let scale_point = legacy_aui.then(|| {
                let mut text = ui.text(&res.info.name).size(0.5);
                let ct = match name_painter {
                    Some(font) => text.measure_using(font).center(),
                    None => text.measure().center(),
                };
                (lf + ct.x, bt - ct.y)
            });
            self.chart.with_element(ui, res, UIElement::Name, scale_point, (lf, bt), |ui, c| {
                let mut text = ui.text(&res.info.name).pos(lf, bt).anchor(0., 1.).size(0.5).color(c).max_width(0.8);
                match name_painter {
                    Some(font) => text.draw_using(font),
                    None => text.draw(),
                }
            });

            let scale_point = legacy_aui.then(|| {
                let mut text = ui.text(&res.info.level).size(0.5);
                let ct = match name_painter {
                    Some(font) => text.measure_using(font).center(),
                    None => text.measure().center(),
                };
                (-lf - ct.x, bt - ct.y)
            });
            self.chart.with_element(ui, res, UIElement::Level, scale_point, (-lf, bt), |ui, c| {
                let mut text = ui.text(&res.info.level).pos(-lf, bt).anchor(1., 1.).size(0.5).color(c);
                match name_painter {
                    Some(font) => text.draw_using(font),
                    None => text.draw(),
                }
            });

            let hw = 0.003;
            let height = eps * 1.0;
            let dest = (2. * res.time / res.track_length).clamp(0., 2.) as f32;
            self.chart
                .with_element(ui, res, UIElement::Bar, Some((-1., top + height / 2.)), (-1., top + height / 2.), |ui, color| {
                    ui.fill_rect(Rect::new(-1., top, dest, height), semi_white(0.6));
                    ui.fill_rect(Rect::new(-1. + dest - hw, top, hw * 2., height), WHITE);
                });
        });
        Ok(())
    }

    fn overlay_ui(&mut self, ui: &mut Ui, tm: &mut TimeManager) -> Result<()> {
        let c = semi_white(self.res.alpha);
        let res = &mut self.res;
        if tm.paused() {
            let h = 1. / res.aspect_ratio;
            draw_rectangle(-1., -h, 2., h * 2., Color::new(0., 0., 0., 0.6));
            let o = if self.mode == GameMode::Exercise { -0.3 } else { 0. };
            let s = 0.06;
            let w = 0.05;
            let no_retry = self.mode == GameMode::NoRetry;
            draw_texture_ex(
                *res.icon_back,
                -s * 3. - w,
                -s + o,
                c,
                DrawTextureParams {
                    dest_size: Some(vec2(s * 2., s * 2.)),
                    ..Default::default()
                },
            );
            let r = Rect::new(0., o, 0., 0.).feather(s);
            let disabled_color = semi_white(res.alpha * 0.4);
            ui.fill_rect(r, (*res.icon_retry, r.feather(0.02), ScaleType::Fit, if no_retry { disabled_color } else { c }));
            draw_texture_ex(
                *res.icon_resume,
                s + w,
                -s + o,
                if self.dead { disabled_color } else { c },
                DrawTextureParams {
                    dest_size: Some(vec2(s * 2., s * 2.)),
                    ..Default::default()
                },
            );
            if res.config.interactive {
                let mut clicked = None;
                for touch in Judge::get_touches() {
                    if touch.phase != TouchPhase::Started {
                        continue;
                    }
                    let p = touch.position;
                    let p = Point::new(p.x, p.y);
                    for i in -1..=1 {
                        let ct = Point::new((s * 2. + w) * i as f32, o);
                        let d = p - ct;
                        if d.x.abs() <= s && d.y.abs() <= s {
                            clicked = Some(i);
                            break;
                        }
                    }
                }
                if no_retry && clicked == Some(0) || self.dead && clicked == Some(1) {
                    clicked = None;
                }
                let mut pos = self.music.position();
                if self.mode == GameMode::Exercise {
                    pos = tm.now();
                }
                if clicked.is_some_and(|it| it != -1) && (tm.speed - res.config.speed as f64).abs() > 0.01 {
                    debug!("recreating music");
                    self.music = res.audio.create_music(
                        res.music.clone(),
                        MusicParams {
                            amplifier: res.config.volume_music as _,
                            playback_rate: res.config.speed as _,
                            ..Default::default()
                        },
                    )?;
                }
                match clicked {
                    Some(-1) => {
                        self.should_exit = true;
                        #[cfg(target_env = "ohos")]
                        miniquad::native::set_interceptor_state(false);
                    }
                    Some(0) => {
                        reset!(self, res, tm);
                        if self.mode == GameMode::Exercise {
                            self.judge.advance_to(&mut self.chart, self.exercise_range.start);
                        }
                        #[cfg(target_env = "ohos")]
                        miniquad::native::set_interceptor_state(true);
                    }
                    Some(1) => {
                        if self.mode == GameMode::Exercise && (tm.now() > self.exercise_range.end || tm.now() < self.exercise_range.start) {
                            tm.seek_to(self.exercise_range.start);
                            self.music.seek_to(self.exercise_range.start)?;
                            pos = self.exercise_range.start;
                        }
                        self.music.play()?;
                        res.time -= 3.;
                        let dst = pos - 3.;
                        if dst < 0. {
                            self.music.pause()?;
                            self.state = State::BeforeMusic;
                        } else {
                            self.music.seek_to(dst)?;
                        }
                        let now = tm.now();
                        tm.speed = res.config.speed as _;
                        tm.resume();
                        tm.seek_to(now - 3.);
                        self.pause_rewind = Some(tm.now() - 0.2);
                        #[cfg(target_env = "ohos")]
                        miniquad::native::set_interceptor_state(true);
                    }
                    _ => {}
                }
            }
            if self.mode == GameMode::Exercise {
                let asp = self.touch_scale();
                for touch in ui.ensure_touches() {
                    touch.position *= asp;
                }
                ui.scope(|ui| {
                    ui.dx(0.3);
                    ui.dy(-0.3);
                    ui.slider(tl!("speed"), 0.5..2.0, 0.05, &mut self.res.config.speed, Some(0.5));
                });
                ui.dy(0.06);
                let hw = 0.7;
                let h = 0.06;
                let eh = 0.12;
                let rad = 0.03;
                let sp = self.offset().min(0.) as f64;
                ui.fill_rect(Rect::new(-hw, -h, hw * 2., h * 2.), GRAY);
                let st = -hw + ((self.exercise_range.start - sp) / (self.res.track_length - sp)) as f32 * hw * 2.;
                let en = -hw + ((self.exercise_range.end - sp) / (self.res.track_length - sp)) as f32 * hw * 2.;
                let t = tm.now();
                let cur = -hw + ((t - sp) / (self.res.track_length - sp)) as f32 * hw * 2.;
                ui.fill_rect(Rect::new(st, -h, en - st, h * 2.), WHITE);
                ui.fill_rect(Rect::new(st, -eh, 0., eh + h).feather(0.005), BLUE);
                ui.fill_circle(st, -eh, rad, BLUE);
                if self.exercise_press.is_none() {
                    let r = ui.rect_to_global(Rect::new(st, -eh, 0., 0.).feather(rad));
                    self.exercise_press = Judge::get_touches()
                        .iter()
                        .find(|it| it.phase == TouchPhase::Started && r.contains(it.position))
                        .map(|it| (-1, it.id));
                }
                ui.fill_rect(Rect::new(en, -h, 0., eh + h).feather(0.005), RED);
                ui.fill_circle(en, eh, rad, RED);
                if self.exercise_press.is_none() {
                    let r = ui.rect_to_global(Rect::new(en, eh, 0., 0.).feather(rad));
                    self.exercise_press = Judge::get_touches()
                        .iter()
                        .find(|it| it.phase == TouchPhase::Started && r.contains(it.position))
                        .map(|it| (1, it.id));
                }
                ui.fill_rect(Rect::new(cur, -h, 0., h * 2.).feather(0.005), GREEN);
                ui.fill_circle(cur, 0., rad, GREEN);
                if self.exercise_press.is_none() {
                    let r = ui.rect_to_global(Rect::new(cur, 0., 0., 0.).feather(rad));
                    self.exercise_press = Judge::get_touches()
                        .iter()
                        .find(|it| it.phase == TouchPhase::Started && r.contains(it.position))
                        .map(|it| (0, it.id));
                }
                ui.text(fmt_time(t as f32)).pos(0., -0.23).anchor(0.5, 0.).size(0.8).draw();
                if let Some((ctrl, id)) = &self.exercise_press {
                    if let Some(touch) = Judge::get_touches().iter().rfind(|it| it.id == *id) {
                        let x = touch.position.x;
                        let p = (x + hw) as f64 / (hw * 2.) as f64 * (self.res.track_length - sp) + sp;
                        let p = if self.res.track_length - sp <= 3. || *ctrl == 0 {
                            p.clamp(sp, self.res.track_length)
                        } else {
                            p.clamp(
                                if *ctrl == -1 { sp } else { self.exercise_range.start + 3. },
                                if *ctrl == -1 {
                                    self.exercise_range.end - 3.
                                } else {
                                    self.res.track_length
                                },
                            )
                        };
                        if *ctrl == 0 {
                            tm.seek_to(p);
                            self.music.seek_to(p)?;
                            self.bad_notes.clear();
                            self.judge.reset();
                            self.chart.reset();
                            self.res.judge_line_color = self.res.res_pack.info.color_perfect();
                        } else {
                            *(if *ctrl == -1 {
                                &mut self.exercise_range.start
                            } else {
                                &mut self.exercise_range.end
                            }) = p;
                        }
                        if matches!(touch.phase, TouchPhase::Cancelled | TouchPhase::Ended) {
                            self.exercise_press = None;
                        }
                    }
                }
                ui.dy(0.2);
                let r = ui.text(tl!("to")).size(0.8).anchor(0.5, 0.).draw();
                let mut tx = ui
                    .text(fmt_time(self.exercise_range.start as f32))
                    .pos(r.x - 0.02, 0.)
                    .anchor(1., 0.)
                    .size(0.8)
                    .color(BLACK);
                let re = tx.measure();
                self.exercise_btns.0.set(tx.ui, re);
                tx.ui
                    .fill_rect(re.feather(0.01), Color::new(1., 1., 1., if self.exercise_btns.0.touching() { 0.5 } else { 1. }));
                tx.draw();

                let mut tx = ui
                    .text(fmt_time(self.exercise_range.end as f32))
                    .pos(r.right() + 0.02, 0.)
                    .size(0.8)
                    .color(BLACK);
                let re = tx.measure();
                self.exercise_btns.1.set(tx.ui, re);
                tx.ui
                    .fill_rect(re.feather(0.01), Color::new(1., 1., 1., if self.exercise_btns.1.touching() { 0.5 } else { 1. }));
                tx.draw();
                for touch in ui.ensure_touches() {
                    touch.position /= asp;
                }
            }
        }
        if let Some(time) = self.pause_rewind {
            let dt = tm.now() - time;
            let t = 3 - dt.floor() as i32;
            if t <= 0 {
                self.pause_rewind = None;
            } else {
                let a = (1. - dt as f32 / 3.) * 1.;
                let h = 1. / self.res.aspect_ratio;
                draw_rectangle(-1., -h, 2., h * 2., Color::new(0., 0., 0., a));
                ui.text(t.to_string()).anchor(0.5, 0.5).size(1.).color(c).draw();
            }
        }
        if self.res.config.chart_debug_line || self.res.config.chart_debug_note {
            self.debug_overlay(ui);
        }
        if self.res.config.pause_offset_adjust && self.mode != GameMode::TweakOffset && tm.paused() && self.pause_rewind.is_none() {
            self.pause_offset_ui(ui);
        }
        if self.res.config.touch_debug {
            // 触点标记：样式（圆点 / 命中特效贴图 / 头像）、颜色、不透明度都能配
            let alpha = self.res.config.touch_marker_alpha_value();
            let (r, g, b) = self.res.config.touch_marker_color.rgb();
            let color = Color::new(r, g, b, alpha);
            let style = self.res.config.touch_marker_style;
            for touch in Judge::get_touches() {
                self.draw_touch_marker(ui, touch.position, style, color);
            }
        }
        for pos in &self.touch_points {
            let alpha = self.res.config.touch_marker_alpha_value();
            let (r, g, b) = self.res.config.touch_marker_color.rgb();
            self.draw_touch_marker(ui, Vec2::new(pos.0, pos.1), self.res.config.touch_marker_style, Color::new(r, g, b, alpha));
        }
        Ok(())
    }

    /// 画一个触点标记。
    fn draw_touch_marker(&self, ui: &mut Ui, pos: Vec2, style: crate::config::TouchMarkerStyle, color: Color) {
        use crate::config::TouchMarkerStyle::*;
        const R: f32 = 0.04;
        match style {
            Circle => ui.fill_circle(pos.x, pos.y, R, color),
            HitFx => {
                let r = Rect::new(pos.x - R, pos.y - R, R * 2., R * 2.);
                ui.fill_rect(r, (*self.res.res_pack.hit_fx, r, ScaleType::Fit, color));
            }
            Avatar => {
                let r = Rect::new(pos.x - R, pos.y - R, R * 2., R * 2.);
                ui.fill_rect(r, (*self.res.player, r, ScaleType::Fit, color));
            }
        }
    }

    fn interactive(res: &Resource, state: &State) -> bool {
        res.config.interactive && matches!(state, State::Playing)
    }

    /// 调试显示：判定线编号 / 线高 / z-index，以及音符时间与横向判定范围。
    fn debug_overlay(&self, ui: &mut Ui) {
        let res = &self.res;
        let lines = &self.chart.lines;
        let font = text_font(&res.config);
        if res.config.chart_debug_line {
            for (id, line) in lines.iter().enumerate() {
                let tr = line.now_transform(res, lines);
                let pos = tr.transform_point(&Point::new(0., 0.));
                let h = line.height.now();
                // f32 的 ULP：越大说明这个线高在浮点上越不可靠（速度快到丢精度）
                let ulp = if h == 0. { 0. } else { f32::from_bits(h.to_bits() + 1) - h };
                let color = if ulp > 0.018518519 {
                    RED
                } else if ulp > 0.0018518519 {
                    YELLOW
                } else {
                    WHITE
                };
                let kind = match &line.kind {
                    JudgeLineKind::Normal => "",
                    JudgeLineKind::Texture(..) => " img",
                    JudgeLineKind::TextureGif(..) => " gif",
                    JudgeLineKind::Text(..) => " text",
                    JudgeLineKind::Paint(..) => " paint",
                };
                let z = line.z_index;
                let attach = if line.attach_ui.is_some() { " +ui" } else { "" };
                ui.text(format!("[{id}] h:{h:.2} z:{z}{attach}{kind}"))
                    .pos(pos.x, pos.y - 0.012)
                    .anchor(0.5, 1.)
                    .size(0.045)
                    .color(color)
                    .draw_using(font);
            }
        }
        if res.config.chart_debug_note {
            let x_diff = res.config.x_diff_max() as f32;
            for (id, line) in lines.iter().enumerate() {
                let tr = line.now_transform(res, lines);
                for note in &line.notes {
                    if (note.time - res.time).abs() > 0.35 {
                        continue;
                    }
                    let mat = tr * note.object.now(res);
                    let pos = mat.transform_point(&Point::new(0., 0.));
                    let half = x_diff * note.judge_area;
                    let a = mat.transform_point(&Point::new(-half, 0.));
                    let b = mat.transform_point(&Point::new(half, 0.));
                    ui.fill_rect(Rect::new(a.x, pos.y - 0.005, b.x - a.x, 0.01), Color::new(1., 0.6, 0.2, 0.3));
                    let kind = match &note.kind {
                        NoteKind::Click => "click",
                        NoteKind::Hold { .. } => "hold",
                        NoteKind::Flick => "flick",
                        NoteKind::Drag => "drag",
                    };
                    ui.text(format!("[{id}] t:{:.2} h:{:.0} {kind}", note.time, note.height))
                        .pos(pos.x, pos.y - 0.008)
                        .anchor(0.5, 1.)
                        .size(0.04)
                        .color(WHITE)
                        .draw_using(font);
                }
            }
        }
    }

    /// 点击后立刻改变 `config.offset`（判定立刻跟着变），并把新值交给客户端保存。
    /// 暂停时的微调面板：判定偏移 ±1ms / ±5ms / 重置，外加音乐 / 音效音量。
    ///
    /// 位置放在**屏幕下方**：暂停菜单的返回 / 重试 / 继续三个图标是屏幕正中的，
    /// 之前这块面板画在正中，直接和它们糊在一起了。
    /// 音量改完立刻生效（`Music::set_amplifier` + `UI_SFX_VOLUME`），
    /// 新值通过 `PENDING_VOLUME` 交给客户端写回配置保存。
    fn pause_offset_ui(&mut self, ui: &mut Ui) {
        // 字号按打歌界面的尺度来（见 `IN_GAME_TEXT_H`）：
        // 标题 0.6（≈ 屏幕高的 2.9%，和原生偏移面板的数值同档）、
        // 按钮 0.42（= 原生 `Ui::button` 的字号）、行文字 0.4（= ACC / 滑条标签）。
        // 之前这里写的是 0.1 / 0.05，字高只有屏幕高的 0.3%~0.5%（1080p 上 3~5 像素），
        // 整个面板等于一片没有字的黑块。
        let w = 0.94;
        let h = 0.38;
        let x = -w / 2.;
        // 贴着屏幕下边缘（y 正方向朝下），并且保证不侵入中间那三个暂停图标
        // （它们以屏幕中心为圆心、半高 0.06）。
        let bottom = 1. / self.res.aspect_ratio;
        let y = (bottom - h - 0.02).max(0.075);
        let touches = Judge::get_touches();
        let clicked = |rect: Rect| -> bool { touches.iter().any(|it| it.phase == TouchPhase::Started && rect.contains(it.position)) };
        let mut button = |ui: &mut Ui, rect: Rect, label: &str, size: f32| -> bool {
            let hit = clicked(rect);
            ui.fill_rect(
                rect,
                if hit {
                    Color::new(1., 1., 1., 0.85)
                } else {
                    Color::new(1., 1., 1., 0.22)
                },
            );
            ui.text(label)
                .pos(rect.center().x, rect.center().y)
                .anchor(0.5, 0.5)
                .max_width(rect.w * 0.86)
                .size(size)
                .color(if hit { BLACK } else { WHITE })
                .no_baseline()
                .draw();
            hit
        };
        ui.fill_rect(Rect::new(x, y, w, h), Color::new(0., 0., 0., 0.72));

        // ---- 判定偏移 ----
        ui.text(format!("{} {:+.0}ms", tl!("adjust-offset"), self.res.config.offset * 1000.))
            .pos(0., y + 0.03)
            .anchor(0.5, 0.)
            .size(0.6)
            .color(WHITE)
            .draw();
        let bw = 0.13;
        let bh = 0.075;
        let gap = 0.02;
        let total = bw * 5. + gap * 4.;
        let mut bx = x + (w - total) / 2.;
        let by = y + 0.115;
        let mut delta = 0.;
        let mut reset = false;
        for (label, d) in [("-5", -0.005f32), ("-1", -0.001), ("+1", 0.001), ("+5", 0.005)] {
            if button(ui, Rect::new(bx, by, bw, bh), label, 0.42) {
                delta += d;
            }
            bx += bw + gap;
        }
        if button(ui, Rect::new(bx, by, bw, bh), &tl!("offset-reset"), 0.36) {
            reset = true;
        }

        // ---- 音量（游戏内实时可调）----
        // 一行一个：左边文字标签，右边 [-] 数值 [+]
        let mut music_delta = 0.;
        let mut sfx_delta = 0.;
        let vh = 0.075;
        let mut row = |ui: &mut Ui, cy: f32, label: String, value: f32| -> f32 {
            ui.text(label)
                .pos(x + 0.045, cy)
                .anchor(0., 0.5)
                .size(0.4)
                .color(semi_white(0.9))
                .no_baseline()
                .draw();
            ui.text(format!("{:.0}%", value * 100.))
                .pos(x + 0.60, cy)
                .anchor(0.5, 0.5)
                .size(0.4)
                .color(WHITE)
                .no_baseline()
                .draw();
            let mut d = 0.;
            if button(ui, Rect::new(x + 0.46, cy - vh / 2., 0.10, vh), "-", 0.42) {
                d = -0.05;
            }
            if button(ui, Rect::new(x + 0.74, cy - vh / 2., 0.10, vh), "+", 0.42) {
                d = 0.05;
            }
            d
        };
        let music = self.res.config.volume_music.clamp(0., 1.);
        let sfx = self.res.config.volume_sfx.clamp(0., 1.);
        music_delta += row(ui, y + 0.255, tl!("volume-music").into_owned(), music);
        sfx_delta += row(ui, y + 0.33, tl!("volume-sfx").into_owned(), sfx);

        let mut volume_changed = false;
        if music_delta != 0. {
            let v = (self.res.config.volume_music + music_delta).clamp(0., 1.);
            self.res.config.volume_music = v;
            // 立刻作用到正在播放的曲子（不然得等这首打完才生效）
            let _ = self.music.set_amplifier(v);
            volume_changed = true;
        }
        if sfx_delta != 0. {
            let v = (self.res.config.volume_sfx + sfx_delta).clamp(0., 1.);
            self.res.config.volume_sfx = v;
            crate::ui::UI_SFX_VOLUME.store(v.to_bits(), std::sync::atomic::Ordering::Relaxed);
            // 顺手播一下按钮音，直接听效果
            crate::ui::button_hit();
            volume_changed = true;
        }
        if volume_changed {
            PENDING_VOLUME.with(|it| it.set(Some((self.res.config.volume_music, self.res.config.volume_sfx))));
        }

        if reset {
            self.res.config.offset = 0.;
            PENDING_OFFSET.with(|it| it.set(Some(0.)));
        } else if delta != 0. {
            self.res.config.offset = (self.res.config.offset + delta).clamp(-0.5, 0.5);
            PENDING_OFFSET.with(|it| it.set(Some(self.res.config.offset)));
        }
    }

    fn offset(&self) -> f32 {
        self.chart.offset + self.res.config.offset + self.info_offset
    }

    fn tweak_offset(&mut self, ui: &mut Ui, ita: bool) {
        ui.scope(|ui| {
            let width = 0.55;
            let height = 0.4;
            ui.dx(1. - width - 0.02);
            ui.dy(ui.top - height - 0.02);
            ui.fill_rect(Rect::new(0., 0., width, height), GRAY);
            ui.dy(0.02);
            ui.text(tl!("adjust-offset")).pos(width / 2., 0.).anchor(0.5, 0.).size(0.7).draw();
            ui.dy(0.16);
            let r = ui
                .text(format!("{}ms", (self.info_offset * 1000.).round() as i32))
                .pos(width / 2., 0.)
                .anchor(0.5, 0.)
                .size(0.6)
                .no_baseline()
                .draw();
            let d = 0.14;
            if ui.button("lg_sub", Rect::new(d, r.center().y, 0., 0.).feather(0.026), "-") && ita {
                self.info_offset -= 0.05;
            }
            if ui.button("lg_add", Rect::new(width - d, r.center().y, 0., 0.).feather(0.026), "+") && ita {
                self.info_offset += 0.05;
            }
            let d = 0.08;
            if ui.button("sm_sub", Rect::new(d, r.center().y, 0., 0.).feather(0.022), "-") && ita {
                self.info_offset -= 0.005;
            }
            if ui.button("sm_add", Rect::new(width - d, r.center().y, 0., 0.).feather(0.022), "+") && ita {
                self.info_offset += 0.005;
            }
            let d = 0.03;
            if ui.button("ti_sub", Rect::new(d, r.center().y, 0., 0.).feather(0.017), "-") && ita {
                self.info_offset -= 0.001;
            }
            if ui.button("ti_add", Rect::new(width - d, r.center().y, 0., 0.).feather(0.017), "+") && ita {
                self.info_offset += 0.001;
            }
            ui.dy(0.14);
            let pad = 0.02;
            let spacing = 0.01;
            let mut r = Rect::new(pad, 0., (width - pad * 2. - spacing * 2.) / 3., 0.06);
            if ui.button("cancel", r, tl!("offset-cancel")) {
                self.next_scene = Some(NextScene::PopWithResult(Box::new(None::<f32>)));
            }
            r.x += r.w + spacing;
            if ui.button("reset", r, tl!("offset-reset")) {
                self.info_offset = 0.;
            }
            r.x += r.w + spacing;
            if ui.button("save", r, tl!("offset-save")) {
                self.next_scene = Some(NextScene::PopWithResult(Box::new(Some(self.info_offset))));
            }
        });
    }
    pub fn get_avg_fps(&self) -> Option<f32> {
        if self.fps_frame_count > 0 && self.fps_total_time > 0.0 {
            Some(self.fps_frame_count as f32 / self.fps_total_time as f32)
        } else {
            None
        }
    }

    /// 实时 HUD（H1~H8）。
    ///
    /// 用自带的九宫格定位，和谱面的 attach-ui 无关；描边用「先画一层暗色偏移再画亮色」
    /// 冒充（文本 builder 没有真正的 stroke）。
    fn hud_ui(&self, ui: &mut Ui) {
        let res = &self.res;
        let config = &res.config;
        if !config.hud {
            return;
        }
        let alpha = config.hud_alpha_value() * res.alpha;
        if alpha <= 0.01 {
            return;
        }
        let judge = &self.judge;
        let font = text_font(config);
        // HUD 的字号：原生 ACC 是 0.4、连击文字也是 0.4，HUD 比它们再小一档取 0.35
        // （≈ 屏幕高的 1.7%，1080p 上约 18 像素），设置里的倍率在 0.5x~2x 之间调。
        // 行距按**字高**算（`IN_GAME_TEXT_H * size`），不是按 size 算：
        // 字高只有 0.055 * size，行距写 `size * 1.45` 会得到比字高大 26 倍的空档。
        let size = 0.35 * config.hud_size_scale();
        let text_h = IN_GAME_TEXT_H * size;
        let line_h = text_h * 1.9;
        let dim = semi_white(alpha * 0.92);
        let dim2 = semi_white(alpha * 0.72);
        let mut items: Vec<(String, Color)> = Vec::new();
        if config.hud_acc {
            items.push((format!("ACC {:.2}%", judge.real_time_accuracy() * 100.), dim));
        }
        if config.hud_counts {
            let c = judge.counts();
            items.push((format!("P{} G{} B{} M{}", c[0], c[1], c[2], c[3]), dim));
        }
        if config.hud_max_combo {
            items.push((format!("MAX {} ↑{}", judge.max_combo(), judge.combo()), dim));
        }
        if config.hud_delta {
            let text = match judge.feedback.last() {
                Some((_, diff, _)) => format!("{:+05.0}ms", diff * 1000.),
                None => "  ±--ms".to_owned(),
            };
            items.push((text, dim));
        }
        if config.hud_rks {
            let score = judge.score(config.has_mod(Mods::NO_COMBO_SCORE));
            items.push((format!("RKS~{:.2}", config.estimate_rks(res.info.difficulty, score)), dim));
        }
        if config.hud_time {
            items.push((format!("{:.1}s/{:.1}s", res.time.max(0.), res.track_length), dim));
        }
        if config.hud_fps {
            let fps = self.get_avg_fps().map(|it| format!("{it:.0} FPS")).unwrap_or_else(|| "-- FPS".to_owned());
            items.push((fps, dim2));
        }
        if items.is_empty() {
            return;
        }

        let (ax, ay) = config.hud_corner.anchor();
        let margin = 0.03 + text_h;
        let n = items.len() as f32;
        // 注意：这个坐标系的 y 向下为正，top 是屏幕上边缘（负值）
        let top = -1. / res.aspect_ratio;
        let x = ax * (1. - margin);
        let anchor_x = if ax > 0. {
            1.
        } else if ax < 0. {
            0.
        } else {
            0.5
        };
        let mut y = if ay > 0. {
            top + margin
        } else if ay < 0. {
            -top - margin - (n - 1.) * line_h
        } else {
            -(n - 1.) * line_h / 2.
        };
        let outline = config.hud_outline;
        // 描边是「同一行暗色偏移一遍」，偏移量按字高取，不能按 size 取
        let outline_off = text_h * 0.09;
        for (text, color) in &items {
            if outline {
                let mut shadow = *color;
                shadow.a *= 0.85;
                shadow.r = 0.;
                shadow.g = 0.;
                shadow.b = 0.;
                ui.text(text)
                    .pos(x + outline_off, y + outline_off)
                    .anchor(anchor_x, 0.)
                    .size(size)
                    .color(shadow)
                    .draw_using(font);
            }
            ui.text(text)
                .pos(x, y)
                .anchor(anchor_x, 0.)
                .size(size)
                .color(*color)
                .draw_using(font);
            y += line_h;
        }
        // 进度条跟着 HUD 一起走（hud_time 打开时才有）
        if config.hud_time {
            let w = 0.34;
            let h = 0.008;
            let bx = match anchor_x {
                1. => x - w,
                0. => x,
                _ => x - w / 2.,
            };
            let by = y - line_h + line_h * 0.45;
            ui.fill_rect(Rect::new(bx, by, w, h), semi_white(alpha * 0.25));
            let prog = (res.time / res.track_length.max(0.001)).clamp(0., 1.) as f32;
            ui.fill_rect(Rect::new(bx, by, w * prog, h), semi_white(alpha * 0.85));
        }
    }

    /// 判定文字（V12）：只显示最近一次判定，按配置的样式（淡出 / 上浮 / 弹出）出现。
    fn judge_text_ui(&self, ui: &mut Ui) {
        let res = &self.res;
        let config = &res.config;
        if !config.judge_text {
            return;
        }
        const LIFE: f64 = 0.75;
        let Some(&(what, at)) = self.judge.recent_judgements.last() else {
            return;
        };
        let age = res.time - at;
        if !(0. ..LIFE).contains(&age) {
            return;
        }
        let k = (age / LIFE) as f32;
        let alpha = ((1. - k) * 1.6).min(1.).max(0.) * res.alpha;
        if alpha <= 0.01 {
            return;
        }
        let style = config.judge_text_style;
        // 判定文字配色（P 金 / G 蓝 / Bad 红 / Miss 白），每种都配一层同色外发光
        let color = match what {
            Judgement::Perfect => Color::new(1.00, 0.85, 0.36, alpha),
            Judgement::Good => Color::new(0.42, 0.74, 1.00, alpha),
            Judgement::Bad => Color::new(1.00, 0.38, 0.38, alpha),
            Judgement::Miss => Color::new(1.00, 1.00, 1.00, alpha),
        };
        let text = match what {
            Judgement::Perfect => "PERFECT",
            Judgement::Good => "GOOD",
            Judgement::Bad => "BAD",
            Judgement::Miss => "MISS",
        };
        // 弹出：刚判定的 0.1 秒内从 1.5 倍缩回正常
        let pop = style.pop_ratio();
        let pop_scale = if pop > 0. {
            1. + (1. - (age / 0.1).min(1.)) as f32 * 0.5 * pop
        } else {
            1.
        };
        let rise = style.rise_ratio();
        let y = -0.04 - rise * k * 0.20;
        let size = config.judge_text_size_value() * pop_scale;
        let font = text_font(config);
        // 外发光半径按字高取（字高 ≈ IN_GAME_TEXT_H * size），不是按 size 取
        let glow_unit = IN_GAME_TEXT_H * size;
        // 外发光：同色、低透明度，围着本体画两圈（近的一圈实一点，远的一圈散一点）
        for (radius, strength) in [(0.8f32, 0.55f32), (1.7, 0.28)] {
            let mut glow = color;
            glow.a = alpha * strength;
            for (dx, dy) in [
                (-1., 0.),
                (1., 0.),
                (0., -1.),
                (0., 1.),
                (-0.707, -0.707),
                (0.707, -0.707),
                (-0.707, 0.707),
                (0.707, 0.707),
            ] {
                ui.text(text)
                    .pos(dx * glow_unit * radius, y + dy * glow_unit * radius)
                    .anchor(0.5, 0.5)
                    .size(size)
                    .color(glow)
                    .draw_using(font);
            }
        }
        ui.text(text).pos(0., y).anchor(0.5, 0.5).size(size).color(color).draw_using(font);
    }

    /// 漏键标记（V11）：Bad / Miss 的位置在判定线上留一个淡出的叉，方便复盘漏在哪。
    fn miss_marker_ui(&mut self, ui: &mut Ui) {
        let life = self.res.config.miss_marker_secs() as f64;
        if !self.res.config.miss_marker {
            return;
        }
        let res = &mut self.res;
        let chart = &self.chart;
        let base_alpha = res.alpha;
        let now = res.time;
        for mark in &self.judge.miss_marks {
            let age = now - mark.time;
            if !(0. ..life).contains(&age) {
                continue;
            }
            let k = (age / life) as f32;
            let mut color = Color::new(1., 0.32, 0.32, (1. - k) * (1. - k) * 0.95 * base_alpha);
            if color.a <= 0.01 {
                continue;
            }
            let Some(line) = chart.lines.get(mark.line as usize) else {
                continue;
            };
            let tr = line.now_transform(res, &chart.lines);
            let x = mark.x;
            color.a *= 1.;
            res.with_model(tr, |_res| {
                let s = 0.02;
                let w = 0.006;
                draw_line(x - s, -s, x + s, s, w, color);
                draw_line(x - s, s, x + s, -s, w, color);
            });
        }
        let _ = ui;
    }
}

impl Scene for GameScene {
    fn enter(&mut self, tm: &mut TimeManager, target: Option<RenderTarget>) -> Result<()> {
        #[cfg(target_arch = "wasm32")]
        on_game_start();
        #[cfg(target_env = "ohos")]
        miniquad::native::set_interceptor_state(true);
        self.music = Self::new_music(&mut self.res)?;
        self.res.camera.render_target = target;
        tm.speed = self.res.config.speed as _;
        tm.adjust_time = self.res.config.adjust_time;
        reset!(self, self.res, tm);
        set_camera(&self.res.camera);
        self.first_in = true;
        Ok(())
    }

    fn pause(&mut self, tm: &mut TimeManager) -> Result<()> {
        if !tm.paused() {
            self.pause_rewind = None;
            self.music.pause()?;
            tm.pause();
        }
        #[cfg(target_env = "ohos")]
        miniquad::native::set_interceptor_state(false);
        Ok(())
    }

    fn resume(&mut self, tm: &mut TimeManager) -> Result<()> {
        if !matches!(self.state, State::Playing) {
            tm.resume();
        }
        Ok(())
    }

    fn update(&mut self, tm: &mut TimeManager) -> Result<()> {
        self.res.audio.recover_if_needed()?;
        // 摇一摇再玩：设备静止一段时间就自动暂停，摇动超过阈值就自动继续
        // （只恢复自己暂停的，不会打断玩家手动暂停）
        if self.res.config.motion_pause
            && !self.dead
            && matches!(self.mode, GameMode::Normal | GameMode::NoRetry | GameMode::View)
            && motion_level() >= 0.
        {
            let now = tm.real_time();
            let dt = (now - self.motion_last).clamp(0., 0.5) as f32;
            self.motion_last = now;
            let level = motion_level();
            if level >= self.res.config.motion_threshold_g() {
                self.still_time = 0.;
                if self.paused_by_motion {
                    self.paused_by_motion = false;
                    if tm.paused() {
                        self.music.play()?;
                        tm.resume();
                        show_message(tl!("shake-resumed")).ok();
                    }
                }
            } else if !tm.paused() && matches!(self.state, State::Playing) {
                self.still_time += dt;
                if self.still_time >= self.res.config.motion_still_secs() {
                    self.paused_by_motion = true;
                    self.still_time = 0.;
                    if !self.music.paused() {
                        self.music.pause()?;
                    }
                    tm.pause();
                    show_message(tl!("still-paused")).ok();
                }
            } else if !self.paused_by_motion {
                self.still_time = 0.;
            }
        }
        if matches!(self.state, State::Playing) {
            tm.update(self.music.position());
        }
        if self.mode == GameMode::Exercise && tm.now() > self.exercise_range.end && !tm.paused() {
            let state = self.state.clone();
            reset!(self, self.res, tm);
            self.state = state;
            tm.seek_to(self.exercise_range.start);
            tm.pause();
            self.music.pause()?;
            #[cfg(target_env = "ohos")]
            miniquad::native::set_interceptor_state(false);
        }
        let offset = self.offset();
        let time = tm.now();
        let time = match self.state {
            State::Starting => {
                if time >= Self::BEFORE_TIME {
                    self.res.alpha = 1.;
                    self.state = State::BeforeMusic;
                    tm.reset();
                    tm.seek_to(if self.mode == GameMode::Exercise {
                        self.exercise_range.start
                    } else {
                        offset.min(0.) as f64
                    });
                    self.last_update_time = tm.real_time();
                    if self.first_in && self.mode == GameMode::Exercise {
                        tm.pause();
                        self.first_in = false;
                    }
                    tm.now()
                } else {
                    #[cfg(target_os = "windows")]
                    {
                        // wtf bro. why must particles exist on Windows?
                        let emitter_config = self.res.emitter.emitter.config.clone();
                        let emitter_square_config = self.res.emitter.emitter_square.config.clone();
                        self.res.emitter.emitter.config.size = 0.0;
                        self.res.emitter.emitter_square.config.size = 0.0;
                        self.res.emitter.emitter.emit(vec2(0.0, 0.0), 1);
                        self.res.emitter.emitter_square.emit(vec2(0.0, 0.0), 1);
                        self.res.emitter.emitter.config = emitter_config;
                        self.res.emitter.emitter_square.config = emitter_square_config;
                    }
                    self.res.alpha = (1. - (1. - time / Self::BEFORE_TIME).powi(3)) as f32;
                    if self.mode == GameMode::Exercise {
                        self.exercise_range.start
                    } else {
                        offset as f64
                    }
                }
            }
            State::BeforeMusic => {
                if time >= 0.0 {
                    self.music.seek_to(time)?;
                    if !tm.paused() {
                        self.music.play()?;
                    }
                    self.state = State::Playing;
                }
                time
            }
            State::Playing => {
                if time > self.res.track_length + WAIT_TIME {
                    self.state = State::Ending;
                    #[cfg(target_env = "ohos")]
                    miniquad::native::set_interceptor_state(false);
                }
                time
            }
            State::Ending => {
                let t = time - self.res.track_length - WAIT_TIME;
                if t >= AFTER_TIME + 0.3 {
                    let mut record_data = None;
                    // TODO strengthen the protection
                    #[cfg(closed)]
                    if let Some(upload_fn) = &self.upload_fn {
                        if !self.res.config.offline_mode
                            && !self.res.config.mods.intersects(Mods::UNRATED)
                            && !self.res.config.use_keyboard
                            && self.res.config.speed >= 1.0 - 1e-3
                        {
                            if let Some(player) = &self.player {
                                if let Some(chart) = &self.res.info.id {
                                    record_data = Some(encode_record(self, player.id, *chart));
                                }
                            }
                        }
                    }
                    let no_combo_score = self.res.config.has_mod(Mods::NO_COMBO_SCORE);
                    let result = self.judge.result(no_combo_score);
                    let record = if self.res.config.mods.intersects(Mods::UNRATED) || self.res.config.speed < 1.0 - 1e-3 {
                        None
                    } else {
                        Some(SimpleRecord {
                            score: result.score as _,
                            accuracy: result.accuracy as _,
                            full_combo: result.max_combo == result.num_of_notes,
                            counts: result.counts,
                            max_combo: result.max_combo,
                            num_of_notes: result.num_of_notes,
                            hist: result.hist.to_vec(),
                        })
                    };
                    self.next_scene = match self.mode {
                        GameMode::Normal | GameMode::NoRetry | GameMode::View => {
                            let historic_best = self.player.as_ref().map_or(0, |it| it.historic_best);
                            if let Some(new_rec) = &record {
                                if let Some(f) = &self.save_fn {
                                    f(new_rec.clone())?;
                                }
                                if let Some(best) = &mut self.best_record {
                                    best.update(new_rec);
                                } else {
                                    self.best_record = record.clone();
                                }
                                if let Some(best) = &self.best_record {
                                    if let Some(player) = &mut self.player {
                                        player.historic_best = player.historic_best.max(best.score as _);
                                    }
                                }
                            }
                            Some(NextScene::Overlay(Box::new(EndingScene::new(
                                self.res.background.clone(),
                                self.res.illustration.clone(),
                                self.res.player.clone(),
                                self.res.icons.clone(),
                                self.res.icon_retry.clone(),
                                self.res.icon_proceed.clone(),
                                self.res.mod_icons.clone(),
                                self.res.challenge_icons[self.res.config.challenge_color as usize].clone(),
                                self.res.config.challenge_rank,
                                self.res.info.clone(),
                                self.judge.result(no_combo_score),
                                &self.res.config,
                                self.res.res_pack.ending.clone(),
                                self.upload_fn.as_ref().map(Arc::clone),
                                self.player.as_ref().map(|it| it.rks),
                                historic_best,
                                record_data,
                                self.best_record.clone(),
                                if self.res.config.show_avg_fps { self.get_avg_fps() } else { None },
                            )?)))
                        }
                        GameMode::TweakOffset => Some(NextScene::PopWithResult(Box::new(None::<f32>))),
                        GameMode::Exercise => None,
                    };
                }
                self.res.alpha = (1. - (t / AFTER_TIME).min(1.).powi(2)) as f32;
                self.res.track_length
            }
        };
        let time = (time - offset as f64).max(0.);
        self.res.time = time;
        if !tm.paused() && self.pause_rewind.is_none() && self.mode != GameMode::View {
            self.gl.quad_gl.viewport(self.res.camera.viewport);
            self.judge.update(&mut self.res, &mut self.chart, &mut self.bad_notes);
            self.gl.quad_gl.viewport(None);
        }
        if let Some(update) = &mut self.update_fn {
            update(self.res.time, &mut self.res, &mut self.judge);
        }
        let counts = self.judge.counts();
        self.res.judge_line_color = if counts[2] + counts[3] == 0 && self.res.config.ap_fc_indicator {
            if counts[1] == 0 {
                self.res.res_pack.info.color_perfect()
            } else {
                self.res.res_pack.info.color_good()
            }
        } else {
            WHITE
        };
        if !self.dead
            && matches!(self.state, State::Playing)
            && (self.res.config.mods.contains(Mods::INSTANT_DEATH_AP) && counts[1] + counts[2] + counts[3] > 0
                || self.res.config.mods.contains(Mods::INSTANT_DEATH_FC) && counts[2] + counts[3] > 0
                // 血条模式：只在会显示血条的模式下判死（练习 / 调偏移模式不判，与血条显示保持一致）
                || (self.res.config.health_mode()
                    && self.res.health.failed
                    && matches!(self.mode, GameMode::Normal | GameMode::NoRetry | GameMode::View)))
        {
            if !self.music.paused() {
                self.music.pause()?;
            }
            tm.pause();
            self.dead = true;
            #[cfg(target_env = "ohos")]
            miniquad::native::set_interceptor_state(false);
            show_message(tl!("game-over")).error();
        }
        self.res.judge_line_color.a *= self.res.alpha;
        self.chart.update(&mut self.res);
        let res = &mut self.res;
        if res.config.interactive && is_key_pressed(KeyCode::Space) {
            if tm.paused() {
                if matches!(self.state, State::Playing) {
                    self.music.play()?;
                    tm.resume();
                }
            } else if matches!(self.state, State::Playing | State::BeforeMusic) {
                if !self.music.paused() {
                    self.music.pause()?;
                }
                tm.pause();
            }
        }
        if Self::interactive(res, &self.state) {
            if is_key_pressed(KeyCode::Left) && res.config.use_keyboard {
                res.time -= 1.;
                let dst = (self.music.position() - 1.).max(0.);
                self.music.seek_to(dst)?;
                tm.seek_to(dst);
            }
            if is_key_pressed(KeyCode::Right) && res.config.use_keyboard {
                res.time += 5.;
                let dst = (self.music.position() + 5.).min(res.track_length);
                self.music.seek_to(dst)?;
                tm.seek_to(dst);
            }
            if is_key_pressed(KeyCode::Q) {
                self.should_exit = true;
            }
        }
        for e in &mut self.effects {
            e.update(&self.res);
        }
        if let Some((id, text)) = take_input() {
            let offset = self.offset().min(0.);
            match id.as_str() {
                "exercise_start" => {
                    if let Some(t) = parse_time(&text) {
                        if !(offset as f64..self.res.track_length.min(self.exercise_range.end - 3.).max(offset as f64)).contains(&t) {
                            show_message(tl!("ex-time-out-of-range")).error();
                        } else {
                            self.exercise_range.start = t;
                            show_message(tl!("ex-time-set")).ok();
                        }
                    } else {
                        show_message(tl!("ex-invalid-format")).error();
                    }
                }
                "exercise_end" => {
                    if let Some(t) = parse_time(&text) {
                        if !((self.exercise_range.start + 3.).max(offset as f64).min(self.res.track_length)..self.res.track_length).contains(&t) {
                            show_message(tl!("ex-time-out-of-range")).error();
                        } else {
                            self.exercise_range.end = t;
                            show_message(tl!("ex-time-set")).ok();
                        }
                    } else {
                        show_message(tl!("ex-invalid-format")).error();
                    }
                }
                _ => return_input(id, text),
            }
        }
        Ok(())
    }

    fn touch(&mut self, tm: &mut TimeManager, touch: &Touch) -> Result<bool> {
        if self.mode == GameMode::Exercise && tm.paused() {
            let touch = Touch {
                position: touch.position * self.touch_scale(),
                ..touch.clone()
            };
            if self.exercise_btns.0.touch(&touch) {
                request_input("exercise_start", InputBox::new().default_text(fmt_time(self.exercise_range.start as f32)));
                return Ok(true);
            }
            if self.exercise_btns.1.touch(&touch) {
                request_input("exercise_end", InputBox::new().default_text(fmt_time(self.exercise_range.end as f32)));
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn render(&mut self, tm: &mut TimeManager, ui: &mut Ui) -> Result<()> {
        if self.res.config.show_avg_fps || self.res.config.hud_fps {
            let current_time = tm.real_time();
            if matches!(self.state, State::Playing) && !tm.paused() {
                let frame_delta = current_time - self.fps_last_frame_time;
                self.fps_total_time += frame_delta;
                self.fps_frame_count += 1;
            }
            self.fps_last_frame_time = current_time;
        }

        let res = &mut self.res;
        let asp = ui.viewport.2 as f32 / ui.viewport.3 as f32;
        if res.update_size(ui.viewport) || self.mode == GameMode::View {
            set_camera(&res.camera);
        }

        let msaa = res.config.sample_count > 1;

        let chart_onto = res
            .chart_target
            .as_ref()
            .map(|it| if msaa { it.input() } else { it.output() })
            .or(res.camera.render_target);
        push_camera_state();
        set_camera(&Camera2D {
            zoom: vec2(1., -asp),
            viewport: if res.chart_target.is_some() { None } else { Some(ui.viewport) },
            render_target: chart_onto,
            ..Default::default()
        });
        clear_background(BLACK);
        draw_background(*res.background);
        pop_camera_state();

        let chart_target_vp = if res.chart_target.is_some() {
            let vp = res.camera.viewport.unwrap();
            Some((vp.0 - ui.viewport.0, vp.1 - ui.viewport.1, vp.2, vp.3))
        } else {
            res.camera.viewport
        };
        self.gl.quad_gl.render_pass(chart_onto.map(|it| it.render_pass));
        self.gl.quad_gl.viewport(chart_target_vp);

        let h = 1. / res.aspect_ratio;
        draw_rectangle(-1., -h, 2., h * 2., Color::new(0., 0., 0., res.alpha * res.info.background_dim));

        // 音乐可视化：每帧算一次频谱（判定线渲染时读它画条）
        if res.config.music_spectrum {
            crate::spectrum::update(res.music.frames(), res.music.sample_rate(), res.time, res.config.music_spectrum_gain_value());
        } else {
            crate::spectrum::silence();
        }
        self.chart.render(ui, res);

        self.gl.quad_gl.render_pass(
            res.chart_target
                .as_ref()
                .map(|it| it.output().render_pass)
                .or_else(|| res.camera.render_pass()),
        );

        self.bad_notes.retain(|dummy| dummy.render(res));
        let t = tm.real_time();
        let dt = (t - std::mem::replace(&mut self.last_update_time, t)) as f32;
        if res.config.particle {
            res.emitter.draw(dt);
        }
        self.ui(ui, tm)?;
        // 实时 HUD / 判定文字 / 漏键标记：都画在暂停遮罩（overlay_ui）下面
        self.hud_ui(ui);
        self.judge_text_ui(ui);
        self.miss_marker_ui(ui);
        self.overlay_ui(ui, tm)?;

        if self.mode == GameMode::TweakOffset {
            push_camera_state();
            self.gl.quad_gl.viewport(None);
            set_camera(&Camera2D {
                zoom: vec2(1., -screen_aspect()),
                render_target: self.res.chart_target.as_ref().map(|it| it.output()).or(self.res.camera.render_target),
                ..Default::default()
            });
            self.tweak_offset(ui, Self::interactive(&self.res, &self.state));
            pop_camera_state();
        }

        if !self.res.no_effect && !self.effects.is_empty() {
            push_camera_state();
            set_camera(&Camera2D {
                zoom: vec2(1., asp),
                ..Default::default()
            });
            for e in &self.effects {
                e.render(&mut self.res);
            }
            pop_camera_state();
        }
        if msaa || !self.res.no_effect {
            // render the texture onto screen
            if let Some(target) = &self.res.chart_target {
                self.gl.flush();
                push_camera_state();
                self.gl.quad_gl.viewport(None);
                set_camera(&Camera2D {
                    zoom: vec2(1., asp),
                    render_target: self.res.camera.render_target,
                    viewport: Some(ui.viewport),
                    ..Default::default()
                });
                draw_texture_ex(
                    target.output().texture,
                    -1.,
                    -ui.top,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(vec2(2., ui.top * 2.)),
                        ..Default::default()
                    },
                );
                pop_camera_state();
            }
        }
        Ok(())
    }

    fn next_scene(&mut self, tm: &mut TimeManager) -> NextScene {
        if self.should_exit {
            if tm.paused() {
                tm.resume();
            }
            tm.speed = 1.0;
            tm.adjust_time = false;
            match self.mode {
                // return result to update score and refresh
                GameMode::Normal => {
                    if let Some(rec) = &self.best_record {
                        NextScene::PopWithResult(Box::new(rec.clone()))
                    } else {
                        NextScene::Pop
                    }
                }
                // not sure if they need result. just keep it
                GameMode::Exercise | GameMode::NoRetry | GameMode::View => NextScene::Pop,
                GameMode::TweakOffset => NextScene::PopWithResult(Box::new(None::<f32>)),
            }
        } else if let Some(next_scene) = self.next_scene.take() {
            if !matches!(next_scene, NextScene::None) && tm.paused() {
                tm.resume();
            }
            tm.speed = 1.0;
            tm.adjust_time = false;
            next_scene
        } else {
            NextScene::None
        }
    }
}
