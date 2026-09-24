//! 内嵌的双人对战服务端。
//!
//! 这个文件现在是**真正的服务端**，不再是"占位监听器"：
//! `phira_duel::Duel` 直接跑在开服器进程里，界面轮询 `state()` 就能读到
//! 房间码、阶段、玩家、比分、事件流。
//!
//! 为什么要绑两个 listener：**Windows 上 tokio 绑 `[::]` 出来的 socket 只收 IPv6**，
//! IPv4 连过来会被直接拒绝（实测 `127.0.0.1` 报「目标计算机积极拒绝」）。
//! 所以 `[::]` 收 IPv6（公网直连走这条）、`0.0.0.0` 收 IPv4（同一 WiFi 走这条），
//! 各自跑一个 accept 循环，共用同一份房间状态。

use phira_duel::{server::Duel, RoomState, ServerConfig};
use serde::Serialize;
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter};
use tokio::{net::TcpListener, sync::Notify};

const LOG_LINES: usize = 500;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LogLine {
    pub seq: u64,
    pub at: u64,
    pub level: String,
    pub text: String,
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|it| it.as_secs()).unwrap_or(0)
}

#[derive(Default)]
pub struct Log {
    lines: Mutex<VecDeque<LogLine>>,
    seq: AtomicU64,
}

impl Log {
    pub fn push(&self, app: &AppHandle, level: &str, text: impl Into<String>) {
        let line = LogLine {
            seq: self.seq.fetch_add(1, Ordering::Relaxed),
            at: now_secs(),
            level: level.to_string(),
            text: text.into(),
        };
        {
            let mut guard = self.lines.lock().unwrap();
            guard.push_back(line.clone());
            while guard.len() > LOG_LINES {
                guard.pop_front();
            }
        }
        // 同时写一份文件：绿色版工具，出问题时能直接翻日志（界面没开/崩了也看得到）
        append_file_log(&line);
        let _ = app.emit("log", line);
    }

    pub fn snapshot(&self) -> Vec<LogLine> {
        self.lines.lock().unwrap().iter().cloned().collect()
    }
}

/// 日志文件放在 exe 旁边（`launcher.log`），超过 2MB 就截断重来。
fn append_file_log(line: &LogLine) {
    use std::io::Write;
    let path = crate::config::config_path().with_file_name("launcher.log");
    if let Ok(meta) = std::fs::metadata(&path) {
        if meta.len() > 2 * 1024 * 1024 {
            let _ = std::fs::remove_file(&path);
        }
    }
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let secs = line.at % 86400;
        let _ = writeln!(
            file,
            "[{:02}:{:02}:{:02}] [{}] {}",
            secs / 3600,
            (secs % 3600) / 60,
            secs % 60,
            line.level,
            line.text
        );
    }
}

/// 服务端运行状态（给界面看）
#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub running: bool,
    pub port: u16,
    /// 对战房间状态
    pub room: RoomState,
}

pub struct Server {
    running: AtomicBool,
    port: AtomicU16,
    stop: Arc<Notify>,
    duel: Mutex<Option<Arc<Duel>>>,
    /// 已经推给界面的对战事件条数（只推增量，免得每秒重复刷屏）
    event_mark: Mutex<usize>,
}

impl Default for Server {
    fn default() -> Self {
        Self {
            running: AtomicBool::new(false),
            port: AtomicU16::new(0),
            stop: Arc::new(Notify::new()),
            duel: Mutex::new(None),
            event_mark: Mutex::new(0),
        }
    }
}

impl Server {
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// 拿当前房间状态；顺便把新增的对战事件写进开服器日志。
    pub async fn snapshot(&self, app: Option<&AppHandle>, log: Option<&Log>) -> Snapshot {
        let duel = self.duel.lock().unwrap().clone();
        let running = self.running.load(Ordering::Relaxed);
        let Some(duel) = duel else {
            return Snapshot {
                running,
                port: self.port.load(Ordering::Relaxed),
                room: RoomState::default(),
            };
        };
        let room = duel.state().await;
        // 增量推送事件
        if let (Some(app), Some(log)) = (app, log) {
            let mut mark = self.event_mark.lock().unwrap();
            if room.events.len() < *mark {
                *mark = 0; // 房间被重置过
            }
            for ev in room.events.iter().skip(*mark) {
                log.push(app, "duel", ev.clone());
            }
            *mark = room.events.len();
        }
        Snapshot {
            running,
            port: self.port.load(Ordering::Relaxed),
            room,
        }
    }

    pub fn stop(&self, app: &AppHandle, log: &Log) {
        self.stop.notify_waiters();
        self.running.store(false, Ordering::Relaxed);
        *self.duel.lock().unwrap() = None;
        *self.event_mark.lock().unwrap() = 0;
        log.push(app, "info", "服务端已停止");
    }

    /// 起服务端：`[::]` 和 `0.0.0.0` 各绑一个，共用同一份房间状态。
    pub async fn start(&self, app: AppHandle, log: Arc<Log>, port: u16) -> anyhow::Result<()> {
        if self.is_running() {
            anyhow::bail!("服务端已经在跑了");
        }
        let mut listeners: Vec<TcpListener> = Vec::new();
        let mut v6 = false;
        let mut v4 = false;
        match TcpListener::bind(("::", port)).await {
            Ok(it) => {
                v6 = true;
                listeners.push(it);
            }
            Err(err) => log.push(&app, "warn", format!("IPv6 [::]:{port} 绑定失败：{err}")),
        }
        match TcpListener::bind(("0.0.0.0", port)).await {
            Ok(it) => {
                v4 = true;
                listeners.push(it);
            }
            Err(err) => log.push(&app, "warn", format!("IPv4 0.0.0.0:{port} 绑定失败：{err}")),
        }
        if listeners.is_empty() {
            anyhow::bail!("端口 {port} 既绑不上 IPv6 也绑不上 IPv4（可能被别的程序占用了）");
        }

        let duel = Arc::new(Duel::new(ServerConfig::default()));
        *self.duel.lock().unwrap() = Some(duel.clone());
        *self.event_mark.lock().unwrap() = 0;
        self.running.store(true, Ordering::Relaxed);
        self.port.store(port, Ordering::Relaxed);

        log.push(
            &app,
            "ok",
            format!(
                "双人对战服务端已就绪，端口 {port}（{}）",
                match (v6, v4) {
                    (true, true) => "IPv6 + IPv4",
                    (true, false) => "仅 IPv6（IPv4 客户端连不上）",
                    _ => "仅 IPv4",
                }
            ),
        );
        log.push(&app, "info", "客户端在「设置 → 对战服务器」里填地址，进对战房间即可");

        for listener in listeners {
            let duel = duel.clone();
            let task_log = log.clone();
            let task_app = app.clone();
            let stop = self.stop.clone();
            tokio::spawn(async move {
                tokio::select! {
                    _ = stop.notified() => {}
                    res = duel.serve(listener) => {
                        if let Err(err) = res {
                            task_log.push(&task_app, "warn", format!("accept 循环退出：{err}"));
                        }
                    }
                }
            });
        }
        Ok(())
    }
}
