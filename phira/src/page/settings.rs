prpr_l10n::tl_file!("settings");

use super::{NextPage, OffsetPage, Page, SharedState};
use crate::{
    dir, get_data, get_data_mut,
    popup::ChooseButton,
    save_data,
    scene::BGM_VOLUME_UPDATED,
    sync_data,
    tabs::{Tabs, TitleFn},
};
use anyhow::Result;
use bytesize::ByteSize;
use inputbox::InputBox;
use macroquad::prelude::*;
use once_cell::sync::Lazy;
use prpr::{
    config::Config,
    core::BOLD_FONT,
    ext::{open_url, poll_future, semi_white, LocalTask, RectExt, SafeTexture},
    scene::{request_file, request_input, return_file, return_input, show_error, show_message, take_file, take_input},
    task::Task,
    ui::{DRectButton, Dialog, Scroll, Slider, Ui, PREFER_REDUCED_MOTION, UI_SFX_VOLUME},
};
use prpr_l10n::{LanguageIdentifier, LANG_IDENTS, LANG_NAMES};
use reqwest::Url;
use serde::Deserialize;
use std::{
    borrow::Cow,
    fs, io,
    net::ToSocketAddrs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

const ITEM_HEIGHT: f32 = 0.15;
const INTERACT_WIDTH: f32 = 0.26;
const STATUS_PAGE: &str = "https://status.phira.cn";

struct NameList(String);
impl<'de> Deserialize<'de> for NameList {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = Vec::<String>::deserialize(deserializer)?;
        Ok(Self(s.join(", ")))
    }
}

#[derive(Deserialize)]
struct LocalizationListRaw {
    #[serde(rename = "en-US")]
    en_us: NameList,
    #[serde(rename = "fr-FR")]
    fr_fr: NameList,
    #[serde(rename = "de-DE")]
    de_de: NameList,
    #[serde(rename = "id-ID")]
    id_id: NameList,
    #[serde(rename = "ja-JP")]
    ja_jp: NameList,
    #[serde(rename = "ko-KR")]
    ko_kr: NameList,
    #[serde(rename = "pl-PL")]
    pl_pl: NameList,
    #[serde(rename = "pt-BR")]
    pt_br: NameList,
    #[serde(rename = "ru-RU")]
    ru_ru: NameList,
    #[serde(rename = "th-TH")]
    th_th: NameList,
    #[serde(rename = "zh-TW")]
    zh_tw: NameList,
    #[serde(rename = "tr-TR")]
    tr_tr: NameList,
    #[serde(rename = "vi-VN")]
    vi_vn: NameList,
}

struct LocalizationList(String);
impl<'de> Deserialize<'de> for LocalizationList {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = LocalizationListRaw::deserialize(deserializer)?;
        Ok(Self(format!(
            "\
English (en-US)\n{}\n
French (fr-FR)\n{}\n
German (de-DE)\n{}\n
Indonesian (id-ID)\n{}\n
Japanese (ja-JP)\n{}\n
Korean (ko-KR)\n{}\n
Polish (pl-PL)\n{}\n
Portuguese (pt-BR)\n{}\n
Russian (ru-RU)\n{}\n
Thai (th-TH)\n{}\n
Traditional Chinese (zh-TW)\n{}\n
Turkish (tr-TR)\n{}\n
Vietnamese (vi-VN)\n{}",
            raw.en_us.0,
            raw.fr_fr.0,
            raw.de_de.0,
            raw.id_id.0,
            raw.ja_jp.0,
            raw.ko_kr.0,
            raw.pl_pl.0,
            raw.pt_br.0,
            raw.ru_ru.0,
            raw.th_th.0,
            raw.zh_tw.0,
            raw.tr_tr.0,
            raw.vi_vn.0
        )))
    }
}

#[derive(Deserialize)]
struct StaffList {
    development: NameList,
    operations: NameList,
    documentation: NameList,
    art: NameList,
    music: NameList,
    audio: NameList,
    community: NameList,
    localization: LocalizationList,
}

static STAFF_LIST: Lazy<StaffList> = Lazy::new(|| {
    let data = include_str!("../../staff.yml");
    serde_yaml::from_str(data).unwrap()
});

#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingListType {
    General,
    Audio,
    Chart,
    Debug,
    About,
}

pub struct SettingsPage {
    list_general: GeneralList,
    list_audio: AudioList,
    list_chart: ChartList,
    list_debug: DebugList,

    tabs: Tabs<SettingListType>,

    scroll: Scroll,
    save_time: f32,

    icon: SafeTexture,
}

impl SettingsPage {
    const SAVE_TIME: f32 = 0.5;

    pub fn new(icon: SafeTexture, icon_lang: SafeTexture) -> Self {
        Self {
            list_general: GeneralList::new(icon_lang),
            list_audio: AudioList::new(),
            list_chart: ChartList::new(),
            list_debug: DebugList::new(),

            tabs: Tabs::new([
                (SettingListType::General, || tl!("general")),
                (SettingListType::Audio, || tl!("audio")),
                (SettingListType::Chart, || tl!("chart")),
                (SettingListType::Debug, || tl!("debug")),
                (SettingListType::About, || tl!("about")),
            ] as [(SettingListType, TitleFn); 5]),

            scroll: Scroll::new(),
            save_time: f32::INFINITY,

            icon,
        }
    }
}

impl Page for SettingsPage {
    fn label(&self) -> Cow<'static, str> {
        tl!("label")
    }

    fn exit(&mut self) -> Result<()> {
        BGM_VOLUME_UPDATED.store(true, Ordering::Relaxed);
        if self.save_time.is_finite() {
            save_data()?;
        }
        Ok(())
    }

    fn touch(&mut self, touch: &Touch, s: &mut SharedState) -> Result<bool> {
        let t = s.t;
        if match self.tabs.selected() {
            SettingListType::General => self.list_general.top_touch(touch, t),
            SettingListType::Audio => self.list_audio.top_touch(touch, t),
            SettingListType::Chart => self.list_chart.top_touch(touch, t),
            SettingListType::Debug => self.list_debug.top_touch(touch, t),
            SettingListType::About => false,
        } {
            return Ok(true);
        }

        if self.tabs.touch(touch, s.rt) {
            return Ok(true);
        }

        if self.scroll.touch(touch, t) {
            return Ok(true);
        }
        if let Some(p) = match self.tabs.selected() {
            SettingListType::General => self.list_general.touch(touch, t)?,
            SettingListType::Audio => self.list_audio.touch(touch, t)?,
            SettingListType::Chart => self.list_chart.touch(touch, t)?,
            SettingListType::Debug => self.list_debug.touch(touch, t)?,
            SettingListType::About => None,
        } {
            if p {
                self.save_time = t;
            }
            self.scroll.y_scroller.halt();
            return Ok(true);
        }
        Ok(false)
    }

    fn update(&mut self, s: &mut SharedState) -> Result<()> {
        let t = s.t;
        let changed = match self.tabs.selected() {
            SettingListType::General => self.list_general.update(t)?,
            SettingListType::Audio => self.list_audio.update(t)?,
            SettingListType::Chart => self.list_chart.update(t)?,
            SettingListType::Debug => self.list_debug.update(t)?,
            SettingListType::About => false,
        };
        self.scroll.update(t);
        if changed {
            self.save_time = t;
        }
        if t > self.save_time + Self::SAVE_TIME {
            save_data()?;
            self.save_time = f32::INFINITY;
        }
        Ok(())
    }

    fn render(&mut self, ui: &mut Ui, s: &mut SharedState) -> Result<()> {
        let t = s.t;
        let rt = s.rt;

        s.fader.render(ui, s.t, |ui| {
            let r = ui.content_rect();
            self.tabs.render(ui, rt, r, |ui, item| {
                let r = r.feather(-0.01);
                self.scroll.size((r.w, r.h));
                ui.scope(|ui| {
                    ui.dx(r.x);
                    ui.dy(r.y);
                    self.scroll.render(ui, |ui| match item {
                        SettingListType::General => self.list_general.render(ui, r, t),
                        SettingListType::Audio => self.list_audio.render(ui, r, t),
                        SettingListType::Chart => self.list_chart.render(ui, r, t),
                        SettingListType::Debug => self.list_debug.render(ui, r, t),
                        SettingListType::About => render_about(ui, r, &self.icon),
                    });
                });

                Ok(())
            })
        })?;

        Ok(())
    }

    fn next_page(&mut self) -> NextPage {
        if matches!(self.tabs.selected(), SettingListType::Audio) {
            return self.list_audio.next_page().unwrap_or_default();
        }
        if self.list_chart.open_history {
            self.list_chart.open_history = false;
            return NextPage::Overlay(Box::new(super::HistoryPage::new()));
        }
        NextPage::None
    }
}

fn render_about(ui: &mut Ui, mut r: Rect, icon: &SafeTexture) -> (f32, f32) {
    r.x = 0.;
    r.y = 0.;
    let ow = r.w;
    let r = r.feather(-0.02);

    let ct = r.center();
    let s = 0.1;
    let ir = Rect::new(ct.x - s, r.y + 0.05, s * 2., s * 2.);
    ui.fill_path(&ir.rounded(0.02), (**icon, ir));

    let staff = &*STAFF_LIST;
    let text = tl!(
        "about-content",
        "version" => format!("{} ({})", env!("CARGO_PKG_VERSION"), env!("GIT_HASH")),

        "development" => &staff.development.0,
        "operations" => &staff.operations.0,
        "documentation" => &staff.documentation.0,
        "art" => &staff.art.0,
        "music" => &staff.music.0,
        "audio" => &staff.audio.0,
        "community" => &staff.community.0,
        "localization" => &staff.localization.0
    );
    let (first, text) = text.split_once('\n').unwrap();
    let tr = ui
        .text(first)
        .pos(ct.x, ir.bottom() + 0.03)
        .anchor(0.5, 0.)
        .size(0.6)
        .draw_using(&BOLD_FONT);

    let r = ui
        .text(text.trim())
        .pos(r.x, tr.bottom() + 0.06)
        .size(0.55)
        .multiline()
        .max_width(r.w)
        .h_center()
        .draw();

    (ow, r.bottom() + 0.03)
}

fn render_title<'a>(ui: &mut Ui, title: impl Into<Cow<'a, str>>, subtitle: Option<Cow<'a, str>>) -> f32 {
    const TITLE_SIZE: f32 = 0.6;
    const SUBTITLE_SIZE: f32 = 0.35;
    const LEFT: f32 = 0.06;
    const PAD: f32 = 0.01;
    const SUB_MAX_WIDTH: f32 = 1.4;
    if let Some(subtitle) = subtitle {
        let title = title.into();
        let r1 = ui.text(Cow::clone(&title)).size(TITLE_SIZE).measure();
        let r2 = ui
            .text(Cow::clone(&subtitle))
            .size(SUBTITLE_SIZE)
            .max_width(SUB_MAX_WIDTH)
            .no_baseline()
            .measure();
        let h = r1.h + PAD + r2.h;
        let r1 = ui
            .text(subtitle)
            .pos(LEFT, (ITEM_HEIGHT + h) / 2.)
            .anchor(0., 1.)
            .size(SUBTITLE_SIZE)
            .max_width(SUB_MAX_WIDTH)
            .color(semi_white(0.6))
            .draw()
            .right();
        let r2 = ui
            .text(title)
            .pos(LEFT, (ITEM_HEIGHT - h) / 2.)
            .no_baseline()
            .size(TITLE_SIZE)
            .draw()
            .right();
        r1.max(r2)
    } else {
        ui.text(title.into())
            .pos(LEFT, ITEM_HEIGHT / 2.)
            .anchor(0., 0.5)
            .no_baseline()
            .size(TITLE_SIZE)
            .draw()
            .right()
    }
}

#[inline]
fn render_switch(ui: &mut Ui, r: Rect, t: f32, btn: &mut DRectButton, on: bool) {
    btn.render_text(ui, r, t, if on { ttl!("switch-on") } else { ttl!("switch-off") }, 0.5, on);
}

#[inline]
fn right_rect(w: f32) -> Rect {
    let rh = ITEM_HEIGHT * 2. / 3.;
    Rect::new(w - 0.3, (ITEM_HEIGHT - rh) / 2., INTERACT_WIDTH, rh)
}

struct GeneralList {
    icon_lang: SafeTexture,

    lang_btn: ChooseButton,

    #[cfg(all(any(target_os = "windows", target_os = "linux"), not(target_env = "ohos")))]
    fullscreen_btn: DRectButton,

    cache_btn: DRectButton,
    offline_btn: DRectButton,
    server_status_btn: DRectButton,
    mp_btn: DRectButton,
    mp_addr_btn: DRectButton,
    #[cfg(not(target_env = "ohos"))]
    lowq_btn: DRectButton,
    prefer_reduced_motion_btn: DRectButton,
    insecure_btn: DRectButton,
    enable_anys_btn: DRectButton,
    anys_gateway_btn: DRectButton,

    // 结算画面：手动指定 rks / 挑战徽章数字与颜色
    rks_btn: DRectButton,
    challenge_rank_btn: DRectButton,
    challenge_color_btn: DRectButton,

    // 外观：自定义字体 / 主页立绘 / 恢复默认
    font_btn: DRectButton,
    font_reset_btn: DRectButton,
    char_btn: DRectButton,
    char_reset_btn: DRectButton,
    reset_btn: DRectButton,
    has_custom_font: bool,
    has_custom_char: bool,
    /// 「恢复默认设置」的确认框结果
    reset_confirm: Option<Arc<AtomicBool>>,

    cache_size: Option<u64>,
    cache_task: Option<Task<Result<u64>>>,
}

/// 自定义字体存放位置（启动时优先加载）
fn custom_font_path() -> Result<String> {
    Ok(format!("{}/font.ttf", dir::root()?))
}

/// 自定义主页立绘存放位置
fn custom_char_path() -> Result<String> {
    Ok(format!("{}/char.img", dir::root()?))
}

impl GeneralList {
    pub fn new(icon_lang: SafeTexture) -> Self {
        let mut this = Self {
            icon_lang,

            lang_btn: ChooseButton::new()
                .with_options(LANG_NAMES.iter().map(|s| s.to_string()).collect())
                .with_selected(
                    get_data()
                        .language
                        .as_ref()
                        .and_then(|it| it.parse::<LanguageIdentifier>().ok())
                        .and_then(|ident| LANG_IDENTS.iter().position(|it| *it == ident))
                        .unwrap_or_default(),
                ),

            #[cfg(all(any(target_os = "windows", target_os = "linux"), not(target_env = "ohos")))]
            fullscreen_btn: DRectButton::new(),

            cache_btn: DRectButton::new(),
            offline_btn: DRectButton::new(),
            server_status_btn: DRectButton::new(),
            mp_btn: DRectButton::new(),
            mp_addr_btn: DRectButton::new(),
            #[cfg(not(target_env = "ohos"))]
            lowq_btn: DRectButton::new(),
            prefer_reduced_motion_btn: DRectButton::new(),
            insecure_btn: DRectButton::new(),
            enable_anys_btn: DRectButton::new(),
            anys_gateway_btn: DRectButton::new(),

            rks_btn: DRectButton::new(),
            challenge_rank_btn: DRectButton::new(),
            challenge_color_btn: DRectButton::new(),

            font_btn: DRectButton::new(),
            font_reset_btn: DRectButton::new(),
            char_btn: DRectButton::new(),
            char_reset_btn: DRectButton::new(),
            reset_btn: DRectButton::new(),
            has_custom_font: custom_font_path().map(|it| std::path::Path::new(&it).is_file()).unwrap_or(false),
            has_custom_char: custom_char_path().map(|it| std::path::Path::new(&it).is_file()).unwrap_or(false),
            reset_confirm: None,

            cache_size: None,
            cache_task: None,
        };
        let _ = this.update_cache_size();
        this
    }

    pub fn top_touch(&mut self, touch: &Touch, t: f32) -> bool {
        if self.lang_btn.top_touch(touch, t) {
            return true;
        }
        false
    }

    fn dir_size(path: impl Into<PathBuf>) -> io::Result<u64> {
        fn inner(mut dir: fs::ReadDir) -> io::Result<u64> {
            dir.try_fold(0, |acc, file| {
                let file = file?;
                let size = match file.metadata()? {
                    data if data.is_dir() => inner(fs::read_dir(file.path())?)?,
                    data => data.len(),
                };
                Ok(acc + size)
            })
        }

        inner(fs::read_dir(path.into())?)
    }

    fn update_cache_size(&mut self) -> Result<()> {
        self.cache_size = None;

        let cache_dir = dir::cache()?;
        self.cache_task = Some(Task::new(async { Ok(Self::dir_size(cache_dir)?) }));
        Ok(())
    }

    pub fn touch(&mut self, touch: &Touch, t: f32) -> Result<Option<bool>> {
        let data = get_data_mut();
        let config = &mut data.config;
        if self.lang_btn.touch(touch, t) {
            return Ok(Some(false));
        }

        #[cfg(all(any(target_os = "windows", target_os = "linux"), not(target_env = "ohos")))]
        if self.fullscreen_btn.touch(touch, t) {
            config.fullscreen_mode ^= true;

            macroquad::window::set_fullscreen(config.fullscreen_mode);

            return Ok(Some(true));
        }

        if self.cache_btn.touch(touch, t) {
            fs::remove_dir_all(dir::cache()?)?;
            self.update_cache_size()?;
            show_message(tl!("item-cache-cleared")).ok();
            return Ok(Some(false));
        }
        if self.offline_btn.touch(touch, t) {
            config.offline_mode ^= true;
            return Ok(Some(true));
        }
        if self.server_status_btn.touch(touch, t) {
            let _ = open_url(STATUS_PAGE);
            return Ok(Some(true));
        }
        if self.mp_btn.touch(touch, t) {
            config.mp_enabled ^= true;
            return Ok(Some(true));
        }
        if self.mp_addr_btn.touch(touch, t) {
            request_input("mp_addr", InputBox::new().default_text(&config.mp_address));
            return Ok(Some(true));
        }
        #[cfg(not(target_env = "ohos"))]
        if self.lowq_btn.touch(touch, t) {
            config.sample_count = if config.sample_count == 1 { 2 } else { 1 };
            return Ok(Some(true));
        }
        if self.prefer_reduced_motion_btn.touch(touch, t) {
            data.prefer_reduced_motion ^= true;
            PREFER_REDUCED_MOTION.store(data.prefer_reduced_motion, Ordering::Relaxed);
            return Ok(Some(true));
        }
        if self.insecure_btn.touch(touch, t) {
            data.accept_invalid_cert ^= true;
            return Ok(Some(true));
        }
        if self.enable_anys_btn.touch(touch, t) {
            data.enable_anys ^= true;
            return Ok(Some(true));
        }
        if self.anys_gateway_btn.touch(touch, t) {
            request_input("anys_gateway", InputBox::new().default_text(&data.anys_gateway));
            return Ok(Some(true));
        }
        if self.rks_btn.touch(touch, t) {
            request_input("rks", InputBox::new().default_text(&config.rks_override.map(|it| format!("{it}")).unwrap_or_default()));
            return Ok(Some(true));
        }
        if self.challenge_rank_btn.touch(touch, t) {
            request_input("challenge_rank", InputBox::new().default_text(&config.challenge_rank.to_string()));
            return Ok(Some(true));
        }
        if self.challenge_color_btn.touch(touch, t) {
            config.challenge_color = config.challenge_color.next();
            return Ok(Some(true));
        }
        if self.font_btn.touch(touch, t) {
            request_file("ui_font");
            return Ok(Some(false));
        }
        if self.font_reset_btn.touch(touch, t) {
            if let Ok(path) = custom_font_path() {
                let _ = fs::remove_file(path);
            }
            self.has_custom_font = false;
            show_message(tl!("font-reset-done")).ok();
            return Ok(Some(false));
        }
        if self.char_btn.touch(touch, t) {
            request_file("home_char");
            return Ok(Some(false));
        }
        if self.char_reset_btn.touch(touch, t) {
            if let Ok(path) = custom_char_path() {
                let _ = fs::remove_file(path);
            }
            self.has_custom_char = false;
            show_message(tl!("char-reset-done")).ok();
            return Ok(Some(false));
        }
        if self.reset_btn.touch(touch, t) {
            let res = Arc::new(AtomicBool::new(false));
            // 确认后由 update() 轮询处理
            crate::scene::confirm_dialog(tl!("item-reset"), tl!("item-reset-confirm"), res.clone());
            self.reset_confirm = Some(res);
            return Ok(Some(false));
        }
        Ok(None)
    }

    pub fn update(&mut self, t: f32) -> Result<bool> {
        self.lang_btn.update(t);
        let data = get_data_mut();
        if self.lang_btn.changed() {
            data.language = Some(LANG_IDENTS[self.lang_btn.selected()].to_string());
            sync_data();
            return Ok(true);
        }
        // 「恢复默认设置」确认框
        if let Some(res) = &self.reset_confirm {
            if res.load(Ordering::SeqCst) {
                self.reset_confirm = None;
                data.config = Config::default();
                data.config.init();
                // 运行时镜像量也要跟着刷新，否则音量 / 全屏要等到再动一次滑块才生效
                UI_SFX_VOLUME.store(data.config.volume_sfx.to_bits(), Ordering::Relaxed);
                BGM_VOLUME_UPDATED.store(true, Ordering::Relaxed);
                show_message(tl!("reset-done")).ok();
                return Ok(true);
            }
        }
        // 导入的字体 / 主页立绘（iOS 选中的文件在临时目录，必须复制到数据目录才能长期使用）
        if let Some((id, file)) = take_file() {
            match id.as_str() {
                "ui_font" => {
                    match custom_font_path().and_then(|dst| -> Result<()> {
                        fs::copy(&file, dst)?;
                        Ok(())
                    }) {
                        Ok(_) => {
                            self.has_custom_font = true;
                            show_message(tl!("font-imported")).ok();
                        }
                        Err(err) => show_error(err.context(tl!("font-import-failed"))),
                    }
                    return Ok(false);
                }
                "home_char" => {
                    match custom_char_path().and_then(|dst| -> Result<()> {
                        fs::copy(&file, dst)?;
                        Ok(())
                    }) {
                        Ok(_) => {
                            self.has_custom_char = true;
                            show_message(tl!("char-imported")).ok();
                        }
                        Err(err) => show_error(err.context(tl!("char-import-failed"))),
                    }
                    return Ok(false);
                }
                _ => return_file(id, file),
            }
        }
        if let Some((id, text)) = take_input() {
            if id == "mp_addr" {
                if let Err(err) = text.to_socket_addrs() {
                    show_error(anyhow::Error::new(err).context(tl!("item-mp-addr-invalid")));
                    return Ok(false);
                } else {
                    data.config.mp_address = text;
                    return Ok(true);
                }
            } else if id == "anys_gateway" {
                if let Err(err) = Url::parse(&text) {
                    show_error(anyhow::Error::new(err).context(tl!("item-anys-gateway-invalid")));
                    return Ok(false);
                } else {
                    data.anys_gateway = text.trim_end_matches('/').to_string();
                    return Ok(true);
                }
            } else if id == "rks" {
                let text = text.trim();
                // 留空 = 跟随账号真实 rks
                if text.is_empty() {
                    data.config.rks_override = None;
                    return Ok(true);
                }
                match text.parse::<f32>() {
                    Ok(v) if (0. ..=100.).contains(&v) => {
                        data.config.rks_override = Some(v);
                        return Ok(true);
                    }
                    _ => {
                        show_error(anyhow::anyhow!("{}", tl!("rks-invalid")));
                        return Ok(false);
                    }
                }
            } else if id == "health_max" {
                match text.trim().parse::<f32>() {
                    Ok(v) if (10. ..=10000.).contains(&v) => {
                        data.config.health_max = v;
                        return Ok(true);
                    }
                    _ => {
                        show_error(anyhow::anyhow!("{}", tl!("health-max-invalid")));
                        return Ok(false);
                    }
                }
            } else if id == "combo_text" {
                // 留空 = 默认（Autoplay 时 AUTOPLAY，否则 COMBO）
                data.config.combo_text = text.trim().chars().take(Config::COMBO_TEXT_MAX_CHARS).collect();
                return Ok(true);
            } else if id == "challenge_rank" {
                match text.trim().parse::<u32>() {
                    Ok(v) if v <= 999 => {
                        data.config.challenge_rank = v;
                        return Ok(true);
                    }
                    _ => {
                        show_error(anyhow::anyhow!("{}", tl!("challenge-rank-invalid")));
                        return Ok(false);
                    }
                }
            } else {
                return_input(id, text);
            }
        }
        if let Some(task) = &mut self.cache_task {
            if let Some(size) = task.take() {
                self.cache_size = size.ok();
                self.cache_task = None;
            }
        }
        Ok(false)
    }

    pub fn render(&mut self, ui: &mut Ui, r: Rect, t: f32) -> (f32, f32) {
        let w = r.w;
        let mut h = 0.;
        macro_rules! item {
            ($($b:tt)*) => {{
                $($b)*
                ui.dy(ITEM_HEIGHT);
                h += ITEM_HEIGHT;
            }}
        }
        let rr = right_rect(w);

        let data = get_data();
        let config = &data.config;
        item! {
            let rt = render_title(ui, tl!("item-lang"), None);
            let w = 0.06;
            let r = Rect::new(rt + 0.01, (ITEM_HEIGHT - w) / 2., w, w);
            ui.fill_rect(r, (*self.icon_lang, r));
            self.lang_btn.render(ui, rr, t);
        }

        #[cfg(all(any(target_os = "windows", target_os = "linux"), not(target_env = "ohos")))]
        item! {
            render_title(ui, tl!("item-fullscreen"), None);
            render_switch(ui, rr, t, &mut self.fullscreen_btn, config.fullscreen_mode);
        }

        item! {
            render_title(ui, tl!("item-offline"), Some(tl!("item-offline-sub")));
            render_switch(ui, rr, t, &mut self.offline_btn, config.offline_mode);
        }
        item! {
            render_title(ui, tl!("item-server-status"), Some(tl!("item-server-status-sub")));
            self.server_status_btn.render_text(ui, rr, t, tl!("check-status"), 0.5, true);
        }
        item! {
            render_title(ui, tl!("item-mp"), Some(tl!("item-mp-sub")));
            render_switch(ui, rr, t, &mut self.mp_btn, config.mp_enabled);
        }
        item! {
            render_title(ui, tl!("item-mp-addr"), Some(tl!("item-mp-addr-sub")));
            self.mp_addr_btn.render_text(ui, rr, t, &config.mp_address, 0.4, false);
        }
        item! {
            render_title(ui, tl!("item-prefer-reduced-motion"), Some(tl!("item-prefer-reduced-motion-sub")));
            render_switch(ui, rr, t, &mut self.prefer_reduced_motion_btn, data.prefer_reduced_motion);
        }
        #[cfg(not(target_env = "ohos"))]
        item! {
            render_title(ui, tl!("item-lowq"), Some(tl!("item-lowq-sub")));
            render_switch(ui, rr, t, &mut self.lowq_btn, config.sample_count == 1);
        }
        item! {
            let cache_size = if let Some(size) = self.cache_size {
                Cow::Owned(tl!("item-cache-size", "size" => ByteSize(size).to_string()))
            } else {
                tl!("item-cache-size-loading")
            };
            render_title(ui, tl!("item-clear-cache"), Some(cache_size));
            self.cache_btn.render_text(ui, rr, t, tl!("item-clear-cache-btn"), 0.5, true);
        }
        ui.dy(0.04);
        h += 0.04;
        item! {
            render_title(ui, tl!("item-insecure"), Some(tl!("item-insecure-sub")));
            render_switch(ui, rr, t, &mut self.insecure_btn, data.accept_invalid_cert);
        }
        item! {
            render_title(ui, tl!("item-enable-anys"), Some(tl!("item-enable-anys-sub")));
            render_switch(ui, rr, t, &mut self.enable_anys_btn, data.enable_anys);
        }
        item! {
            render_title(ui, tl!("item-anys-gateway"), Some(tl!("item-anys-gateway-sub")));
            self.anys_gateway_btn.render_text(ui, rr, t, &data.anys_gateway, 0.4, false);
        }
        ui.dy(0.04);
        h += 0.04;
        // 结算画面相关：rks / 挑战徽章
        item! {
            let rks_text = match config.rks_override {
                Some(rks) => format!("{rks:.2}"),
                None => tl!("rks-follow").to_string(),
            };
            render_title(ui, tl!("item-rks"), Some(tl!("item-rks-sub")));
            self.rks_btn.render_text(ui, rr, t, rks_text, 0.5, false);
        }
        item! {
            render_title(ui, tl!("item-challenge-rank"), Some(tl!("item-challenge-rank-sub")));
            self.challenge_rank_btn
                .render_text(ui, rr, t, config.challenge_rank.to_string(), 0.5, false);
        }
        item! {
            render_title(ui, tl!("item-challenge-color"), Some(tl!("item-challenge-color-sub")));
            let color_text = format!("{:?}", config.challenge_color);
            self.challenge_color_btn.render_text(ui, rr, t, color_text, 0.5, false);
        }
        ui.dy(0.04);
        h += 0.04;
        // 外观：自定义字体 / 主页立绘 / 恢复默认
        item! {
            render_title(ui, tl!("item-font"), Some(tl!("item-font-sub")));
            let text = if self.has_custom_font {
                tl!("imported").to_string()
            } else {
                tl!("import-font").to_string()
            };
            self.font_btn.render_text(ui, rr, t, text, 0.5, false);
        }
        item! {
            render_title(ui, tl!("item-font-reset"), Some(tl!("item-font-reset-sub")));
            self.font_reset_btn.render_text(ui, rr, t, tl!("reset"), 0.5, true);
        }
        item! {
            render_title(ui, tl!("item-home-char"), Some(tl!("item-home-char-sub")));
            let text = if self.has_custom_char {
                tl!("imported").to_string()
            } else {
                tl!("import-image").to_string()
            };
            self.char_btn.render_text(ui, rr, t, text, 0.5, false);
        }
        item! {
            render_title(ui, tl!("item-home-char-reset"), Some(tl!("item-home-char-reset-sub")));
            self.char_reset_btn.render_text(ui, rr, t, tl!("reset"), 0.5, true);
        }
        ui.dy(0.04);
        h += 0.04;
        item! {
            render_title(ui, tl!("item-reset"), Some(tl!("item-reset-sub")));
            self.reset_btn.render_text(ui, rr, t, tl!("reset"), 0.5, true);
        }
        self.lang_btn.render_top(ui, t, 1.);
        (w, h)
    }
}

struct AudioList {
    adjust_btn: DRectButton,
    music_slider: Slider,
    sfx_slider: Slider,
    bgm_slider: Slider,
    cali_btn: DRectButton,
    #[cfg(not(target_os = "android"))]
    preferred_sample_rate_btn: DRectButton,
    #[cfg(target_env = "ohos")]
    audio_buffer_size_btn: DRectButton,
    cali_task: LocalTask<Result<OffsetPage>>,
    next_page: Option<NextPage>,
}

impl AudioList {
    pub fn new() -> Self {
        Self {
            adjust_btn: DRectButton::new(),
            music_slider: Slider::new(0.0..2.0, 0.05),
            sfx_slider: Slider::new(0.0..2.0, 0.05),
            bgm_slider: Slider::new(0.0..2.0, 0.05),
            cali_btn: DRectButton::new(),
            #[cfg(not(target_os = "android"))]
            preferred_sample_rate_btn: DRectButton::new(),
            #[cfg(target_env = "ohos")]
            audio_buffer_size_btn: DRectButton::new(),

            cali_task: None,
            next_page: None,
        }
    }

    pub fn top_touch(&mut self, _touch: &Touch, _t: f32) -> bool {
        false
    }

    pub fn touch(&mut self, touch: &Touch, t: f32) -> Result<Option<bool>> {
        let data = get_data_mut();
        let config = &mut data.config;
        if self.adjust_btn.touch(touch, t) {
            config.adjust_time ^= true;
            return Ok(Some(true));
        }
        if let wt @ Some(_) = self.music_slider.touch(touch, t, &mut config.volume_music) {
            return Ok(wt);
        }
        if let wt @ Some(_) = self.sfx_slider.touch(touch, t, &mut config.volume_sfx) {
            UI_SFX_VOLUME.store(config.volume_sfx.to_bits(), Ordering::Relaxed);
            return Ok(wt);
        }
        let old = config.volume_bgm;
        if let wt @ Some(_) = self.bgm_slider.touch(touch, t, &mut config.volume_bgm) {
            if (config.volume_bgm - old).abs() > 0.001 {
                BGM_VOLUME_UPDATED.store(true, Ordering::Relaxed);
            }
            return Ok(wt);
        }
        if self.cali_btn.touch(touch, t) {
            self.cali_task = Some(Box::pin(OffsetPage::new()));
            return Ok(Some(false));
        }
        #[cfg(not(target_os = "android"))]
        if self.preferred_sample_rate_btn.touch(touch, t) {
            let options = [None, Some(44100), Some(48000), Some(88200), Some(96000), Some(192000)];
            let current = config.preferred_sample_rate;
            let selected = options.iter().position(|&r| r == current).unwrap_or(0);
            config.preferred_sample_rate = options[(selected + 1) % options.len()];
            return Ok(Some(true));
        }
        #[cfg(target_env = "ohos")]
        if self.audio_buffer_size_btn.touch(touch, t) {
            let options = [128u32, 256u32, 512u32];
            let current = config.audio_buffer_size.unwrap_or(256);
            let selected = options.iter().position(|&r| r == current).unwrap_or(1);
            config.audio_buffer_size = Some(options[(selected + 1) % options.len()]);
            return Ok(Some(true));
        }
        Ok(None)
    }

    pub fn update(&mut self, _t: f32) -> Result<bool> {
        if let Some(task) = &mut self.cali_task {
            if let Some(res) = poll_future(task.as_mut()) {
                match res {
                    Err(err) => show_error(err.context(tl!("load-cali-failed"))),
                    Ok(page) => {
                        self.next_page = Some(NextPage::Overlay(Box::new(page)));
                    }
                }
                self.cali_task = None;
            }
        }
        Ok(false)
    }

    pub fn render(&mut self, ui: &mut Ui, r: Rect, t: f32) -> (f32, f32) {
        let w = r.w;
        let mut h = 0.;
        macro_rules! item {
            ($($b:tt)*) => {{
                $($b)*
                ui.dy(ITEM_HEIGHT);
                h += ITEM_HEIGHT;
            }}
        }
        let rr = right_rect(w);

        let data = get_data();
        let config = &data.config;
        item! {
            render_title(ui, tl!("item-adjust"), Some(tl!("item-adjust-sub")));
            render_switch(ui, rr, t, &mut self.adjust_btn, config.adjust_time);
        }
        item! {
            render_title(ui, tl!("item-music"), None);
            self.music_slider.render(ui, rr, t, config.volume_music, format!("{:.2}", config.volume_music));
        }
        item! {
            render_title(ui, tl!("item-sfx"), None);
            self.sfx_slider.render(ui, rr, t, config.volume_sfx, format!("{:.2}", config.volume_sfx));
        }
        item! {
            render_title(ui, tl!("item-bgm"), None);
            self.bgm_slider.render(ui, rr, t, config.volume_bgm, format!("{:.2}", config.volume_bgm));
        }
        item! {
            render_title(ui, tl!("item-cali"), None);
            self.cali_btn.render_text(ui, rr, t, format!("{:.0}ms", config.offset * 1000.), 0.5, true);
        }
        #[cfg(not(target_os = "android"))]
        item! {
            render_title(ui, tl!("item-preferred-sample-rate"), None);
            let text = if let Some(rate) = config.preferred_sample_rate {
                format!("{} Hz", rate)
            } else {
                tl!("preferred-sample-rate-default").to_string()
            };
            self.preferred_sample_rate_btn.render_text(ui, rr, t, text, 0.5, false);
        }
        #[cfg(target_env = "ohos")]
        item! {
            render_title(ui, tl!("item-audio-buffer-size"), None);
            let buf_size = config.audio_buffer_size.unwrap_or(256);
            self.audio_buffer_size_btn.render_text(ui, rr, t, format!("{}", buf_size), 0.5, false);
        }
        (w, h)
    }

    pub fn next_page(&mut self) -> Option<NextPage> {
        self.next_page.take()
    }
}

struct ChartList {
    show_acc_btn: DRectButton,
    ap_fc_indicator_btn: DRectButton,
    show_avg_fps_btn: DRectButton,
    dc_pause_btn: DRectButton,
    dhint_btn: DRectButton,
    opt_btn: DRectButton,
    use_keyboard_btn: DRectButton,
    drag_protect_btn: DRectButton,
    flick_protect_btn: DRectButton,
    speed_slider: Slider,
    size_slider: Slider,
    judge_slider: Slider,
    judge_good_slider: Slider,
    judge_bad_slider: Slider,
    combo_text_btn: DRectButton,
    line_ref_btn: DRectButton,
    late_leniency_slider: Slider,
    debug_line_btn: DRectButton,
    debug_note_btn: DRectButton,
    pause_offset_btn: DRectButton,
    early_late_btn: DRectButton,
    judge_chart_btn: DRectButton,
    health_max_btn: DRectButton,
    health_scale_slider: Slider,
    health_color_btn: DRectButton,
    health_len_slider: Slider,
    motion_btn: DRectButton,
    motion_threshold_slider: Slider,
    motion_still_slider: Slider,

    // ---- 打击表现 ----
    in_game_font_btn: DRectButton,
    judge_text_btn: DRectButton,
    judge_text_style_btn: DRectButton,
    judge_text_size_slider: Slider,
    miss_marker_btn: DRectButton,
    miss_marker_time_slider: Slider,
    note_trail_btn: DRectButton,
    note_trail_len_slider: Slider,
    note_trail_alpha_slider: Slider,
    note_trail_count_slider: Slider,
    note_trail_click_btn: DRectButton,
    note_trail_drag_btn: DRectButton,
    note_trail_flick_btn: DRectButton,
    line_afterimage_btn: DRectButton,
    line_afterimage_count_slider: Slider,
    line_afterimage_alpha_slider: Slider,
    music_spectrum_btn: DRectButton,
    music_spectrum_gain_slider: Slider,
    line_glow_btn: DRectButton,
    line_glow_strength_slider: Slider,

    // ---- 实时 HUD ----
    hud_btn: DRectButton,
    hud_acc_btn: DRectButton,
    hud_counts_btn: DRectButton,
    hud_max_combo_btn: DRectButton,
    hud_time_btn: DRectButton,
    hud_delta_btn: DRectButton,
    hud_rks_btn: DRectButton,
    hud_fps_btn: DRectButton,
    hud_corner_btn: DRectButton,
    hud_size_slider: Slider,
    hud_alpha_slider: Slider,
    hud_outline_btn: DRectButton,

    /// 上传成绩开关 + 查看协议
    upload_btn: DRectButton,
    upload_consent_btn: DRectButton,
    /// 一键切换成「可上传成绩」的配置
    upload_profile_btn: DRectButton,
    /// 打开「成绩历史」页
    history_btn: DRectButton,
    open_history: bool,
}

impl ChartList {
    pub fn new() -> Self {
        Self {
            show_acc_btn: DRectButton::new(),
            ap_fc_indicator_btn: DRectButton::new(),
            show_avg_fps_btn: DRectButton::new(),
            dc_pause_btn: DRectButton::new(),
            dhint_btn: DRectButton::new(),
            opt_btn: DRectButton::new(),
            use_keyboard_btn: DRectButton::new(),
            drag_protect_btn: DRectButton::new(),
            flick_protect_btn: DRectButton::new(),
            speed_slider: Slider::new(0.5..2., 0.05),
            size_slider: Slider::new(0.8..1.2, 0.005),
            judge_slider: Slider::new(Config::JUDGE_WINDOW_MIN..Config::JUDGE_WINDOW_MAX, 5.),
            judge_good_slider: Slider::new(Config::JUDGE_WINDOW_MIN..Config::GOOD_WINDOW_MAX, 5.),
            judge_bad_slider: Slider::new(Config::JUDGE_WINDOW_MIN..Config::BAD_WINDOW_MAX, 5.),
            combo_text_btn: DRectButton::new(),
            line_ref_btn: DRectButton::new(),
            late_leniency_slider: Slider::new(0. ..Config::LATE_LENIENCY_MAX + 1., 5.),
            debug_line_btn: DRectButton::new(),
            debug_note_btn: DRectButton::new(),
            pause_offset_btn: DRectButton::new(),
            early_late_btn: DRectButton::new(),
            judge_chart_btn: DRectButton::new(),
            health_max_btn: DRectButton::new(),
            health_scale_slider: Slider::new(0.1..5., 0.1),
            health_color_btn: DRectButton::new(),
            health_len_slider: Slider::new(Config::HEALTH_BAR_LEN_MIN..Config::HEALTH_BAR_LEN_MAX, 0.05),
            motion_btn: DRectButton::new(),
            motion_threshold_slider: Slider::new(Config::MOTION_THRESHOLD_MIN..Config::MOTION_THRESHOLD_MAX, 0.01),
            motion_still_slider: Slider::new(Config::MOTION_STILL_MIN..Config::MOTION_STILL_MAX, 0.1),

            in_game_font_btn: DRectButton::new(),
            judge_text_btn: DRectButton::new(),
            judge_text_style_btn: DRectButton::new(),
            judge_text_size_slider: Slider::new(0.2..1.5, 0.02),
            miss_marker_btn: DRectButton::new(),
            miss_marker_time_slider: Slider::new(0.2..10., 0.1),
            note_trail_btn: DRectButton::new(),
            note_trail_len_slider: Slider::new(0.05..1., 0.05),
            note_trail_alpha_slider: Slider::new(0.05..1., 0.05),
            note_trail_count_slider: Slider::new(1. ..256., 1.),
            note_trail_click_btn: DRectButton::new(),
            note_trail_drag_btn: DRectButton::new(),
            note_trail_flick_btn: DRectButton::new(),
            line_afterimage_btn: DRectButton::new(),
            line_afterimage_count_slider: Slider::new(1. ..256., 1.),
            line_afterimage_alpha_slider: Slider::new(0.05..1., 0.05),
            music_spectrum_btn: DRectButton::new(),
            music_spectrum_gain_slider: Slider::new(0.2..4., 0.1),
            line_glow_btn: DRectButton::new(),
            line_glow_strength_slider: Slider::new(0.05..1., 0.05),

            hud_btn: DRectButton::new(),
            hud_acc_btn: DRectButton::new(),
            hud_counts_btn: DRectButton::new(),
            hud_max_combo_btn: DRectButton::new(),
            hud_time_btn: DRectButton::new(),
            hud_delta_btn: DRectButton::new(),
            hud_rks_btn: DRectButton::new(),
            hud_fps_btn: DRectButton::new(),
            hud_corner_btn: DRectButton::new(),
            hud_size_slider: Slider::new(0.5..2., 0.05),
            hud_alpha_slider: Slider::new(0.1..1., 0.05),
            hud_outline_btn: DRectButton::new(),

            upload_btn: DRectButton::new(),
            upload_consent_btn: DRectButton::new(),
            upload_profile_btn: DRectButton::new(),
            history_btn: DRectButton::new(),
            open_history: false,
        }
    }

    pub fn top_touch(&mut self, _touch: &Touch, _t: f32) -> bool {
        false
    }

    pub fn touch(&mut self, touch: &Touch, t: f32) -> Result<Option<bool>> {
        let data = get_data_mut();
        let config = &mut data.config;
        if self.show_acc_btn.touch(touch, t) {
            config.show_acc ^= true;
            return Ok(Some(true));
        }
        if self.ap_fc_indicator_btn.touch(touch, t) {
            config.ap_fc_indicator ^= true;
            return Ok(Some(true));
        }
        if self.show_avg_fps_btn.touch(touch, t) {
            config.show_avg_fps ^= true;
            return Ok(Some(true));
        }
        if self.dc_pause_btn.touch(touch, t) {
            config.double_click_to_pause ^= true;
            return Ok(Some(true));
        }
        if self.dhint_btn.touch(touch, t) {
            config.double_hint ^= true;
            return Ok(Some(true));
        }
        if self.opt_btn.touch(touch, t) {
            config.aggressive ^= true;
            return Ok(Some(true));
        }
        if self.use_keyboard_btn.touch(touch, t) {
            config.use_keyboard ^= true;
            return Ok(Some(true));
        }
        if self.drag_protect_btn.touch(touch, t) {
            config.drag_protect ^= true;
            return Ok(Some(true));
        }
        if self.flick_protect_btn.touch(touch, t) {
            config.flick_protect ^= true;
            return Ok(Some(true));
        }
        if let wt @ Some(_) = self.speed_slider.touch(touch, t, &mut config.speed) {
            return Ok(wt);
        }
        if let wt @ Some(_) = self.size_slider.touch(touch, t, &mut config.note_scale) {
            return Ok(wt);
        }
        if let wt @ Some(_) = self.judge_slider.touch(touch, t, &mut config.judge_window) {
            config.judge_window = config.judge_window.clamp(Config::JUDGE_WINDOW_MIN, Config::JUDGE_WINDOW_MAX);
            return Ok(wt);
        }
        {
            // 拖动即改为「单独指定」，不再跟随 Perfect 的比例
            let mut v = config.judge_window_good_ms();
            if let wt @ Some(_) = self.judge_good_slider.touch(touch, t, &mut v) {
                config.judge_window_good = Some(v.clamp(Config::JUDGE_WINDOW_MIN, Config::GOOD_WINDOW_MAX));
                return Ok(wt);
            }
        }
        {
            let mut v = config.judge_window_bad_ms();
            if let wt @ Some(_) = self.judge_bad_slider.touch(touch, t, &mut v) {
                config.judge_window_bad = Some(v.clamp(Config::JUDGE_WINDOW_MIN, Config::BAD_WINDOW_MAX));
                return Ok(wt);
            }
        }
        if self.combo_text_btn.touch(touch, t) {
            request_input("combo_text", InputBox::new().default_text(&config.combo_text));
            return Ok(Some(true));
        }
        if self.line_ref_btn.touch(touch, t) {
            config.line_ref_y_axis ^= true;
            return Ok(Some(true));
        }
        if let wt @ Some(_) = self.late_leniency_slider.touch(touch, t, &mut config.late_leniency_ms) {
            config.late_leniency_ms = config.late_leniency_ms.clamp(0., Config::LATE_LENIENCY_MAX);
            return Ok(wt);
        }
        if self.debug_line_btn.touch(touch, t) {
            config.chart_debug_line ^= true;
            return Ok(Some(true));
        }
        if self.debug_note_btn.touch(touch, t) {
            config.chart_debug_note ^= true;
            return Ok(Some(true));
        }
        if self.pause_offset_btn.touch(touch, t) {
            config.pause_offset_adjust ^= true;
            return Ok(Some(true));
        }
        if self.early_late_btn.touch(touch, t) {
            config.early_late_hint ^= true;
            return Ok(Some(true));
        }
        if self.judge_chart_btn.touch(touch, t) {
            config.ending_judge_chart ^= true;
            return Ok(Some(true));
        }
        if self.health_max_btn.touch(touch, t) {
            request_input("health_max", InputBox::new().default_text(&format!("{:.0}", config.health_max)));
            return Ok(Some(true));
        }
        if let wt @ Some(_) = self.health_scale_slider.touch(touch, t, &mut config.health_scale) {
            config.health_scale = config.health_scale.clamp(0.1, 5.);
            return Ok(wt);
        }
        if self.health_color_btn.touch(touch, t) {
            config.health_bar_color = config.health_bar_color.next();
            return Ok(Some(true));
        }
        if let wt @ Some(_) = self.health_len_slider.touch(touch, t, &mut config.health_bar_len) {
            config.health_bar_len = config.health_bar_len.clamp(Config::HEALTH_BAR_LEN_MIN, Config::HEALTH_BAR_LEN_MAX);
            return Ok(wt);
        }
        if self.motion_btn.touch(touch, t) {
            config.motion_pause ^= true;
            return Ok(Some(true));
        }
        if let wt @ Some(_) = self.motion_threshold_slider.touch(touch, t, &mut config.motion_threshold) {
            config.motion_threshold = config.motion_threshold.clamp(Config::MOTION_THRESHOLD_MIN, Config::MOTION_THRESHOLD_MAX);
            return Ok(wt);
        }
        if let wt @ Some(_) = self.motion_still_slider.touch(touch, t, &mut config.motion_still_time) {
            config.motion_still_time = config.motion_still_time.clamp(Config::MOTION_STILL_MIN, Config::MOTION_STILL_MAX);
            return Ok(wt);
        }

        // ---- 打击表现 ----
        if self.in_game_font_btn.touch(touch, t) {
            config.phi_recorder_font ^= true;
            return Ok(Some(true));
        }
        if self.judge_text_btn.touch(touch, t) {
            config.judge_text ^= true;
            return Ok(Some(true));
        }
        if self.judge_text_style_btn.touch(touch, t) {
            config.judge_text_style = config.judge_text_style.next();
            return Ok(Some(true));
        }
        if let wt @ Some(_) = self.judge_text_size_slider.touch(touch, t, &mut config.judge_text_size) {
            config.judge_text_size = config.judge_text_size.clamp(0.2, 1.5);
            return Ok(wt);
        }
        if self.miss_marker_btn.touch(touch, t) {
            config.miss_marker ^= true;
            return Ok(Some(true));
        }
        if let wt @ Some(_) = self.miss_marker_time_slider.touch(touch, t, &mut config.miss_marker_time) {
            config.miss_marker_time = config.miss_marker_time.clamp(0.2, 10.);
            return Ok(wt);
        }
        if self.note_trail_btn.touch(touch, t) {
            config.note_trail ^= true;
            return Ok(Some(true));
        }
        if let wt @ Some(_) = self.note_trail_len_slider.touch(touch, t, &mut config.note_trail_len) {
            config.note_trail_len = config.note_trail_len.clamp(0., 1.);
            return Ok(wt);
        }
        if let wt @ Some(_) = self.note_trail_alpha_slider.touch(touch, t, &mut config.note_trail_alpha) {
            config.note_trail_alpha = config.note_trail_alpha.clamp(0., 1.);
            return Ok(wt);
        }
        {
            let mut v = config.note_trail_count_value() as f32;
            if let wt @ Some(_) = self.note_trail_count_slider.touch(touch, t, &mut v) {
                config.note_trail_count = (v.round() as u32).clamp(1, Config::AFTERIMAGE_COUNT_MAX);
                return Ok(wt);
            }
        }
        if self.note_trail_click_btn.touch(touch, t) {
            config.note_trail_click ^= true;
            return Ok(Some(true));
        }
        if self.note_trail_drag_btn.touch(touch, t) {
            config.note_trail_drag ^= true;
            return Ok(Some(true));
        }
        if self.note_trail_flick_btn.touch(touch, t) {
            config.note_trail_flick ^= true;
            return Ok(Some(true));
        }
        if self.line_afterimage_btn.touch(touch, t) {
            config.line_afterimage ^= true;
            return Ok(Some(true));
        }
        {
            let mut v = config.line_afterimage_count_value() as f32;
            if let wt @ Some(_) = self.line_afterimage_count_slider.touch(touch, t, &mut v) {
                config.line_afterimage_count = (v.round() as u32).clamp(1, Config::AFTERIMAGE_COUNT_MAX);
                return Ok(wt);
            }
        }
        if let wt @ Some(_) = self.line_afterimage_alpha_slider.touch(touch, t, &mut config.line_afterimage_alpha) {
            config.line_afterimage_alpha = config.line_afterimage_alpha.clamp(0., 1.);
            return Ok(wt);
        }
        if self.music_spectrum_btn.touch(touch, t) {
            config.music_spectrum ^= true;
            return Ok(Some(true));
        }
        if let wt @ Some(_) = self.music_spectrum_gain_slider.touch(touch, t, &mut config.music_spectrum_gain) {
            config.music_spectrum_gain = config.music_spectrum_gain.clamp(0.2, 4.);
            return Ok(wt);
        }
        if self.line_glow_btn.touch(touch, t) {
            config.line_glow ^= true;
            return Ok(Some(true));
        }
        if let wt @ Some(_) = self.line_glow_strength_slider.touch(touch, t, &mut config.line_glow_strength) {
            config.line_glow_strength = config.line_glow_strength.clamp(0., 1.);
            return Ok(wt);
        }

        // ---- 实时 HUD ----
        if self.hud_btn.touch(touch, t) {
            config.hud ^= true;
            return Ok(Some(true));
        }
        if self.hud_acc_btn.touch(touch, t) {
            config.hud_acc ^= true;
            return Ok(Some(true));
        }
        if self.hud_counts_btn.touch(touch, t) {
            config.hud_counts ^= true;
            return Ok(Some(true));
        }
        if self.hud_max_combo_btn.touch(touch, t) {
            config.hud_max_combo ^= true;
            return Ok(Some(true));
        }
        if self.hud_time_btn.touch(touch, t) {
            config.hud_time ^= true;
            return Ok(Some(true));
        }
        if self.hud_delta_btn.touch(touch, t) {
            config.hud_delta ^= true;
            return Ok(Some(true));
        }
        if self.hud_rks_btn.touch(touch, t) {
            config.hud_rks ^= true;
            return Ok(Some(true));
        }
        if self.hud_fps_btn.touch(touch, t) {
            config.hud_fps ^= true;
            return Ok(Some(true));
        }
        if self.hud_corner_btn.touch(touch, t) {
            config.hud_corner = config.hud_corner.next();
            return Ok(Some(true));
        }
        if let wt @ Some(_) = self.hud_size_slider.touch(touch, t, &mut config.hud_size) {
            config.hud_size = config.hud_size.clamp(0.5, 2.);
            return Ok(wt);
        }
        if let wt @ Some(_) = self.hud_alpha_slider.touch(touch, t, &mut config.hud_alpha) {
            config.hud_alpha = config.hud_alpha.clamp(0.1, 1.);
            return Ok(wt);
        }
        if self.hud_outline_btn.touch(touch, t) {
            config.hud_outline ^= true;
            return Ok(Some(true));
        }
        if self.upload_btn.touch(touch, t) {
            if config.upload_record {
                // 已经开着：直接关掉
                config.upload_record = false;
            } else if config.upload_agreed {
                // 之前同意过：直接打开
                config.upload_record = true;
            } else {
                // 第一次打开：先看协议，同意了才真正开
                show_upload_consent(true, false);
            }
            return Ok(Some(true));
        }
        if self.upload_profile_btn.touch(touch, t) {
            // 一键可上传配置：没同意过协议就先弹协议，同意了再真正切换
            if config.upload_agreed {
                let turned = apply_upload_profile_to(config);
                show_message(tl!("upload-profile-done", "mods" => turned.to_string()));
            } else {
                show_upload_consent(true, true);
            }
            return Ok(Some(true));
        }
        if self.upload_consent_btn.touch(touch, t) {
            show_upload_consent(false, false);
            return Ok(Some(false));
        }
        if self.history_btn.touch(touch, t) {
            self.open_history = true;
            return Ok(Some(false));
        }
        Ok(None)
    }

    pub fn update(&mut self, _t: f32) -> Result<bool> {
        Ok(false)
    }

    pub fn render(&mut self, ui: &mut Ui, r: Rect, t: f32) -> (f32, f32) {
        let w = r.w;
        let mut h = 0.;
        macro_rules! item {
            ($($b:tt)*) => {{
                $($b)*
                ui.dy(ITEM_HEIGHT);
                h += ITEM_HEIGHT;
            }}
        }
        let rr = right_rect(w);

        let data = get_data();
        let config = &data.config;
        item! {
            render_title(ui, tl!("item-show-acc"), None);
            render_switch(ui, rr, t, &mut self.show_acc_btn, config.show_acc);
        }
        item! {
            render_title(ui, tl!("item-ap-fc-indicator"), Some(tl!("item-ap-fc-indicator-sub")));
            render_switch(ui, rr, t, &mut self.ap_fc_indicator_btn, config.ap_fc_indicator);
        }
        item! {
            render_title(ui, tl!("item-show-avg-fps"), Some(tl!("item-show-avg-fps-sub")));
            render_switch(ui, rr, t, &mut self.show_avg_fps_btn, config.show_avg_fps);
        }
        item! {
            render_title(ui, tl!("item-dc-pause"), None);
            render_switch(ui, rr, t, &mut self.dc_pause_btn, config.double_click_to_pause);
        }
        item! {
            render_title(ui, tl!("item-dhint"), Some(tl!("item-dhint-sub")));
            render_switch(ui, rr, t, &mut self.dhint_btn, config.double_hint);
        }
        item! {
            render_title(ui, tl!("item-opt"), Some(tl!("item-opt-sub")));
            render_switch(ui, rr, t, &mut self.opt_btn, config.aggressive);
        }
        item! {
            render_title(ui, tl!("item-use-keyboard"), Some(tl!("item-use-keyboard-sub")));
            render_switch(ui, rr, t, &mut self.use_keyboard_btn, config.use_keyboard);
        }
        item! {
            render_title(ui, tl!("item-drag-protect"), Some(tl!("item-drag-protect-sub")));
            render_switch(ui, rr, t, &mut self.drag_protect_btn, config.drag_protect);
        }
        item! {
            render_title(ui, tl!("item-flick-protect"), Some(tl!("item-flick-protect-sub")));
            render_switch(ui, rr, t, &mut self.flick_protect_btn, config.flick_protect);
        }
        item! {
            render_title(ui, tl!("item-judge"), Some(tl!("item-judge-sub")));
            self.judge_slider
                .render(ui, rr, t, config.judge_window, format!("±{:.0}ms", config.judge_window));
        }
        item! {
            let good = config.judge_window_good_ms();
            render_title(ui, tl!("item-judge-good"), Some(tl!("item-judge-good-sub")));
            self.judge_good_slider.render(ui, rr, t, good, format!("±{good:.0}ms"));
        }
        item! {
            let bad = config.judge_window_bad_ms();
            render_title(ui, tl!("item-judge-bad"), Some(tl!("item-judge-bad-sub")));
            self.judge_bad_slider.render(ui, rr, t, bad, format!("±{bad:.0}ms"));
        }
        item! {
            render_title(ui, tl!("item-late-leniency"), Some(tl!("item-late-leniency-sub")));
            self.late_leniency_slider
                .render(ui, rr, t, config.late_leniency_ms, format!("{:.0}ms", config.late_leniency_ms));
        }
        item! {
            let combo_text = if config.combo_text.is_empty() {
                tl!("combo-text-default").to_string()
            } else {
                config.combo_text.clone()
            };
            render_title(ui, tl!("item-combo-text"), Some(tl!("item-combo-text-sub")));
            self.combo_text_btn.render_text(ui, rr, t, &combo_text, 0.5, false);
        }
        item! {
            render_title(ui, tl!("item-line-ref-y-axis"), Some(tl!("item-line-ref-y-axis-sub")));
            render_switch(ui, rr, t, &mut self.line_ref_btn, config.line_ref_y_axis);
        }
        item! {
            render_title(ui, tl!("item-debug-line"), Some(tl!("item-debug-line-sub")));
            render_switch(ui, rr, t, &mut self.debug_line_btn, config.chart_debug_line);
        }
        item! {
            render_title(ui, tl!("item-debug-note"), Some(tl!("item-debug-note-sub")));
            render_switch(ui, rr, t, &mut self.debug_note_btn, config.chart_debug_note);
        }
        item! {
            render_title(ui, tl!("item-pause-offset"), Some(tl!("item-pause-offset-sub")));
            render_switch(ui, rr, t, &mut self.pause_offset_btn, config.pause_offset_adjust);
        }
        item! {
            render_title(ui, tl!("item-early-late"), Some(tl!("item-early-late-sub")));
            render_switch(ui, rr, t, &mut self.early_late_btn, config.early_late_hint);
        }
        item! {
            render_title(ui, tl!("item-judge-chart"), Some(tl!("item-judge-chart-sub")));
            render_switch(ui, rr, t, &mut self.judge_chart_btn, config.ending_judge_chart);
        }
        item! {
            render_title(ui, tl!("item-health-max"), Some(tl!("item-health-max-sub")));
            self.health_max_btn
                .render_text(ui, rr, t, format!("{:.0}", config.health_max), 0.5, false);
        }
        item! {
            render_title(ui, tl!("item-health-scale"), Some(tl!("item-health-scale-sub")));
            self.health_scale_slider
                .render(ui, rr, t, config.health_scale, format!("{:.1}x", config.health_scale));
        }
        item! {
            let color = match config.health_bar_color {
                prpr::config::HealthBarColor::White => tl!("health-color-white"),
                prpr::config::HealthBarColor::Green => tl!("health-color-green"),
                prpr::config::HealthBarColor::Blue => tl!("health-color-blue"),
                prpr::config::HealthBarColor::Red => tl!("health-color-red"),
                prpr::config::HealthBarColor::Gold => tl!("health-color-gold"),
                prpr::config::HealthBarColor::Purple => tl!("health-color-purple"),
            };
            render_title(ui, tl!("item-health-color"), Some(tl!("item-health-color-sub")));
            self.health_color_btn.render_text(ui, rr, t, color, 0.5, false);
        }
        item! {
            render_title(ui, tl!("item-health-len"), Some(tl!("item-health-len-sub")));
            self.health_len_slider
                .render(ui, rr, t, config.health_bar_len, format!("{:.0}%", config.health_bar_len * 100.));
        }
        item! {
            render_title(ui, tl!("item-motion"), Some(tl!("item-motion-sub")));
            render_switch(ui, rr, t, &mut self.motion_btn, config.motion_pause);
        }
        item! {
            render_title(ui, tl!("item-motion-threshold"), Some(tl!("item-motion-threshold-sub")));
            self.motion_threshold_slider
                .render(ui, rr, t, config.motion_threshold, format!("{:.2}g", config.motion_threshold));
        }
        item! {
            render_title(ui, tl!("item-motion-still"), Some(tl!("item-motion-still-sub")));
            self.motion_still_slider
                .render(ui, rr, t, config.motion_still_time, format!("{:.1}s", config.motion_still_time));
        }
        item! {
            render_title(ui, tl!("item-speed"), None);
            self.speed_slider.render(ui, rr, t, config.speed, format!("{:.2}", config.speed));
        }
        item! {
            render_title(ui, tl!("item-note-size"), None);
            self.size_slider.render(ui, rr, t, config.note_scale, format!("{:.3}", config.note_scale));
        }
        // ---- 打击表现 ----
        item! {
            render_title(ui, tl!("item-in-game-font"), Some(tl!("item-in-game-font-sub")));
            render_switch(ui, rr, t, &mut self.in_game_font_btn, config.phi_recorder_font);
        }
        item! {
            render_title(ui, tl!("item-judge-text"), Some(tl!("item-judge-text-sub")));
            render_switch(ui, rr, t, &mut self.judge_text_btn, config.judge_text);
        }
        item! {
            let style = match config.judge_text_style {
                prpr::config::JudgeTextStyle::Fade => tl!("judge-text-style-fade"),
                prpr::config::JudgeTextStyle::Rise => tl!("judge-text-style-rise"),
                prpr::config::JudgeTextStyle::Pop => tl!("judge-text-style-pop"),
                prpr::config::JudgeTextStyle::RisePop => tl!("judge-text-style-rise-pop"),
            };
            render_title(ui, tl!("item-judge-text-style"), Some(tl!("item-judge-text-style-sub")));
            self.judge_text_style_btn.render_text(ui, rr, t, style, 0.5, false);
        }
        item! {
            render_title(ui, tl!("item-judge-text-size"), Some(tl!("item-judge-text-size-sub")));
            self.judge_text_size_slider
                .render(ui, rr, t, config.judge_text_size, format!("{:.2}", config.judge_text_size));
        }
        item! {
            render_title(ui, tl!("item-miss-marker"), Some(tl!("item-miss-marker-sub")));
            render_switch(ui, rr, t, &mut self.miss_marker_btn, config.miss_marker);
        }
        item! {
            render_title(ui, tl!("item-miss-marker-time"), Some(tl!("item-miss-marker-time-sub")));
            self.miss_marker_time_slider
                .render(ui, rr, t, config.miss_marker_time, format!("{:.1}s", config.miss_marker_time));
        }
        item! {
            render_title(ui, tl!("item-note-trail"), Some(tl!("item-note-trail-sub")));
            render_switch(ui, rr, t, &mut self.note_trail_btn, config.note_trail);
        }
        item! {
            render_title(ui, tl!("item-note-trail-len"), Some(tl!("item-note-trail-len-sub")));
            self.note_trail_len_slider
                .render(ui, rr, t, config.note_trail_len, format!("{:.0}%", config.note_trail_len * 100.));
        }
        item! {
            render_title(ui, tl!("item-note-trail-alpha"), Some(tl!("item-note-trail-alpha-sub")));
            self.note_trail_alpha_slider
                .render(ui, rr, t, config.note_trail_alpha, format!("{:.0}%", config.note_trail_alpha * 100.));
        }
        item! {
            render_title(ui, tl!("item-note-trail-count"), Some(tl!("item-note-trail-count-sub")));
            self.note_trail_count_slider
                .render(ui, rr, t, config.note_trail_count as f32, format!("{}", config.note_trail_count_value()));
        }
        item! {
            render_title(ui, tl!("item-note-trail-click"), None);
            render_switch(ui, rr, t, &mut self.note_trail_click_btn, config.note_trail_click);
        }
        item! {
            render_title(ui, tl!("item-note-trail-drag"), None);
            render_switch(ui, rr, t, &mut self.note_trail_drag_btn, config.note_trail_drag);
        }
        item! {
            render_title(ui, tl!("item-note-trail-flick"), None);
            render_switch(ui, rr, t, &mut self.note_trail_flick_btn, config.note_trail_flick);
        }
        item! {
            render_title(ui, tl!("item-line-afterimage"), Some(tl!("item-line-afterimage-sub")));
            render_switch(ui, rr, t, &mut self.line_afterimage_btn, config.line_afterimage);
        }
        item! {
            render_title(ui, tl!("item-line-afterimage-count"), Some(tl!("item-line-afterimage-count-sub")));
            self.line_afterimage_count_slider.render(
                ui,
                rr,
                t,
                config.line_afterimage_count as f32,
                format!("{}", config.line_afterimage_count_value()),
            );
        }
        item! {
            render_title(ui, tl!("item-line-afterimage-alpha"), Some(tl!("item-line-afterimage-alpha-sub")));
            self.line_afterimage_alpha_slider
                .render(ui, rr, t, config.line_afterimage_alpha, format!("{:.0}%", config.line_afterimage_alpha * 100.));
        }
        item! {
            render_title(ui, tl!("item-music-spectrum"), Some(tl!("item-music-spectrum-sub")));
            render_switch(ui, rr, t, &mut self.music_spectrum_btn, config.music_spectrum);
        }
        item! {
            render_title(ui, tl!("item-music-spectrum-gain"), Some(tl!("item-music-spectrum-gain-sub")));
            self.music_spectrum_gain_slider
                .render(ui, rr, t, config.music_spectrum_gain, format!("{:.1}x", config.music_spectrum_gain));
        }
        item! {
            render_title(ui, tl!("item-line-glow"), Some(tl!("item-line-glow-sub")));
            render_switch(ui, rr, t, &mut self.line_glow_btn, config.line_glow);
        }
        item! {
            render_title(ui, tl!("item-line-glow-strength"), Some(tl!("item-line-glow-strength-sub")));
            self.line_glow_strength_slider
                .render(ui, rr, t, config.line_glow_strength, format!("{:.0}%", config.line_glow_strength * 100.));
        }
        // ---- 实时 HUD ----
        item! {
            render_title(ui, tl!("item-hud"), Some(tl!("item-hud-sub")));
            render_switch(ui, rr, t, &mut self.hud_btn, config.hud);
        }
        item! {
            render_title(ui, tl!("item-hud-acc"), None);
            render_switch(ui, rr, t, &mut self.hud_acc_btn, config.hud_acc);
        }
        item! {
            render_title(ui, tl!("item-hud-counts"), None);
            render_switch(ui, rr, t, &mut self.hud_counts_btn, config.hud_counts);
        }
        item! {
            render_title(ui, tl!("item-hud-max-combo"), None);
            render_switch(ui, rr, t, &mut self.hud_max_combo_btn, config.hud_max_combo);
        }
        item! {
            render_title(ui, tl!("item-hud-time"), None);
            render_switch(ui, rr, t, &mut self.hud_time_btn, config.hud_time);
        }
        item! {
            render_title(ui, tl!("item-hud-delta"), None);
            render_switch(ui, rr, t, &mut self.hud_delta_btn, config.hud_delta);
        }
        item! {
            render_title(ui, tl!("item-hud-rks"), Some(tl!("item-hud-rks-sub")));
            render_switch(ui, rr, t, &mut self.hud_rks_btn, config.hud_rks);
        }
        item! {
            render_title(ui, tl!("item-hud-fps"), None);
            render_switch(ui, rr, t, &mut self.hud_fps_btn, config.hud_fps);
        }
        item! {
            let corner = match config.hud_corner {
                prpr::config::HudCorner::TopLeft => tl!("hud-corner-tl"),
                prpr::config::HudCorner::TopCenter => tl!("hud-corner-tc"),
                prpr::config::HudCorner::TopRight => tl!("hud-corner-tr"),
                prpr::config::HudCorner::LeftCenter => tl!("hud-corner-ml"),
                prpr::config::HudCorner::Center => tl!("hud-corner-mc"),
                prpr::config::HudCorner::RightCenter => tl!("hud-corner-mr"),
                prpr::config::HudCorner::BottomLeft => tl!("hud-corner-bl"),
                prpr::config::HudCorner::BottomCenter => tl!("hud-corner-bc"),
                prpr::config::HudCorner::BottomRight => tl!("hud-corner-br"),
            };
            render_title(ui, tl!("item-hud-corner"), Some(tl!("item-hud-corner-sub")));
            self.hud_corner_btn.render_text(ui, rr, t, corner, 0.5, false);
        }
        item! {
            render_title(ui, tl!("item-hud-size"), Some(tl!("item-hud-size-sub")));
            self.hud_size_slider
                .render(ui, rr, t, config.hud_size, format!("{:.2}x", config.hud_size));
        }
        item! {
            render_title(ui, tl!("item-hud-alpha"), Some(tl!("item-hud-alpha-sub")));
            self.hud_alpha_slider
                .render(ui, rr, t, config.hud_alpha, format!("{:.0}%", config.hud_alpha * 100.));
        }
        item! {
            render_title(ui, tl!("item-hud-outline"), Some(tl!("item-hud-outline-sub")));
            render_switch(ui, rr, t, &mut self.hud_outline_btn, config.hud_outline);
        }
        item! {
            render_title(ui, tl!("item-upload"), Some(tl!("item-upload-sub")));
            render_switch(ui, rr, t, &mut self.upload_btn, config.upload_record);
        }
        item! {
            render_title(ui, tl!("item-upload-profile"), Some(tl!("item-upload-profile-sub")));
            self.upload_profile_btn.render_text(ui, rr, t, tl!("item-upload-profile-apply"), 0.42, false);
        }
        item! {
            render_title(ui, tl!("item-upload-consent"), None);
            self.upload_consent_btn.render_text(ui, rr, t, tl!("item-upload-consent-open"), 0.45, false);
        }
        item! {
            render_title(ui, tl!("item-history"), Some(tl!("item-history-sub")));
            self.history_btn.render_text(ui, rr, t, tl!("item-history-open"), 0.45, false);
        }
        (w, h)
    }
}

struct DebugList {
    chart_debug_btn: DRectButton,
    touch_style_btn: DRectButton,
    touch_color_btn: DRectButton,
    touch_alpha_slider: Slider,
    touch_debug_btn: DRectButton,
}

impl DebugList {
    pub fn new() -> Self {
        Self {
            chart_debug_btn: DRectButton::new(),
            touch_debug_btn: DRectButton::new(),
            touch_style_btn: DRectButton::new(),
            touch_color_btn: DRectButton::new(),
            touch_alpha_slider: Slider::new(0.05..1., 0.05),
        }
    }

    pub fn top_touch(&mut self, _touch: &Touch, _t: f32) -> bool {
        false
    }

    pub fn touch(&mut self, touch: &Touch, t: f32) -> Result<Option<bool>> {
        let data = get_data_mut();
        let config = &mut data.config;
        if self.chart_debug_btn.touch(touch, t) {
            config.chart_debug ^= true;
            return Ok(Some(true));
        }
        if self.touch_debug_btn.touch(touch, t) {
            config.touch_debug ^= true;
            return Ok(Some(true));
        }
        if self.touch_style_btn.touch(touch, t) {
            config.touch_marker_style = config.touch_marker_style.next();
            return Ok(Some(true));
        }
        if self.touch_color_btn.touch(touch, t) {
            config.touch_marker_color = config.touch_marker_color.next();
            return Ok(Some(true));
        }
        if let wt @ Some(_) = self.touch_alpha_slider.touch(touch, t, &mut config.touch_marker_alpha) {
            config.touch_marker_alpha = config.touch_marker_alpha.clamp(0.05, 1.);
            return Ok(wt);
        }
        Ok(None)
    }

    pub fn update(&mut self, _t: f32) -> Result<bool> {
        Ok(false)
    }

    pub fn render(&mut self, ui: &mut Ui, r: Rect, t: f32) -> (f32, f32) {
        let w = r.w;
        let mut h = 0.;
        macro_rules! item {
            ($($b:tt)*) => {{
                $($b)*
                ui.dy(ITEM_HEIGHT);
                h += ITEM_HEIGHT;
            }}
        }
        let rr = right_rect(w);

        let data = get_data();
        let config = &data.config;
        item! {
            render_title(ui, tl!("item-chart-debug"), Some(tl!("item-chart-debug-sub")));
            render_switch(ui, rr, t, &mut self.chart_debug_btn, config.chart_debug);
        }
        item! {
            render_title(ui, tl!("item-touch-debug"), Some(tl!("item-touch-debug-sub")));
            render_switch(ui, rr, t, &mut self.touch_debug_btn, config.touch_debug);
        }
        item! {
            let style = match config.touch_marker_style {
                prpr::config::TouchMarkerStyle::Circle => tl!("touch-style-circle"),
                prpr::config::TouchMarkerStyle::HitFx => tl!("touch-style-hitfx"),
                prpr::config::TouchMarkerStyle::Avatar => tl!("touch-style-avatar"),
            };
            render_title(ui, tl!("item-touch-style"), Some(tl!("item-touch-style-sub")));
            self.touch_style_btn.render_text(ui, rr, t, style, 0.5, false);
        }
        item! {
            let color = match config.touch_marker_color {
                prpr::config::TouchMarkerColor::White => tl!("touch-color-white"),
                prpr::config::TouchMarkerColor::Red => tl!("touch-color-red"),
                prpr::config::TouchMarkerColor::Green => tl!("touch-color-green"),
                prpr::config::TouchMarkerColor::Blue => tl!("touch-color-blue"),
                prpr::config::TouchMarkerColor::Gold => tl!("touch-color-gold"),
                prpr::config::TouchMarkerColor::Purple => tl!("touch-color-purple"),
            };
            render_title(ui, tl!("item-touch-color"), Some(tl!("item-touch-color-sub")));
            self.touch_color_btn.render_text(ui, rr, t, color, 0.5, false);
        }
        item! {
            render_title(ui, tl!("item-touch-alpha"), Some(tl!("item-touch-alpha-sub")));
            self.touch_alpha_slider
                .render(ui, rr, t, config.touch_marker_alpha, format!("{:.0}%", config.touch_marker_alpha * 100.));
        }
        (w, h)
    }
}

/// 把配置切成「可以被上传」的状态：
/// - 关掉所有不计分（UNRATED）或影响公平性的选项；
/// - 速度回到 1.0、判定窗口回到官方 ±80 / 160 / 220、late 补偿归零；
/// - 打开成绩上传。
///
/// 返回被关掉的 UNRATED Mod 数量（用于提示里显示）。
/// 直接吃现成的 `&mut Config`，避免在已经借了 config 的地方再 get_data_mut()。
fn apply_upload_profile_to(config: &mut Config) -> usize {
    use prpr::config::Mods;
    // 以前这里只清了 5 个 mod（AUTOPLAY / NO_SHADER / NO_COMBO_SCORE /
    // FULL_SCREEN_JUDGE / HEALTH_MODE），别的 mod（FLIP_X、NIGHTCORE、FADE_IN/OUT、
    // RAINBOW、INSTANT_DEATH_* …）还留着 —— 引擎照样判定"这张谱面被改过"，
    // 于是按了「一键切换到可上传配置」之后依然弹「不能上传成绩」。直接全清。
    let turned = config.mods.bits().count_ones() as usize;
    config.mods = Mods::empty();
    // 每张谱面还各自记了一份 mod（LocalChart.mods），不清的话单独某张谱面照样传不了
    for chart in &mut get_data_mut().charts {
        chart.mods = Mods::default();
    }
    config.speed = 1.;
    // 判定窗口回到官方默认；good / bad 用 None 表示"跟着 perfect 派生"
    config.judge_window = Config::DEFAULT_JUDGE_WINDOW;
    config.judge_window_good = None;
    config.judge_window_bad = None;
    config.late_leniency_ms = 0.;
    config.drag_protect = false;
    config.flick_protect = false;
    // 下面两个是引擎判断「能否上传」的直接条件（见 prpr/src/scene/game.rs 里构造
    // record_data 那一段），之前一直漏着 —— 开了离线模式或键盘游玩时，就算 mod 全清、
    // 速度 1.0，照样不上传，而且只弹一句「不能上传」，极难排查。
    config.offline_mode = false;
    config.use_keyboard = false;
    config.upload_record = true;
    turned
}

/// 成绩上传协议。
///
/// `ask` = true：这是「要打开上传 / 一键配置」的流程，两个按钮，点了同意才继续；
/// `ask` = false：只是查看协议。
/// `apply_profile` = true：同意之后顺带切换成可上传配置。
fn show_upload_consent(ask: bool, apply_profile: bool) {
    let mut dialog = Dialog::plain(tl!("upload-consent-title"), tl!("upload-consent-text").into_owned());
    if ask {
        dialog = dialog
            .buttons(vec![tl!("upload-consent-deny").into_owned(), tl!("upload-consent-accept").into_owned()])
            .listener(move |_dialog, pos| {
                if pos == 1 {
                    let config = &mut get_data_mut().config;
                    config.upload_agreed = true;
                    let turned = if apply_profile { apply_upload_profile_to(config) } else { 0 };
                    config.upload_record = true;
                    let _ = save_data();
                    if apply_profile {
                        show_message(tl!("upload-profile-done", "mods" => turned.to_string()));
                    }
                }
                false
            });
    } else {
        dialog = dialog.buttons(vec![tl!("ok").into_owned()]);
    }
    dialog.show();
}
