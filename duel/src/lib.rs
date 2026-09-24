//! Phira 双人对战：自研协议 + 服务端。
//!
//! 为什么自研而不是 fork `phira-mp`：我们要的玩法（BP 选曲 + 能量制干扰）需要
//! 自己的消息，而 phira-mp 里那些「谱面从官方 API 拉、谱面要传、聊天、锁房、
//! 房主轮换」我们一样都用不上。抛开这些之后协议能小很多：
//! 服务端**不碰谱面和音频**，只交换谱面 ID + 事件。
//!
//! 代价（认下来的）：这套协议只有本修改版的 App 能用，官方服务器和其它客户端
//! 永远用不了。标准联机不受影响——那条路继续走 phira-mp。

pub mod client;
pub mod proto;
pub mod server;

pub use client::DuelClient;
pub use proto::{BpAction, ClientMsg, Interference, NextStep, PlayerInfo, RoundScore, ServerMsg};
pub use server::{make_room_code, room_summary, serve, serve_on, Duel, RoomState, ServerConfig};
