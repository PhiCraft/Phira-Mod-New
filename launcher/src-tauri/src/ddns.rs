//! dynv6 的 AAAA 记录更新。
//!
//! dynv6 免费、支持 IPv6（AAAA），更新就是一次 GET：
//! `https://dynv6.com/api/update?hostname=<域名>&token=<token>&ipv6=<地址>`
//! 返回体是纯文本：`addresses updated` / `no change` / `invalid token` 之类。

use crate::config::Config;
use serde::Serialize;
use std::{
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub ok: bool,
    pub message: String,
    /// unix 秒，前端自己格式化。
    pub at: u64,
    pub address: String,
    pub domain: String,
    pub manual: bool,
}

#[derive(Default)]
pub struct Ddns {
    last: Mutex<Option<Report>>,
    client: reqwest::Client,
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|it| it.as_secs()).unwrap_or(0)
}

impl Ddns {
    pub fn last(&self) -> Option<Report> {
        self.last.lock().unwrap().clone()
    }

    fn remember(&self, report: Report) -> Report {
        *self.last.lock().unwrap() = Some(report.clone());
        report
    }

    /// 把 `address` 推到 dynv6。`manual` 区分「点刷新」和「后台自动检查」。
    pub async fn push(&self, cfg: &Config, address: &str, manual: bool) -> Report {
        let domain = cfg.domain.trim().to_string();
        if domain.is_empty() || cfg.dns_token.trim().is_empty() {
            return self.remember(Report {
                ok: false,
                message: "还没填 dynv6 域名 / token".into(),
                at: now_secs(),
                address: address.to_string(),
                domain,
                manual,
            });
        }
        let url = format!(
            "https://dynv6.com/api/update?hostname={}&token={}&ipv6={}",
            urlencode(&domain),
            urlencode(cfg.dns_token.trim()),
            urlencode(address)
        );
        let res = self.client.get(&url).send().await;
        let report = match res {
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                let body = body.trim().to_string();
                Report {
                    ok: status.is_success() && !body.to_ascii_lowercase().contains("invalid"),
                    message: if body.is_empty() { format!("HTTP {status}") } else { body },
                    at: now_secs(),
                    address: address.to_string(),
                    domain,
                    manual,
                }
            }
            Err(err) => Report {
                ok: false,
                message: format!("推送失败：{err}"),
                at: now_secs(),
                address: address.to_string(),
                domain,
                manual,
            },
        };
        self.remember(report)
    }
}

fn urlencode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b':' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}
