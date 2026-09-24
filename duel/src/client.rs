//! 客户端：一个很薄的异步封装（连接、发、收）。
//!
//! 集成测试用它；之后 App 侧的联机任务也用它——把网络细节收在这一个文件里，
//! 免得协议逻辑散落到界面的 UI 代码里。

use crate::proto::{read_msg, write_msg, ClientMsg, ServerMsg};
use tokio::{
    net::tcp::{OwnedReadHalf, OwnedWriteHalf},
    net::TcpStream,
};

pub struct DuelClient {
    rd: OwnedReadHalf,
    wr: OwnedWriteHalf,
}

impl DuelClient {
    pub async fn connect(addr: impl tokio::net::ToSocketAddrs) -> anyhow::Result<Self> {
        let stream = TcpStream::connect(addr).await?;
        stream.set_nodelay(true).ok();
        let (rd, wr) = stream.into_split();
        Ok(Self { rd, wr })
    }

    pub async fn send(&mut self, msg: &ClientMsg) -> anyhow::Result<()> {
        write_msg(&mut self.wr, msg).await
    }

    pub async fn recv(&mut self) -> anyhow::Result<Option<ServerMsg>> {
        read_msg(&mut self.rd).await
    }

    /// 一直收到满足 `want` 的消息为止（其余消息丢弃）。
    /// 测试和界面里的「等下一个关键事件」都很常用。
    pub async fn recv_until<F>(&mut self, mut want: F, limit: usize) -> anyhow::Result<ServerMsg>
    where
        F: FnMut(&ServerMsg) -> bool,
    {
        for _ in 0..limit {
            let Some(msg) = self.recv().await? else {
                anyhow::bail!("连接在等到目标消息之前就断了");
            };
            if want(&msg) {
                return Ok(msg);
            }
        }
        anyhow::bail!("等了 {limit} 条消息也没等到目标消息")
    }
}
