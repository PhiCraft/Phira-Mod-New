//! 网络相关的系统操作：找公网 IPv6、管 Windows 防火墙规则。
//!
//! 这里全部靠系统自带命令（PowerShell / netsh），不引额外依赖：
//! * IPv6 用 `Get-NetIPAddress`——它返回的是**结构化对象**，属性名是英文，
//!   在中文 Windows 上一样能解析（不像 netsh 的输出是本地化文本）；
//! * 脚本用 `-EncodedCommand`（UTF-16LE + base64）传，彻底躲开引号转义问题。

use serde::{Deserialize, Serialize};
use std::{
    net::Ipv6Addr,
    process::Stdio,
};
use tokio::process::Command;

/// 跑系统命令但**不弹黑框**（GUI 程序里必须的）。
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 跑系统命令。**stdout 和 stderr 严格分开**：PowerShell 在非交互场景会把
/// `#< CLIXML ...` 之类的记录写进 stderr，早先把 stderr 拼到 stdout 后面，
/// 结果 JSON 尾部多了一段垃圾、解析直接失败（表现为「没找到公网 IPv6」）。
pub async fn run(program: &str, args: &[&str]) -> anyhow::Result<(i32, String)> {
    let mut cmd = Command::new(program);
    cmd.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let out = cmd.output().await?;
    // PowerShell 5.1 重定向时的输出是控制台代码页，但我们的 JSON 全是 ASCII，
    // 所以按 UTF-8 解也不会坏（真出问题下面还有「按最后一个 ] 截断」的兜底）。
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    Ok((out.status.code().unwrap_or(-1), text))
}

/// 从可能夹着噪声的输出里抠出 JSON：第一个 `[`/`{` 到最后一个 `]`/`}`。
fn extract_json(text: &str) -> Option<&str> {
    let start = text.find(['[', '{'])?;
    let end = text.rfind([']', '}'])?;
    if end <= start {
        return None;
    }
    Some(&text[start..=end])
}

async fn powershell(script: &str) -> anyhow::Result<String> {
    // PowerShell 的 -EncodedCommand 要 UTF-16LE
    let mut bytes = Vec::with_capacity(script.len() * 2);
    for unit in script.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    let encoded = base64(&bytes);
    let (_, out) = run("powershell", &["-NoProfile", "-NonInteractive", "-EncodedCommand", &encoded]).await?;
    Ok(out)
}

/// 不引 base64 crate，自己编一下（就这几行）。
fn base64(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { TABLE[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { TABLE[n as usize & 63] as char } else { '=' });
    }
    out
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct V6Info {
    pub address: String,
    #[serde(default)]
    pub iface: String,
    #[serde(default)]
    pub suffix: String,
    #[serde(default)]
    pub valid: i64,
    #[serde(default)]
    pub preferred: i64,
    #[serde(default)]
    pub state: String,
    /// 我们自己的判断：是不是**公网**（2000::/3）地址。
    #[serde(default)]
    pub gua: bool,
}

/// 只认公网单播 `2000::/3`，顺手排掉回环 / 链路本地 / ULA / v4-mapped。
pub fn is_gua(addr: &Ipv6Addr) -> bool {
    if addr.is_loopback() || addr.is_unspecified() || addr.is_multicast() {
        return false;
    }
    let seg = addr.segments();
    // ::ffff:a.b.c.d —— IPv4 映射，不是真 v6
    if seg[0] == 0 && seg[1] == 0 && seg[2] == 0 && seg[3] == 0 && seg[4] == 0 && seg[5] == 0xffff {
        return false;
    }
    (seg[0] & 0xe000) == 0x2000
}

/// 拿本机所有「路由器通告 / DHCPv6 派发」来的 IPv6 地址，公网的排前面（存活时间长的更靠前）。
///
/// 两个来源都要收：前缀一般是 RA 给的（`RouterAdvertisement`），但有些组网用 DHCPv6，
/// 这时 `PrefixOrigin` 是 `Dhcp`——只认 RA 会在那种网络上误报「没有公网 IPv6」。
///
/// 为什么按存活时间排：Windows 同时存在**稳定地址**和**隐私扩展的临时地址**。
/// 实测（电信 240e）两个地址是这样的：
/// * `…:dcc8:…`（SuffixOrigin = Random，临时地址）ValidLifetime 约 7 天；
/// * `…:8b31:…`（SuffixOrigin = Link，稳定地址）ValidLifetime 是「无限」。
/// 临时地址几小时到几天就换，推到 DNS 上就会失效，所以取 ValidLifetime 最大的那个。
pub async fn public_ipv6() -> anyhow::Result<Vec<V6Info>> {
    let script = r#"
$out = Get-NetIPAddress -AddressFamily IPv6 -ErrorAction SilentlyContinue |
  Where-Object { $_.PrefixOrigin -eq 'RouterAdvertisement' -or $_.PrefixOrigin -eq 'Dhcp' } |
  Select-Object @{n='address';e={$_.IPAddress}},
                @{n='iface';e={[string]$_.InterfaceAlias}},
                @{n='suffix';e={[string]$_.SuffixOrigin}},
                @{n='valid';e={[int64]$_.ValidLifetime.TotalSeconds}},
                @{n='preferred';e={[int64]$_.PreferredLifetime.TotalSeconds}},
                @{n='state';e={[string]$_.AddressState}};
# 注意：必须用 -InputObject，不能走管道 —— 管道会把单元素数组拆成一个对象，
# 那样只有一个地址时返回的是 `{...}` 而不是 `[{...}]`，Rust 侧解析就空了。
if ($out) { ConvertTo-Json -InputObject @($out) -Compress -Depth 3 } else { '[]' }
"#;
    let text = powershell(script).await?;
    let Some(json) = extract_json(&text) else {
        anyhow::bail!("PowerShell 没有返回 JSON，原始输出：{}", text.trim().chars().take(300).collect::<String>());
    };
    let mut list: Vec<V6Info> = serde_json::from_str(json).map_err(|err| {
        anyhow::anyhow!(
            "解析 IPv6 列表失败（{err}），原始输出：{}",
            text.trim().chars().take(300).collect::<String>()
        )
    })?;
    for it in &mut list {
        it.gua = it.address.parse::<Ipv6Addr>().map(|a| is_gua(&a)).unwrap_or(false);
    }
    list.sort_by(|a, b| {
        b.gua
            .cmp(&a.gua)
            .then(b.state.eq_ignore_ascii_case("Preferred").cmp(&a.state.eq_ignore_ascii_case("Preferred")))
            .then(b.valid.cmp(&a.valid))
    });
    Ok(list)
}

/// 挑出该公布的那一个（公网、存活最久）。
pub fn best_address(list: &[V6Info]) -> Option<&V6Info> {
    list.iter().find(|it| it.gua)
}

/// 给客户端填的地址：有域名用域名，否则退回 `[地址]:端口`。
pub fn share_target(domain: &str, address: Option<&str>, port: u16) -> String {
    let host = if !domain.trim().is_empty() {
        domain.trim().to_string()
    } else {
        match address {
            Some(a) => format!("[{a}]"),
            None => "<未检测到公网 IPv6>".to_string(),
        }
    };
    format!("{host}:{port}")
}

pub fn firewall_rule_name(port: u16) -> String {
    format!("Phira Launcher (TCP {port})")
}

/// 防火墙规则在不在。netsh 的输出是本地化文本，所以判据是「退出码 + 我们的规则名
/// （纯 ASCII，中文系统上也会原样出现）」。
pub async fn firewall_rule_exists(name: &str) -> bool {
    match run("netsh", &["advfirewall", "firewall", "show", "rule", &format!("name={name}")]).await {
        Ok((code, out)) => code == 0 && out.contains(name),
        Err(_) => false,
    }
}

/// 加一条入站放行规则（**需要管理员权限**；没有权限时 netsh 会返回非 0，前端会提示）。
pub async fn firewall_add(name: &str, port: u16) -> anyhow::Result<(bool, String)> {
    let port = port.to_string();
    let (code, out) = run(
        "netsh",
        &[
            "advfirewall",
            "firewall",
            "add",
            "rule",
            &format!("name={name}"),
            "dir=in",
            "action=allow",
            "protocol=TCP",
            &format!("localport={port}"),
            "profile=any",
        ],
    )
    .await?;
    Ok((code == 0, out.trim().to_string()))
}

pub async fn firewall_remove(name: &str) -> anyhow::Result<(bool, String)> {
    let (code, out) = run("netsh", &["advfirewall", "firewall", "delete", "rule", &format!("name={name}")]).await?;
    Ok((code == 0, out.trim().to_string()))
}

/// 自检结果，直接喂给界面。
#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct SelfCheck {
    pub addresses: Vec<V6Info>,
    pub best: Option<String>,
    pub firewall: bool,
    pub firewall_name: String,
    pub listening: bool,
    pub port: u16,
    pub domain: String,
    pub share: String,
}
