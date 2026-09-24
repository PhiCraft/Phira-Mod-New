//! 协议层：客户端与服务端共用的消息定义 + 帧格式。
//!
//! 帧格式刻意做得极简：`[u32 小端长度][JSON]`。
//! 为什么不压成二进制：这个协议每局只传几十~几百条消息（触摸不上传、谱面不上传），
//! JSON 的开销完全无所谓，换来的是**排错时能直接看懂抓包内容**——这在这个阶段
//! 比省字节重要得多。

use anyhow::bail;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// 单帧上限（防止对方发个荒唐长度把内存吃光）
pub const MAX_FRAME: u32 = 1 << 20;

/// 一局对战里可以投放的干扰。
///
/// v1 **只做纯视觉干扰**：遮视线、抖判定线、噪点、压暗。
/// 「变速」「左右反转」这类会动到判定/时间轴的干扰先不做——它们要改
/// `TimeManager` / 判定坐标，出错就是整局判定错乱，风险远大于收益，
/// 等基础玩法跑顺了再单独评估。
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum Interference {
    /// 迷雾：判定线附近叠一层雾，挡视线
    Fog,
    /// 抖线：判定线渲染时叠加晃动
    Shake,
    /// 噪点：屏幕叠一层噪点条纹
    Noise,
    /// 压暗：整体压暗
    Dim,
}

impl Interference {
    pub const ALL: [Interference; 4] = [Self::Fog, Self::Shake, Self::Noise, Self::Dim];

    /// 能量消耗
    pub fn cost(self) -> f32 {
        match self {
            Self::Fog => 30.,
            Self::Shake => 35.,
            Self::Noise => 25.,
            Self::Dim => 40.,
        }
    }

    /// 持续时间（毫秒）
    pub fn duration_ms(self) -> u64 {
        match self {
            Self::Fog => 2500,
            Self::Shake => 2500,
            Self::Noise => 2000,
            Self::Dim => 2500,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Fog => "迷雾",
            Self::Shake => "抖线",
            Self::Noise => "噪点",
            Self::Dim => "压暗",
        }
    }

    pub fn index(self) -> usize {
        match self {
            Self::Fog => 0,
            Self::Shake => 1,
            Self::Noise => 2,
            Self::Dim => 3,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BpAction {
    Ban,
    Pick,
}

impl BpAction {
    pub fn name(self) -> &'static str {
        match self {
            Self::Ban => "禁用",
            Self::Pick => "选曲",
        }
    }
}

/// 回合结束后服务端给的「下一步」
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum NextStep {
    /// 进入下一局（索引）
    Round { index: u8 },
    /// 回到 BP：由 `actor` 再选一张（1:1 时的决胜局）
    Bp { actor: u8 },
    /// 比赛结束
    End,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PlayerInfo {
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RoundScore {
    pub player: u8,
    pub score: u32,
    pub accuracy: f32,
    pub max_combo: u32,
}

// ---------------------------------------------------------------- 客户端 → 服务端
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "t", rename_all = "camelCase")]
pub enum ClientMsg {
    /// 进房。`charts` 是本地拥有的谱面 ID（服务端用它求双方交集当曲池）
    Hello {
        room: String,
        name: String,
        charts: Vec<u32>,
    },
    Ban {
        chart: u32,
    },
    Pick {
        chart: u32,
    },
    /// 打歌过程中的进度上报（分数 / 准度 / 连击 / 能量）
    Progress {
        score: u32,
        accuracy: f32,
        combo: u32,
        energy: f32,
    },
    Cast {
        kind: Interference,
    },
    Finish {
        score: u32,
        accuracy: f32,
        max_combo: u32,
        counts: [u32; 4],
    },
    Ping {
        at: u64,
    },
    Leave,
}

// ---------------------------------------------------------------- 服务端 → 客户端
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "t", rename_all = "camelCase")]
pub enum ServerMsg {
    Welcome {
        you: u8,
        room: String,
        players: Vec<PlayerInfo>,
    },
    PeerJoined {
        player: u8,
        name: String,
    },
    PeerLeft {
        player: u8,
        name: String,
    },
    /// 双方曲池交集
    Pool {
        charts: Vec<u32>,
    },
    /// 轮到谁 ban / pick
    BpTurn {
        actor: u8,
        action: BpAction,
        pool: Vec<u32>,
        decider: bool,
    },
    /// BP 结束：`chart_a` 是第一局（A 选的），`chart_b` 是第二局（B 选的）
    BpDone {
        chart_a: u32,
        chart_b: u32,
    },
    /// 开局：`starts_in_ms` 之后正式开始（相对时间，不需要对表）
    RoundStart {
        index: u8,
        chart: u32,
        starts_in_ms: u64,
    },
    /// 对手的进度（用来显示实时分差）
    PeerProgress {
        player: u8,
        score: u32,
        accuracy: f32,
        combo: u32,
        energy: f32,
    },
    CastOk {
        kind: Interference,
        energy: f32,
    },
    CastDenied {
        kind: Interference,
        reason: String,
    },
    /// 你被干扰了（服务端只发给被打的那一方）
    Attacked {
        kind: Interference,
        from: u8,
        duration_ms: u64,
    },
    RoundEnd {
        index: u8,
        winner: Option<u8>,
        scores: Vec<RoundScore>,
        next: NextStep,
    },
    MatchEnd {
        winner: u8,
        scores: Vec<RoundScore>,
        reason: String,
    },
    /// 提示信息（谁 ban 了哪张、谁投了干扰之类），不是错误
    Notice {
        message: String,
    },
    Error {
        message: String,
    },
    Pong {
        at: u64,
    },
}

// ---------------------------------------------------------------- 帧读写
pub async fn write_msg<W, T>(w: &mut W, msg: &T) -> anyhow::Result<()>
where
    W: AsyncWrite + Unpin,
    T: Serialize,
{
    let data = serde_json::to_vec(msg)?;
    if data.len() as u32 > MAX_FRAME {
        bail!("帧太大：{} 字节", data.len());
    }
    w.write_all(&(data.len() as u32).to_le_bytes()).await?;
    w.write_all(&data).await?;
    w.flush().await?;
    Ok(())
}

/// 读一帧；对端正常关闭返回 `Ok(None)`
pub async fn read_msg<R, T>(r: &mut R) -> anyhow::Result<Option<T>>
where
    R: AsyncRead + Unpin,
    T: DeserializeOwned,
{
    let mut len = [0u8; 4];
    match r.read_exact(&mut len).await {
        Ok(_) => {}
        Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(err) => return Err(err.into()),
    }
    let n = u32::from_le_bytes(len);
    if n > MAX_FRAME {
        bail!("对方发来的帧长度离谱：{n}");
    }
    let mut buf = vec![0u8; n as usize];
    r.read_exact(&mut buf).await?;
    Ok(Some(serde_json::from_slice(&buf)?))
}
