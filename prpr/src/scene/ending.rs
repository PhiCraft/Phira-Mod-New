prpr_l10n::tl_file!("ending");

use super::{draw_background, draw_illustration, game::SimpleRecord, loading::UploadFn, NextScene, Scene};
use crate::{
    config::{Config, Mods},
    core::{GAME_FONT, Matrix, Vector},
    ext::{
        create_audio_manger, draw_parallelogram, draw_parallelogram_ex, draw_text_aligned_opt_width_using, draw_text_aligned_using, nalgebra_to_glm, rect_shadow,
        semi_black, semi_white, SafeTexture, ScaleType, PARALLELOGRAM_SLOPE,
    },
    info::ChartInfo,
    judge::{icon_index, PlayResult, HIST_BUCKETS},
    scene::show_message,
    task::Task,
    time::TimeManager,
    ui::{DRectButton, Dialog, MessageHandle, Ui},
};
use anyhow::Result;
use macroquad::prelude::*;
use sasa::{AudioClip, AudioManager, Music, MusicParams};
use serde::Deserialize;
use std::{cell::RefCell, ops::DerefMut};

#[derive(Deserialize)]
pub struct RecordUpdateState {
    pub best: bool,
    pub improvement: u32,
    pub gain_exp: f32,
    pub new_rks: Option<f32>,
}

pub struct EndingScene {
    background: SafeTexture,
    illustration: SafeTexture,
    player: SafeTexture,
    icons: [SafeTexture; 8],
    icon_retry: SafeTexture,
    icon_proceed: SafeTexture,
    // The classic (prpr) ending shows the challenge badge instead of mod icons.
    #[allow(dead_code)]
    mod_icons: [SafeTexture; 7],
    challenge_icon: SafeTexture,
    challenge_rank: u32,
    target: Option<RenderTarget>,
    audio: AudioManager,
    bgm: Music,
    bgm_already_played: bool,
    /// 音频偏移，用于计算结算音效的播放时机
    offset: f32,

    info: ChartInfo,
    result: PlayResult,
    player_name: String,
    player_rks: Option<f32>,
    /// 设置里手动指定的 rks（优先于账号真实 rks 和本次成绩变化）
    rks_override: Option<f32>,
    // 下面这些原本用于分数板上的状态文字（AUTOPLAY / UNRATED / 速度 / NEW BEST），
    // 现在结算画面不再显示这行文字，字段保留备用。
    #[allow(dead_code)]
    autoplay: bool,
    #[allow(dead_code)]
    use_keyboard: bool,
    #[allow(dead_code)]
    speed: f32,
    #[allow(dead_code)]
    mods: Mods,
    /// 是否画判定时间分布图
    judge_chart: bool,
    next: u8, // 0 -> none, 1 -> pop, 2 -> exit
    update_state: Option<RecordUpdateState>,
    #[allow(dead_code)]
    rated: bool,

    upload_fn: Option<UploadFn>,
    upload_task: Option<(Task<Result<RecordUpdateState>>, MessageHandle)>,
    record_data: Option<Vec<u8>>,
    best_record: Option<SimpleRecord>,

    btn_retry: DRectButton,
    btn_proceed: DRectButton,

    tr_start: f32,

    #[allow(dead_code)]
    avg_fps: Option<f32>,
}

impl EndingScene {
    /// 结算画面出现到结算音效播放之间的等待时间（与参考一致）
    pub const BPM_WAIT_TIME: f64 = 0.70;

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        background: SafeTexture,
        illustration: SafeTexture,
        player: SafeTexture,
        icons: [SafeTexture; 8],
        icon_retry: SafeTexture,
        icon_proceed: SafeTexture,
        mod_icons: [SafeTexture; 7],
        challenge_icon: SafeTexture,
        challenge_rank: u32,
        info: ChartInfo,
        result: PlayResult,
        config: &Config,
        bgm: AudioClip,
        upload_fn: Option<UploadFn>,
        player_rks: Option<f32>,
        historic_best: u32,
        record_data: Option<Vec<u8>>,
        best_record: Option<SimpleRecord>,
        avg_fps: Option<f32>,
    ) -> Result<Self> {
        let mut audio = create_audio_manger(config)?;
        let bgm = audio.create_music(
            bgm,
            MusicParams {
                amplifier: config.volume_bgm,
                loop_mix_time: 0.,
                ..Default::default()
            },
        )?;
        let upload_task = upload_fn
            .as_ref()
            .and_then(|f| record_data.clone().map(|data| (f(data), show_message(tl!("uploading")).handle())));
        Ok(Self {
            background,
            illustration,
            player,
            icons,
            icon_retry,
            icon_proceed,
            mod_icons,
            challenge_icon,
            challenge_rank,
            target: None,
            audio,
            bgm,
            bgm_already_played: false,
            offset: config.offset,
            update_state: if upload_task.is_some() {
                None
            } else {
                let (best, improvement) = if result.score > historic_best {
                    (true, result.score - historic_best)
                } else {
                    (false, 0)
                };
                Some(RecordUpdateState {
                    best,
                    improvement,
                    gain_exp: 0.,
                    new_rks: None,
                })
            },
            rated: upload_task.is_some(),

            info,
            result,
            player_name: config.player_name.clone(),
            judge_chart: config.ending_judge_chart,
            player_rks,
            rks_override: config.rks_override,
            autoplay: config.autoplay(),
            use_keyboard: config.use_keyboard,
            speed: config.speed,
            mods: config.mods,
            next: 0,

            upload_fn,
            upload_task,
            record_data,
            best_record,

            btn_retry: DRectButton::new(),
            btn_proceed: DRectButton::new(),

            tr_start: f32::NAN,

            avg_fps,
        })
    }
}

thread_local! {
    static RE_UPLOAD: RefCell<bool> = RefCell::default();
}

impl Scene for EndingScene {
    fn enter(&mut self, tm: &mut TimeManager, target: Option<RenderTarget>) -> Result<()> {
        tm.reset();
        // 与参考一致：结算画面从 t = 0 开始（入场动画与结算音效的 0.7s 都按这个零点算）
        tm.seek_to(0.0);
        self.target = target;
        Ok(())
    }

    fn pause(&mut self, tm: &mut TimeManager) -> Result<()> {
        self.bgm.pause()?;
        tm.pause();
        Ok(())
    }

    fn resume(&mut self, tm: &mut TimeManager) -> Result<()> {
        self.bgm.play()?;
        tm.resume();
        Ok(())
    }

    fn touch(&mut self, tm: &mut TimeManager, touch: &Touch) -> Result<bool> {
        let t = tm.now() as f32;
        if self.btn_retry.touch(touch, t) {
            if self.upload_task.is_some() {
                show_message(tl!("still-uploading"));
            } else {
                self.tr_start = t;
                self.next = 1;
            }
            return Ok(true);
        }
        if self.btn_proceed.touch(touch, t) {
            if self.upload_task.is_some() {
                show_message(tl!("still-uploading"));
            } else {
                self.tr_start = t;
                self.next = 2;
            }
            return Ok(true);
        }
        Ok(false)
    }

    fn update(&mut self, tm: &mut TimeManager) -> Result<()> {
        self.audio.recover_if_needed()?;
        // 与参考一致：结算画面出现后再等 BPM_WAIT_TIME（受音频偏移影响）才播结算音效，且只播一次
        if !self.bgm_already_played && tm.now() >= Self::BPM_WAIT_TIME - self.offset as f64 && self.target.is_none() && self.bgm.paused() {
            self.bgm.play()?;
            self.bgm_already_played = true;
        }
        if RE_UPLOAD.with(|it| std::mem::replace(it.borrow_mut().deref_mut(), false)) && self.upload_task.is_none() {
            self.upload_task = self
                .record_data
                .clone()
                .map(|data| ((self.upload_fn.as_ref().unwrap())(data), show_message(tl!("uploading")).handle()));
        }
        if let Some((task, handle)) = &mut self.upload_task {
            if let Some(result) = task.take() {
                handle.cancel();
                match result {
                    Err(err) => {
                        let error = format!("{:?}", err.context(tl!("upload-failed")));
                        Dialog::plain(tl!("upload-failed"), error)
                            .buttons(vec![tl!("upload-cancel").to_string(), tl!("upload-retry").to_string()])
                            .listener(move |_dialog, pos| {
                                if pos == 1 {
                                    RE_UPLOAD.with(|it| *it.borrow_mut() = true);
                                }
                                false
                            })
                            .show();
                    }
                    Ok(state) => {
                        self.update_state = Some(state);
                        show_message(tl!("uploaded")).ok();
                    }
                }
                self.upload_task = None;
            }
        }
        Ok(())
    }

    fn render(&mut self, tm: &mut TimeManager, ui: &mut Ui) -> Result<()> {
        let mut cam = ui.camera();
        let asp = -cam.zoom.y;
        let top = 1. / asp;
        let t = tm.now() as f32;
        cam.render_target = self.target;
        let sr = ui.screen_rect();
        set_camera(&cam);
        draw_background(*self.background);

        fn ran(t: f32, l: f32, r: f32) -> f32 {
            ((t - l) / (r - l)).clamp(0., 1.)
        }

        /// 与参考实现一致的平移：`x` 个单位（1 = 半个屏幕宽）向右推动画。
        fn tran(x: f32) -> Mat4 {
            nalgebra_to_glm(&Matrix::new_translation(&Vector::new(x * 2., 0.)))
        }

        let score = self.result.score;
        let accuracy = self.result.accuracy;
        let max_combo = self.result.max_combo;
        let num_of_notes = self.result.num_of_notes;
        let counts = self.result.counts;
        let early = self.result.early;
        let late = self.result.late;

        let slope = PARALLELOGRAM_SLOPE;
        let dx = 0.07;
        let c = Color::new(0., 0., 0., 1.0);
        let c2 = Color::new(0., 0., 0., 0.5);

        // 三段式入场：主信息（曲绘 + 面板）→ 连击 / 准度 → 判定统计
        let p_main = (1. - ran(t, 0.00, 3.00) + 0.15).powi(10);

        // 曲绘（斜切平行四边形）+ 底部黑条上的难度 / 曲名
        let mut r = Rect::new(0., 0., 0., 0.);
        ui.with_gl(tran(p_main), |ui| {
            r = draw_illustration(*self.illustration, -0.372, -0.002, 1.052, 1.22, WHITE);
            let ratio = 0.2;
            draw_parallelogram_ex(
                Rect::new(r.x, r.y + r.h * (1. - ratio), r.w - r.h * (1. - ratio) * slope, r.h * ratio),
                None,
                Color::default(),
                Color::new(0., 0., 0., 0.7),
                true,
            );
            let p = (r.x + 0.055, r.bottom() - top / 14.5);
            let mw = (r.right() - p.0) * 0.4 - 0.02;
            draw_text_aligned_opt_width_using(
                ui,
                &GAME_FONT,
                &self.info.level,
                r.right() - r.h / 7. * 13. * 0.13 - 0.029,
                r.bottom() - top / 18.5,
                (1., 1.),
                0.40,
                WHITE,
                mw,
            );
            draw_text_aligned_opt_width_using(ui, &GAME_FONT, &self.info.name, p.0, p.1, (0., 1.), 0.92, WHITE, mw);
        });
        let main = Rect::new(r.right() - 0.053, r.y, r.w * 0.782, r.h / 2.);

        // 分数板：分数 + 成绩等级图标（第一段动画的后续淡入）
        ui.with_gl(tran((1. - ran(t, 0.00, 1.15)).powi(2) + p_main), |ui| {
            draw_parallelogram(main, None, c2, true);
            // 原来这里有一行 "PHIRA[AUTOPLAY]/[UNRATED]/速度/新纪录" 状态文字；
            // 现在这个位置改放判定时间分布图（Early ← → Late），
            // 依然用一次空白文字占位来保住行高，让分数位置与参考保持一致。
            let r0 = draw_text_aligned_using(ui, &GAME_FONT, "  ", main.x + dx + 0.01, main.bottom() - 0.040, (0., 1.), 0.34, WHITE);
            if self.judge_chart {
                let hist = self.result.hist;
                if hist.iter().any(|it| *it > 0) {
                    let bw = 0.0125;
                    let x0 = main.x + dx + 0.01;
                    let base = main.bottom() - 0.006;
                    let h = 0.030;
                    let mid = HIST_BUCKETS as i32 / 2;
                    let max = hist.iter().copied().max().unwrap_or(1).max(1) as f32;
                    for (i, count) in hist.iter().enumerate() {
                        let bh = h * (*count as f32 / max);
                        let color = match (i as i32).cmp(&mid) {
                            std::cmp::Ordering::Equal => Color::new(1., 0.95, 0.6, 0.95),
                            std::cmp::Ordering::Less => Color::new(0.45, 0.75, 1., 0.9),
                            std::cmp::Ordering::Greater => Color::new(1., 0.62, 0.35, 0.9),
                        };
                        ui.fill_rect(Rect::new(x0 + bw * i as f32, base - bh, bw * 0.8, bh), color);
                    }
                    // 中间（0ms）的参考线
                    ui.fill_rect(Rect::new(x0 + bw * mid as f32 + bw * 0.4 - 0.0005, base - h, 0.001, h), Color::new(1., 1., 1., 0.35));
                }
            }
            let pa = ran(t, 1.08, 1.35);
            let r0 = draw_text_aligned_opt_width_using(
                ui,
                &GAME_FONT,
                &format!("{score:07}"),
                r0.x - 0.012,
                r0.y - 0.019,
                (0., 1.),
                1.05,
                Color::new(1., 1., 1., pa),
                0.4,
            );
            let ps = ran(t, 2.50, 2.90).powi(3);
            let pa = ran(t, 2.40, 2.70);
            let s = main.h * 0.72;
            let ct = (main.right() + 0.015 - main.h * slope - s / 2., r0.bottom() + 0.033 - s / 2.);
            let s = s + s * (1. - ps) * 0.3;
            let icon = &self.icons[icon_index(score, max_combo == num_of_notes)];
            let ir = Rect::new(ct.0 - s * 0.99 / 2., ct.1 - s * 1.05 / 2., s * 0.99, s * 1.05);
            ui.fill_rect(ir, (**icon, ir, ScaleType::Fit, semi_white(pa)));
        });

        // 最大连击 / 准确率
        let d = r.h / 15.2;
        let s1 = Rect::new(main.x - d * 4. * slope, main.bottom() + d, main.w - d * 5. * slope, d * 2.8);
        ui.with_gl(tran((1. - ran(t, 0.00, 1.60)).powi(2) + p_main), |ui| {
            let dy = 0.025;
            let pa = ran(t, 1.50, 1.83);
            draw_parallelogram(s1, None, c2, true);
            let r1 = draw_text_aligned_using(ui, &GAME_FONT, "Max Combo", s1.x + dx - 0.005, s1.bottom() - dy, (0., 1.), 0.31, Color::new(1., 1., 1., pa));
            draw_text_aligned_opt_width_using(ui, &GAME_FONT, &max_combo.to_string(), r1.x, r1.y - 0.006, (0., 1.), 0.65, Color::new(1., 1., 1., pa), 0.3);
            let r1 = draw_text_aligned_using(ui, &GAME_FONT, "Accuracy", s1.right() - dx + 0.022, s1.bottom() - dy, (1., 1.), 0.31, Color::new(1., 1., 1., pa));
            draw_text_aligned_opt_width_using(
                ui,
                &GAME_FONT,
                &format!("{:.2}%", accuracy * 100.),
                r1.right(),
                r1.y - 0.008,
                (1., 1.),
                0.62,
                Color::new(1., 1., 1., pa),
                0.3,
            );
        });

        // 判定统计 + Early / Late
        let s2 = Rect::new(s1.x - d * 4. * slope, s1.bottom() + d, s1.w, s1.h);
        ui.with_gl(tran((1. - ran(t, 0.00, 2.05)).powi(2) + p_main), |ui| {
            let dy = 0.028;
            let dy2 = 0.010;
            let bg = 0.55;
            let sm = 0.21;
            let pa = ran(t, 1.91, 2.25);
            let text = Color::new(1., 1., 1., pa);
            draw_parallelogram(s2, None, c2, true);
            let draw_count = |ui: &mut Ui, ratio: f32, name: &str, count: u32| {
                let r = draw_text_aligned_using(ui, &GAME_FONT, name, s2.x + s2.w * ratio, s2.bottom() - dy, (0.5, 1.), sm, text);
                draw_text_aligned_opt_width_using(ui, &GAME_FONT, &count.to_string(), r.center().x, r.y - dy2, (0.5, 1.), bg, text, 0.125);
            };
            draw_count(ui, 0.127, "Perfect", counts[0]);
            draw_count(ui, 0.325, "Good", counts[1]);
            draw_count(ui, 0.46, "Bad", counts[2]);
            draw_count(ui, 0.595, "Miss", counts[3]);

            let sm = 0.32;
            let l = s2.x + s2.w * 0.72;
            let rt = s2.x + s2.w * 0.930;
            let cy = s2.center().y;
            let r2 = draw_text_aligned_using(ui, &GAME_FONT, "Early", l, cy, (0., 1.), sm, text);
            draw_text_aligned_opt_width_using(ui, &GAME_FONT, &early.to_string(), rt, r2.bottom(), (1., 1.), sm, text, 0.1);
            let r2 = draw_text_aligned_using(ui, &GAME_FONT, "Late", l, cy + dy2 / 2.3, (0., 0.), sm, text);
            draw_text_aligned_opt_width_using(ui, &GAME_FONT, &late.to_string(), rt, r2.y, (1., 0.), sm, text, 0.1);
        });

        // 重试（左上）/ 继续（右下）
        let dy = 0.010;
        let w = 0.202;
        let h = 0.117;
        let s = 0.10;
        let hs = h * 0.28;
        let p = (1. - ran(t, 1.20, 2.40)).powi(7);
        let p2 = (1. - ran(t, 1.35, 2.40)).powi(5);

        let retry = Rect::new(-1. - h * slope, -top + dy, w, h);
        // 入场动画播完之前不响应点击
        self.btn_retry.inner.set(ui, if p <= 0. { retry } else { Rect::new(-4., -4., 0.01, 0.01) });
        ui.with_gl(tran(-p * 0.1), |ui| {
            draw_parallelogram(retry, None, c, true);
            draw_parallelogram(Rect::new(retry.x + retry.w * (1. - s), retry.y, retry.w * s, retry.h), None, WHITE, false);
            let ct = retry.center();
            // 与参考一致：图标相对按钮中心略微右移
            let ir = Rect::new(ct.x - hs * 0.9, ct.y - hs, hs * 2., hs * 2.);
            ui.fill_rect(ir, (*self.icon_retry, ir, ScaleType::Fit, WHITE));
        });

        let proceed = Rect::new(1. + h * slope - w, top - dy - h, w, h);
        self.btn_proceed.inner.set(ui, if p2 <= 0. { proceed } else { Rect::new(-4., -4., 0.01, 0.01) });
        ui.with_gl(tran(p2 * 0.1), |ui| {
            draw_parallelogram(proceed, None, c, true);
            draw_parallelogram(Rect::new(proceed.x, proceed.y, proceed.w * s, proceed.h), None, WHITE, false);
            let ct = proceed.center();
            // 与参考一致：图标避让左侧白条
            let ir = Rect::new(ct.x - hs * 0.8 - w * s / 2., ct.y - hs, hs * 2., hs * 2.);
            ui.fill_rect(ir, (*self.icon_proceed, ir, ScaleType::Fit, WHITE));
        });

        // 玩家卡：rks 白牌 + 头像（斜切）+ 昵称黑牌 + 挑战徽章
        let alpha = ran(t, 1.25, 1.75);
        let card = Rect::new(1. - 0.27, -top + dy * 3.2, 0.35, 0.11);
        draw_parallelogram(card, None, Color::new(0., 0., 0., alpha), false);

        let sub = Rect::new(1. - 0.125, card.center().y + 0.015, 0.12, 0.03);
        let color = Color::new(1., 1., 1., alpha);
        draw_parallelogram(sub, None, color, false);
        let rks_text = match self.rks_override.or(self.player_rks) {
            Some(rks) => format!("{rks:.2}"),
            None => "16.00".to_owned(),
        };
        draw_text_aligned_opt_width_using(
            ui,
            &GAME_FONT,
            &rks_text,
            sub.center().x,
            sub.center().y - 0.002,
            (0.5, 0.5),
            0.37,
            Color::new(0., 0., 0., alpha),
            0.10,
        );

        let av = draw_illustration(*self.player, 1. - 0.21, card.center().y, 0.12 / (0.076 * 7.), 0.12 / (0.076 * 7.), color);
        let mut text = ui.text(&self.player_name).pos(av.x - 0.015, av.center().y - 0.002).anchor(1., 0.5).size(0.54).color(color);
        let text_rect = text.measure_using(&GAME_FONT);
        draw_parallelogram(
            Rect::new(text_rect.x - card.h * slope - 0.02, card.y, av.x - text_rect.x + card.h * slope * 2. + 0.021, card.h),
            None,
            Color::new(0., 0., 0., alpha),
            false,
        );
        text.draw_using(&GAME_FONT);

        let ct = (1. - 0.1 + 0.043, card.center().y - 0.034 + 0.02);
        let (bw, bh) = (0.09 * self.challenge_icon.width() / 78., 0.04 * self.challenge_icon.height() / 38.);
        let badge = Rect::new(ct.0 - bw / 2., ct.1 - bh / 2., bw, bh);
        ui.fill_rect(badge, (*self.challenge_icon, badge, ScaleType::Fit, color));
        let ct = badge.center();
        let rank_text = self.challenge_rank.to_string();
        let mut size = 0.46;
        let text_width = ui.text(&rank_text).size(size).measure_using(&GAME_FONT).w;
        if text_width > 0.05 {
            size *= 0.05 / text_width;
        }
        ui.text(&rank_text)
            .pos(ct.x, ct.y)
            .anchor(0.5, 1.)
            .size(size)
            .color(color)
            .draw_using(&GAME_FONT);

        // 退出过场
        if !self.tr_start.is_nan() {
            let p = ((t - self.tr_start) / 0.5).min(1.);
            if p >= 1. {
                self.tr_start = f32::NAN;
            }
            let p = 1. - (1. - p).powi(3);
            let mut r = sr;
            r.y -= r.h * (1. - p);
            rect_shadow(r, 0.01, 0.5);
            let (tex, alpha) = if self.next == 1 {
                (&self.background, 0.3)
            } else {
                (&self.illustration, 0.55)
            };
            ui.fill_rect(r, (**tex, r));
            ui.fill_rect(r, semi_black(alpha));
        }

        Ok(())
    }

    fn next_scene(&mut self, _tm: &mut TimeManager) -> NextScene {
        if !self.tr_start.is_nan() {
            return NextScene::None;
        }
        if self.next != 0 {
            let _ = self.bgm.pause();
        }
        match self.next {
            0 => NextScene::None,
            1 => NextScene::Pop,
            2 => {
                if let Some(rec) = &self.best_record {
                    NextScene::PopNWithResult(2, Box::new(rec.clone()))
                } else {
                    NextScene::PopN(2)
                }
            }
            _ => unreachable!(),
        }
    }
}
