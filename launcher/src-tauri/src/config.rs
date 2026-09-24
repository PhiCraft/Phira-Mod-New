//! 开服器的配置：跟着 exe 走（portable），方便整包丢给别人。

use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::Mutex,
};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    /// 服务端监听端口。默认 12346（官方是 12345，错开一点避免和别的实例撞车）。
    pub port: u16,
    /// dynv6 上注册的域名，例如 `myphira.dynv6.net`。
    pub domain: String,
    /// dynv6 的 token。
    pub dns_token: String,
    /// 每 3 分钟自动检查公网 IPv6 前缀有没有变，变了就推到 DNS。
    pub ddns_enabled: bool,
    /// 开服时自动加 Windows 防火墙入站规则（需要管理员权限）。
    pub auto_firewall: bool,
    /// 上次推送到 DNS 的地址，用来判断"前缀变了没"。
    pub last_pushed: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: 12346,
            domain: String::new(),
            dns_token: String::new(),
            ddns_enabled: true,
            auto_firewall: true,
            last_pushed: String::new(),
        }
    }
}

/// 配置文件的落点：exe 同级目录（绿色版），取不到就退回当前目录。
pub fn config_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|it| it.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("launcher.json")
}

pub struct Store {
    path: PathBuf,
    inner: Mutex<Config>,
}

impl Store {
    pub fn load() -> Self {
        let path = config_path();
        let mut warning = None;
        let cfg = match std::fs::read(&path) {
            Ok(bytes) => {
                // 去掉 UTF-8 BOM：PowerShell 的 `Set-Content -Encoding utf8`（5.1）
                // 和记事本都会带 BOM，带 BOM 时 serde 会整份解析失败，
                // 然后**静默退回默认值**——用户改了设置却没生效，最难查的一类问题。
                let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes);
                match serde_json::from_slice::<Config>(bytes) {
                    Ok(cfg) => cfg,
                    Err(err) => {
                        warning = Some(format!("配置文件解析失败（{err}），已退回默认值：{}", path.display()));
                        Config::default()
                    }
                }
            }
            Err(_) => Config::default(),
        };
        if let Some(text) = &warning {
            // 这时还没有 AppHandle，写文件日志就够了
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path.with_file_name("launcher.log"))
            {
                use std::io::Write;
                let _ = writeln!(file, "[warn] {text}");
            }
        }
        Self { path, inner: Mutex::new(cfg) }
    }

    pub fn get(&self) -> Config {
        self.inner.lock().unwrap().clone()
    }

    pub fn set(&self, cfg: Config) -> anyhow::Result<()> {
        {
            let mut guard = self.inner.lock().unwrap();
            *guard = cfg.clone();
        }
        let text = serde_json::to_vec_pretty(&cfg)?;
        std::fs::write(&self.path, text)?;
        Ok(())
    }

    /// 只改一个字段并落盘（自动 DDNS 推完之后记 last_pushed 用）。
    pub fn patch(&self, f: impl FnOnce(&mut Config)) {
        let mut cfg = self.get();
        f(&mut cfg);
        let _ = self.set(cfg);
    }
}
