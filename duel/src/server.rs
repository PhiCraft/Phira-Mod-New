//! 服务端：一个房间、两个人、BO3 + BP 选曲 + 能量制干扰。
//!
//! 设计上的几个取舍（都写在对应位置，方便以后回看）：
//! * **不碰谱面和音频**：两个玩家本地都有谱面，服务端只交换谱面 ID 和事件，
//!   所以它很小、很快，也不需要任何文件存储。
//! * **不对表**：开局只发「再过 N 毫秒开始」，各端用自己的时钟算，省掉时间同步。
//! * **能量由客户端上报**：这是朋友之间玩的工具，不做反作弊；服务端只做
//!   上限钳制 + 冷却 + 开局锁定的合法性校验，防止手滑点爆。
//! * v1 一台服务器同时只有一个房间（房间码由第一个进房的人设定，第二个人必须一致）。

use crate::proto::{read_msg, write_msg, BpAction, ClientMsg, Interference, NextStep, PlayerInfo, RoundScore, ServerMsg};
use serde::Serialize;
use std::{
    collections::{HashSet, VecDeque},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{mpsc, Mutex},
};

const SLOTS: usize = 2;
/// 事件环形缓冲长度（给开服器界面滚动显示用）
const EVENT_LINES: usize = 200;

/// 给开服器界面看的房间快照（每秒轮询一次就够，不用推送）
#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct RoomState {
    pub code: Option<String>,
    /// 阶段的中文标签
    pub phase: String,
    /// BP 阶段轮到谁做什么
    pub bp_turn: Option<String>,
    pub players: Vec<String>,
    pub round: u8,
    pub wins: [u8; SLOTS],
    pub pool: Vec<u32>,
    pub banned: Vec<u32>,
    pub picks: Vec<u32>,
    pub decider: Option<u32>,
    pub energies: Vec<f32>,
    /// 最近的事件（界面直接滚出来）
    pub events: Vec<String>,
}

/// 服务端句柄：**同一个进程里内嵌**时用它拿房间状态。
///
/// 开服器就是这么用的：`Duel::new()` → 自己在 `[::]` 和 `0.0.0.0` 上各起一个
/// accept 循环（Windows 上只绑 `[::]` 是收不到 IPv4 的）→ 界面轮询 `state()`。
pub struct Duel {
    room: Arc<Mutex<Room>>,
    cfg: ServerConfig,
}

impl Duel {
    pub fn new(cfg: ServerConfig) -> Self {
        Self {
            room: Arc::new(Mutex::new(Room::new(cfg.clone()))),
            cfg,
        }
    }

    pub fn config(&self) -> &ServerConfig {
        &self.cfg
    }

    /// 在一个已绑好的 listener 上跑 accept 循环（可以同时调多次，绑多个地址）
    pub async fn serve(self: &Arc<Self>, listener: TcpListener) -> anyhow::Result<()> {
        loop {
            let (stream, _peer) = listener.accept().await?;
            let room = self.room.clone();
            tokio::spawn(async move {
                if let Err(err) = handle(stream, room).await {
                    eprintln!("[duel] 连接异常：{err}");
                }
            });
        }
    }

    pub async fn state(&self) -> RoomState {
        self.room.lock().await.state()
    }

    /// 自测用：直接给这个句柄灌一个假房间状态是不允许的，但可以查它有没有人
    pub async fn is_empty(&self) -> bool {
        self.room.lock().await.players.iter().all(|it| it.is_none())
    }
}

#[derive(Clone, Debug)]
pub struct ServerConfig {
    /// 开局后多少毫秒内禁止投干扰（让双方先进入状态）
    pub cast_lock_ms: u64,
    /// 能量上限
    pub energy_max: f32,
    /// 同一个干扰的冷却
    pub cooldown_ms: u64,
    /// BP 结束后到正式开局的准备时间
    pub start_delay_ms: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            cast_lock_ms: 10_000,
            energy_max: 100.,
            cooldown_ms: 8_000,
            start_delay_ms: 3_000,
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|it| it.as_millis() as u64).unwrap_or(0)
}

struct Player {
    name: String,
    charts: Vec<u32>,
    tx: mpsc::UnboundedSender<ServerMsg>,
    /// 客户端上报的最新能量（服务端会扣掉自己花掉的）
    energy: f32,
    /// 每种干扰的上次使用时间
    cooldown: [u64; Interference::ALL.len()],
    /// 本局是否已交卷
    finished: bool,
    score: u32,
    accuracy: f32,
    max_combo: u32,
}

impl Player {
    fn new(name: String, charts: Vec<u32>, tx: mpsc::UnboundedSender<ServerMsg>) -> Self {
        Self {
            name,
            charts,
            tx,
            energy: 0.,
            cooldown: [0; Interference::ALL.len()],
            finished: false,
            score: 0,
            accuracy: 0.,
            max_combo: 0,
        }
    }

    fn send(&self, msg: ServerMsg) {
        let _ = self.tx.send(msg);
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Phase {
    /// 等两个人到齐
    Lobby,
    /// BP 阶段：`actor` 要做一个 `action`
    Bp { actor: u8, action: BpAction, decider: bool },
    /// 一局进行中
    Round { index: u8, started: u64 },
    /// 打完收工
    Done,
}

struct Room {
    cfg: ServerConfig,
    code: Option<String>,
    players: [Option<Player>; SLOTS],
    pool: Vec<u32>,
    banned: Vec<u32>,
    picks: Vec<u32>,
    /// 决胜局选的谱面（1:1 时由败者选）
    decider: Option<u32>,
    played: Vec<u32>,
    phase: Phase,
    round: u8,
    /// 每局结果，用于最终战绩
    outcomes: Vec<RoundScore>,
    wins: [u8; SLOTS],
    /// 给界面看的事件流（谁进房 / 谁 ban 了什么 / 谁投了干扰 / 结算……）
    events: VecDeque<String>,
}

impl Room {
    fn new(cfg: ServerConfig) -> Self {
        Self {
            cfg,
            code: None,
            players: [None, None],
            pool: Vec::new(),
            banned: Vec::new(),
            picks: Vec::new(),
            decider: None,
            played: Vec::new(),
            phase: Phase::Lobby,
            round: 0,
            outcomes: Vec::new(),
            wins: [0; SLOTS],
            events: VecDeque::new(),
        }
    }

    /// 记一条给界面看的事件
    fn log(&mut self, text: impl Into<String>) {
        self.events.push_back(text.into());
        while self.events.len() > EVENT_LINES {
            self.events.pop_front();
        }
    }

    /// 给开服器界面用的快照
    fn state(&self) -> RoomState {
        let phase = match self.phase {
            Phase::Lobby => "等待玩家",
            Phase::Bp { .. } => "BP 选曲",
            Phase::Round { .. } => "对局中",
            Phase::Done => "已结束",
        };
        let bp_turn = match self.phase {
            Phase::Bp { actor, action, decider } => Some(format!("玩家{} {}{}", actor + 1, action.name(), if decider { "（决胜局）" } else { "" })),
            _ => None,
        };
        RoomState {
            code: self.code.clone(),
            phase: phase.to_string(),
            bp_turn,
            players: self.players.iter().flatten().map(|p| p.name.clone()).collect(),
            round: self.round,
            wins: [self.wins[0], self.wins[1]],
            pool: self.pool.clone(),
            banned: self.banned.clone(),
            picks: self.picks.clone(),
            decider: self.decider,
            energies: self.players.iter().map(|p| p.as_ref().map(|p| p.energy).unwrap_or(0.)).collect(),
            events: self.events.iter().cloned().collect(),
        }
    }

    fn broadcast(&self, msg: ServerMsg) {
        for p in self.players.iter().flatten() {
            p.send(msg.clone());
        }
    }

    fn to_other(&self, from: u8, msg: ServerMsg) {
        for (i, p) in self.players.iter().enumerate() {
            if i as u8 != from {
                if let Some(p) = p {
                    p.send(msg.clone());
                }
            }
        }
    }

    fn both_present(&self) -> bool {
        self.players.iter().all(|it| it.is_some())
    }

    /// 可以 ban / pick 的谱面
    fn selectable(&self) -> Vec<u32> {
        self.pool
            .iter()
            .copied()
            .filter(|c| !self.banned.contains(c) && !self.picks.contains(c) && Some(*c) != self.decider)
            .collect()
    }

    fn begin_bp(&mut self) {
        self.pool = {
            let a: HashSet<u32> = self.players[0].as_ref().map(|p| p.charts.iter().copied().collect()).unwrap_or_default();
            let b: HashSet<u32> = self.players[1].as_ref().map(|p| p.charts.iter().copied().collect()).unwrap_or_default();
            let mut common: Vec<u32> = a.intersection(&b).copied().collect();
            common.sort_unstable();
            common
        };
        self.broadcast(ServerMsg::Pool { charts: self.pool.clone() });
        self.phase = Phase::Bp {
            actor: 0,
            action: BpAction::Ban,
            decider: false,
        };
        self.announce_bp_turn();
    }

    fn announce_bp_turn(&mut self) {
        if let Phase::Bp { actor, action, decider } = self.phase {
            self.broadcast(ServerMsg::BpTurn {
                actor,
                action,
                pool: self.selectable(),
                decider,
            });
        }
    }

    fn start_round(&mut self) {
        let (index, chart) = match self.round {
            0 => (0u8, self.picks.first().copied()),
            1 => (1u8, self.picks.get(1).copied()),
            _ => (2u8, self.decider),
        };
        let Some(chart) = chart else {
            self.broadcast(ServerMsg::Error {
                message: "没有可用的谱面，比赛中断".into(),
            });
            self.phase = Phase::Done;
            return;
        };
        self.played.push(chart);
        for p in self.players.iter_mut().flatten() {
            p.finished = false;
            p.energy = 0.;
        }
        self.phase = Phase::Round { index, started: now_ms() };
        self.log(format!("第 {} 局开始：谱面 {chart}", index + 1));
        self.broadcast(ServerMsg::RoundStart {
            index,
            chart,
            starts_in_ms: self.cfg.start_delay_ms,
        });
    }

    /// 一局结束：算胜负，决定下一步
    fn finish_round(&mut self) {
        let index = self.round;
        let mut scores = Vec::new();
        for (i, p) in self.players.iter().enumerate() {
            let Some(p) = p else { continue };
            scores.push(RoundScore {
                player: i as u8,
                score: p.score,
                accuracy: p.accuracy,
                max_combo: p.max_combo,
            });
        }
        // 分数高者胜；同分比准度；都一样就是平局（None）
        let winner = match (self.players[0].as_ref(), self.players[1].as_ref()) {
            (Some(a), Some(b)) => {
                let key = |p: &Player| (p.score, (p.accuracy * 10000.) as u32);
                match key(a).cmp(&key(b)) {
                    std::cmp::Ordering::Greater => Some(0u8),
                    std::cmp::Ordering::Less => Some(1u8),
                    std::cmp::Ordering::Equal => None,
                }
            }
            _ => None,
        };
        if let Some(w) = winner {
            self.wins[w as usize] += 1;
        }
        self.outcomes.extend(scores.iter().cloned());

        // 两胜即结束
        let decided = self.wins.iter().any(|w| *w >= 2) || index >= 2;
        let next = if decided {
            NextStep::End
        } else if index == 0 {
            NextStep::Round { index: 1 }
        } else {
            // 1:1 → 决胜局由**上一局的败者**选曲（给刚输的人一次主动权的翻盘机制）。
            // 上一局是平局时退回「胜场少的一方」，再一样就 0 号。
            let actor = match winner {
                Some(w) => 1 - w,
                None => {
                    if self.wins[0] <= self.wins[1] {
                        0
                    } else {
                        1
                    }
                }
            };
            NextStep::Bp { actor }
        };
        self.log(match winner {
            Some(w) => format!("第 {} 局结束：玩家{} 胜", index + 1, w + 1),
            None => format!("第 {} 局结束：平局", index + 1),
        });
        self.broadcast(ServerMsg::RoundEnd { index, winner, scores, next });

        match next {
            NextStep::Round { index } => {
                self.round = index;
                self.start_round();
            }
            NextStep::Bp { actor } => {
                self.round = 2;
                self.phase = Phase::Bp {
                    actor,
                    action: BpAction::Pick,
                    decider: true,
                };
                self.announce_bp_turn();
            }
            NextStep::End => {
                self.phase = Phase::Done;
                let winner = if self.wins[0] >= self.wins[1] { 0u8 } else { 1u8 };
                self.broadcast(ServerMsg::MatchEnd {
                    winner,
                    scores: self.outcomes.clone(),
                    reason: format!("{}:{}", self.wins[0], self.wins[1]),
                });
            }
        }
    }

    fn on_bp(&mut self, player: u8, action: BpAction, chart: u32, is_ban_msg: bool) {
        let Phase::Bp {
            actor,
            action: want,
            decider,
        } = self.phase
        else {
            self.players[player as usize].as_ref().map(|p| {
                p.send(ServerMsg::Error {
                    message: "现在不是 BP 阶段".into(),
                })
            });
            return;
        };
        let expect_ban = matches!(want, BpAction::Ban);
        if actor != player || expect_ban != is_ban_msg || want != action {
            if let Some(p) = self.players[player as usize].as_ref() {
                p.send(ServerMsg::Error {
                    message: format!("还没轮到你{}", want.name()),
                });
            }
            return;
        }
        if !self.selectable().contains(&chart) {
            if let Some(p) = self.players[player as usize].as_ref() {
                p.send(ServerMsg::Error {
                    message: "这张谱面不在可选范围里".into(),
                });
            }
            return;
        }
        let who = player;
        let picked = format!("玩家{} {}了谱面 {chart}", who + 1, want.name());
        self.log(picked.clone());
        match want {
            BpAction::Ban => {
                self.banned.push(chart);
                self.broadcast(ServerMsg::Notice { message: picked });
            }
            BpAction::Pick => {
                if decider {
                    self.decider = Some(chart);
                } else {
                    self.picks.push(chart);
                }
                self.broadcast(ServerMsg::Notice { message: picked });
            }
        }
        if decider {
            self.start_round();
            return;
        }
        let next = match (self.picks.len(), self.banned.len()) {
            // 0 ban → 1 ban → 2 ban → 1 pick → 2 pick
            (0, 1) => Phase::Bp {
                actor: 1,
                action: BpAction::Ban,
                decider: false,
            },
            (0, 2) => Phase::Bp {
                actor: 0,
                action: BpAction::Pick,
                decider: false,
            },
            (1, _) => Phase::Bp {
                actor: 1,
                action: BpAction::Pick,
                decider: false,
            },
            (2, _) => {
                self.broadcast(ServerMsg::BpDone {
                    chart_a: self.picks[0],
                    chart_b: self.picks[1],
                });
                self.round = 0;
                self.start_round();
                return;
            }
            _ => Phase::Bp {
                actor: 1,
                action: BpAction::Ban,
                decider: false,
            },
        };
        self.phase = next;
        self.announce_bp_turn();
    }

    fn on_progress(&mut self, player: u8, score: u32, accuracy: f32, combo: u32, energy: f32) {
        let Some(p) = self.players[player as usize].as_mut() else { return };
        p.energy = energy.clamp(0., self.cfg.energy_max);
        p.score = score;
        p.accuracy = accuracy;
        let msg = ServerMsg::PeerProgress {
            player,
            score,
            accuracy,
            combo,
            energy: p.energy,
        };
        self.to_other(player, msg);
    }

    fn on_cast(&mut self, player: u8, kind: Interference) {
        let cfg = self.cfg.clone();
        let now = now_ms();
        let Phase::Round { started, .. } = self.phase else {
            if let Some(p) = self.players[player as usize].as_ref() {
                p.send(ServerMsg::CastDenied {
                    kind,
                    reason: "现在不在对局中".into(),
                });
            }
            return;
        };
        let Some(p) = self.players[player as usize].as_mut() else { return };
        if now < started + cfg.cast_lock_ms {
            let left = (started + cfg.cast_lock_ms - now) as f32 / 1000.;
            p.send(ServerMsg::CastDenied {
                kind,
                reason: format!("开局 {left:.0} 秒内不能投干扰"),
            });
            return;
        }
        if p.energy + f32::EPSILON < kind.cost() {
            p.send(ServerMsg::CastDenied {
                kind,
                reason: format!("能量不够（需要 {:.0}，你现在 {:.0}）", kind.cost(), p.energy),
            });
            return;
        }
        let last = p.cooldown[kind.index()];
        if now < last + cfg.cooldown_ms {
            let left = (last + cfg.cooldown_ms - now) as f32 / 1000.;
            p.send(ServerMsg::CastDenied {
                kind,
                reason: format!("{}还在冷却（{left:.1}s）", kind.name()),
            });
            return;
        }
        p.energy -= kind.cost();
        p.cooldown[kind.index()] = now;
        let energy = p.energy;
        p.send(ServerMsg::CastOk { kind, energy });
        self.log(format!("玩家{} 对玩家{} 使用了{}（消耗 {:.0} 能量）", player + 1, 2 - player, kind.name(), kind.cost()));
        self.to_other(
            player,
            ServerMsg::Attacked {
                kind,
                from: player,
                duration_ms: kind.duration_ms(),
            },
        );
    }

    fn on_finish(&mut self, player: u8, score: u32, accuracy: f32, max_combo: u32) {
        if let Some(p) = self.players[player as usize].as_mut() {
            p.finished = true;
            p.score = score;
            p.accuracy = accuracy;
            p.max_combo = max_combo;
        }
        if self.players.iter().flatten().all(|p| p.finished) {
            self.finish_round();
        }
    }

    fn on_disconnect(&mut self, player: u8) {
        let name = self.players[player as usize].as_ref().map(|p| p.name.clone()).unwrap_or_default();
        self.players[player as usize] = None;
        self.log(format!("玩家{}（{name}）离开了", player + 1));
        match self.phase {
            Phase::Lobby => {
                self.broadcast(ServerMsg::PeerLeft { player, name });
            }
            Phase::Done => {}
            _ => {
                // 对局中途掉线：直接判另一方获胜，别让剩下的人干等
                let winner = if player == 0 { 1u8 } else { 0u8 };
                self.phase = Phase::Done;
                self.broadcast(ServerMsg::MatchEnd {
                    winner,
                    scores: self.outcomes.clone(),
                    reason: format!("{name} 掉线了"),
                });
            }
        }
        // 人都走光了就把房间清空：房间码、曲池、比分全部重置，
        // 下一组人可以用新的房间码开新的一局（不然房间码会一直锁在第一个值上）。
        if self.players.iter().all(|it| it.is_none()) {
            self.reset();
        }
    }

    fn reset(&mut self) {
        self.code = None;
        self.pool.clear();
        self.banned.clear();
        self.picks.clear();
        self.decider = None;
        self.played.clear();
        self.outcomes.clear();
        self.wins = [0; SLOTS];
        self.round = 0;
        self.phase = Phase::Lobby;
    }
}

/// 跑服务端（自带 listener 的便捷入口）。
pub async fn serve(listener: TcpListener, cfg: ServerConfig) -> anyhow::Result<()> {
    let duel = Arc::new(Duel::new(cfg));
    duel.serve(listener).await
}

async fn handle(stream: TcpStream, room: Arc<Mutex<Room>>) -> anyhow::Result<()> {
    stream.set_nodelay(true).ok();
    let (mut rd, mut wr) = stream.into_split();
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMsg>();

    // 第一条消息必须是 Hello。说得出话但不是对战协议时，**必须在开服器日志里留痕** ——
    // 最常见的误操作就是把「多人游戏服务器」填成了对战端口（那要填 phira-mp 服务器），
    // 不留痕的话用户只会看到"连接失败/鉴权超时"，完全不知道为什么。
    let (code, name, charts) = match read_msg::<_, ClientMsg>(&mut rd).await {
        Ok(Some(ClientMsg::Hello { room, name, charts })) => (room, name, charts),
        Ok(Some(_)) => {
            room.lock()
                .await
                .log("有人连进来但第一条消息不是对战协议 —— 检查是不是把「多人游戏服务器」填成了对战端口（那个要填 phira-mp 服务器，例如 mp2.phira.cn:12345）");
            write_msg(
                &mut wr,
                &ServerMsg::Error {
                    message: "这不是对战协议：请用 App 的「对战」入口，别用「多人游戏」连这个端口".into(),
                },
            )
            .await?;
            return Ok(());
        }
        Ok(None) => return Ok(()),
        Err(err) => {
            room.lock().await.log(format!(
                "有人连进来但读不出对战协议（{err}）—— 检查是不是把「多人游戏服务器」填成了对战端口"
            ));
            return Ok(());
        }
    };

    let slot;
    {
        let mut room = room.lock().await;
        if let Some(existing) = &room.code {
            if existing != &code {
                write_msg(
                    &mut wr,
                    &ServerMsg::Error {
                        message: format!("房间码不对（现在是 {existing}）"),
                    },
                )
                .await?;
                return Ok(());
            }
        } else {
            room.code = Some(code.clone());
        }
        let Some(free) = room.players.iter().position(|it| it.is_none()) else {
            write_msg(
                &mut wr,
                &ServerMsg::Error {
                    message: "房间满了（v1 一台服务器同时只开一桌）".into(),
                },
            )
            .await?;
            return Ok(());
        };
        slot = free as u8;
        let chart_count = charts.len();
        room.players[free] = Some(Player::new(name.clone(), charts, tx.clone()));
        room.log(format!("玩家{}（{name}）进房，带 {chart_count} 张谱面", slot + 1));
        let players = room.players.iter().flatten().map(|p| PlayerInfo { name: p.name.clone() }).collect();
        room.players[free].as_ref().map(|p| {
            p.send(ServerMsg::Welcome {
                you: slot,
                room: code.clone(),
                players,
            })
        });
        room.to_other(
            slot,
            ServerMsg::PeerJoined {
                player: slot,
                name: name.clone(),
            },
        );
        if room.both_present() {
            room.begin_bp();
        }
    }

    // 循环包在一个 async 块里，这样里面可以随便用 `?`；
    // **无论怎么退出（正常断开 / 读写出错 / 协议错），下面那句 on_disconnect 都会执行**。
    // 早先直接在循环里 `?`，客户端被 RST（Windows 上是 os error 10053）时会在
    // 这里提前 return，玩家永远留在房间里：房间不复位、后来的人用新房间码进不来。
    let outcome: anyhow::Result<()> = async {
        loop {
            tokio::select! {
                incoming = read_msg::<_, ClientMsg>(&mut rd) => {
                    let Some(msg) = incoming? else { break };
                    let mut room = room.lock().await;
                    match msg {
                        ClientMsg::Hello { .. } => {
                            if let Some(p) = room.players[slot as usize].as_ref() {
                                p.send(ServerMsg::Error { message: "你已经进房了".into() });
                            }
                        }
                        ClientMsg::Ban { chart } => room.on_bp(slot, BpAction::Ban, chart, true),
                        ClientMsg::Pick { chart } => room.on_bp(slot, BpAction::Pick, chart, false),
                        ClientMsg::Progress { score, accuracy, combo, energy } => room.on_progress(slot, score, accuracy, combo, energy),
                        ClientMsg::Cast { kind } => room.on_cast(slot, kind),
                        ClientMsg::Finish { score, accuracy, max_combo, counts: _ } => room.on_finish(slot, score, accuracy, max_combo),
                        ClientMsg::Ping { at } => {
                            if let Some(p) = room.players[slot as usize].as_ref() {
                                p.send(ServerMsg::Pong { at });
                            }
                        }
                        ClientMsg::Leave => break,
                    }
                }
                outgoing = rx.recv() => {
                    let Some(msg) = outgoing else { break };
                    write_msg(&mut wr, &msg).await?;
                }
            }
        }
        Ok(())
    }
    .await;

    if let Err(err) = outcome {
        eprintln!("[duel] 连接结束：{err}");
    }
    room.lock().await.on_disconnect(slot);
    Ok(())
}

/// 便捷入口：绑一个端口然后开跑，返回实际监听地址（测试里用 :0 拿随机端口）。
pub async fn serve_on(addr: &str, cfg: ServerConfig) -> anyhow::Result<std::net::SocketAddr> {
    let listener = TcpListener::bind(addr).await?;
    let local = listener.local_addr()?;
    let duel = Arc::new(Duel::new(cfg));
    tokio::spawn(async move {
        let _ = duel.serve(listener).await;
    });
    Ok(local)
}

/// 房间码生成：4 位大写字母数字，避开容易看错的字符
pub fn make_room_code(seed: u64) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut n = seed;
    let mut out = String::with_capacity(4);
    for _ in 0..4 {
        n = n.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        out.push(ALPHABET[(n >> 33) as usize % ALPHABET.len()] as char);
    }
    out
}

/// 给界面用的一眼可见的房间摘要
pub fn room_summary(players: &[PlayerInfo]) -> String {
    let names: Vec<String> = players.iter().map(|p| p.name.clone()).collect();
    format!("{} 人：{}", names.len(), names.join(" vs "))
}
