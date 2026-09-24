#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Phira 联机开服器。
//!
//! 目标形态：家里的 Windows PC 按需开机 → 双击本程序 → 自动确认公网 IPv6、
//! 更新动态域名（dynv6）、加好防火墙放行规则、启动联机服务端，然后把
//! 「域名:端口」和二维码摆出来给别人扫。别人就在 Phira 客户端的
//! 设置 → 联机服务器地址里填这个域名即可。
//!
//! 现在这一步（milestone 1）只做：自检（IPv6 / 防火墙 / 监听）+ 一个最简 TCP
//! 监听器（用来验证外部真的连得进来）+ DDNS 推送 + 界面。服务端逻辑下一步
//! vendor 官方 phira-mp-server 进来替换。

mod config;
mod ddns;
mod net;
mod server;

use std::{sync::Arc, time::Duration};
use tauri::{AppHandle, Emitter, Manager};

pub struct Ctx {
    pub store: config::Store,
    pub log: Arc<server::Log>,
    pub srv: Arc<server::Server>,
    pub ddns: Arc<ddns::Ddns>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct StateOut {
    config: config::Config,
    server: server::Snapshot,
    ddns: Option<ddns::Report>,
    logs: Vec<server::LogLine>,
    /// 记忆里最后一次自检的结论（不含实时探测，实时探测走 self_check）。
    best: Option<String>,
    share: String,
}

#[tauri::command]
async fn get_state(app: AppHandle, ctx: tauri::State<'_, Ctx>) -> Result<StateOut, String> {
    let cfg = ctx.store.get();
    Ok(StateOut {
        server: ctx.srv.snapshot(Some(&app), Some(&ctx.log)).await,
        ddns: ctx.ddns.last(),
        logs: ctx.log.snapshot(),
        best: None,
        share: net::share_target(&cfg.domain, None, cfg.port),
        config: cfg,
    })
}

#[tauri::command]
async fn save_config(ctx: tauri::State<'_, Ctx>, config: config::Config) -> Result<(), String> {
    ctx.store.set(config).map_err(|err| err.to_string())
}

#[tauri::command]
async fn self_check(app: AppHandle, ctx: tauri::State<'_, Ctx>) -> Result<net::SelfCheck, String> {
    let cfg = ctx.store.get();
    let log = ctx.log.clone();
    log.push(&app, "info", "开始自检…");

    let addresses = net::public_ipv6().await.map_err(|err| err.to_string())?;
    let best = net::best_address(&addresses).map(|it| it.address.clone());
    match &best {
        Some(addr) => log.push(&app, "ok", format!("公网 IPv6：{addr}")),
        None => log.push(
            &app,
            "warn",
            "没找到公网 IPv6 —— 先确认光猫开了 IPv6、且运营商给了 240e:/2408: 开头的地址",
        ),
    }

    let name = net::firewall_rule_name(cfg.port);
    let firewall = net::firewall_rule_exists(&name).await;
    log.push(
        &app,
        if firewall { "ok" } else { "warn" },
        if firewall {
            format!("防火墙规则已存在：{name}")
        } else {
            format!("缺少防火墙入站规则：{name}（点「加防火墙规则」需要管理员权限）")
        },
    );

    let listening = ctx.srv.is_running();
    Ok(net::SelfCheck {
        share: net::share_target(&cfg.domain, best.as_deref(), cfg.port),
        best,
        firewall,
        firewall_name: name,
        listening,
        addresses,
        port: cfg.port,
        domain: cfg.domain.clone(),
    })
}

#[tauri::command]
async fn add_firewall(app: AppHandle, ctx: tauri::State<'_, Ctx>) -> Result<String, String> {
    let cfg = ctx.store.get();
    let name = net::firewall_rule_name(cfg.port);
    let (ok, out) = net::firewall_add(&name, cfg.port).await.map_err(|err| err.to_string())?;
    ctx.log.push(&app, if ok { "ok" } else { "warn" }, format!("netsh: {out}"));
    Ok(out)
}

#[tauri::command]
async fn remove_firewall(app: AppHandle, ctx: tauri::State<'_, Ctx>) -> Result<String, String> {
    let cfg = ctx.store.get();
    let name = net::firewall_rule_name(cfg.port);
    let (ok, out) = net::firewall_remove(&name).await.map_err(|err| err.to_string())?;
    ctx.log.push(&app, if ok { "ok" } else { "warn" }, format!("netsh: {out}"));
    Ok(out)
}

/// 开服：需要的话补防火墙规则，然后起监听。
///
/// 抽成函数是为了让「命令行 --autostart」和界面上的「启动服务」按钮走同一条路径
/// —— 自测（包括我这边没法点界面的场景）和自动启动都用它。
async fn boot_server(app: &AppHandle, ctx: &Ctx, port: u16) -> anyhow::Result<()> {
    if ctx.store.get().auto_firewall {
        let name = net::firewall_rule_name(port);
        if !net::firewall_rule_exists(&name).await {
            match net::firewall_add(&name, port).await {
                Ok((true, _)) => ctx.log.push(app, "ok", "已自动加上防火墙入站规则"),
                Ok((false, out)) => ctx
                    .log
                    .push(app, "warn", format!("防火墙规则没加上（需要管理员权限）：{out}")),
                Err(err) => ctx.log.push(app, "warn", format!("防火墙规则没加上：{err}")),
            }
        }
    }
    ctx.srv.start(app.clone(), ctx.log.clone(), port).await
}

#[tauri::command]
async fn start_server(app: AppHandle, ctx: tauri::State<'_, Ctx>, port: u16) -> Result<(), String> {
    {
        let mut cfg = ctx.store.get();
        cfg.port = port;
        ctx.store.set(cfg).map_err(|err| err.to_string())?;
    }
    boot_server(&app, &ctx, port).await.map_err(|err| err.to_string())
}

#[tauri::command]
fn stop_server(app: AppHandle, ctx: tauri::State<'_, Ctx>) {
    ctx.srv.stop(&app, &ctx.log);
}

/// 手动刷新：重新挑地址 + 立刻推 DNS。
#[tauri::command]
async fn refresh_ddns(app: AppHandle, ctx: tauri::State<'_, Ctx>) -> Result<ddns::Report, String> {
    let cfg = ctx.store.get();
    let addresses = net::public_ipv6().await.map_err(|err| err.to_string())?;
    let Some(addr) = net::best_address(&addresses).map(|it| it.address.clone()) else {
        let report = ddns::Report {
            ok: false,
            message: "没找到公网 IPv6 地址，先解决这个再谈推 DNS".into(),
            at: 0,
            address: String::new(),
            domain: cfg.domain.clone(),
            manual: true,
        };
        ctx.log.push(&app, "warn", report.message.clone());
        return Ok(report);
    };
    let report = ctx.ddns.push(&cfg, &addr, true).await;
    ctx.log.push(
        &app,
        if report.ok { "ok" } else { "warn" },
        format!("DDNS 推送 {addr} → {}：{}", report.domain, report.message),
    );
    if report.ok {
        ctx.store.patch(|it| it.last_pushed = addr);
    }
    Ok(report)
}

/// 二维码（SVG 字符串，前端直接 innerHTML 塞进去）。
#[tauri::command]
fn qr_svg(text: String) -> Result<String, String> {
    let code = qrcode::QrCode::new(text.as_bytes()).map_err(|err| err.to_string())?;
    Ok(code
        .render::<qrcode::render::svg::Color>()
        .min_dimensions(240, 240)
        .dark_color(qrcode::render::svg::Color("#0b0e14"))
        .light_color(qrcode::render::svg::Color("#ffffff"))
        .build())
}

/// 让前端也能写进 launcher.log。
///
/// 界面里的 `log()` 只是往页面上追加一行，不进文件；前端出的问题（脚本报错、
/// 玻璃折射支不支持）只有走这里才会留下痕迹——工具是要发给别人的，
/// 出问题时能翻日志比什么都重要。
#[tauri::command]
fn ui_log(app: AppHandle, ctx: tauri::State<'_, Ctx>, level: String, text: String) {
    ctx.log.push(&app, &level, format!("[UI] {text}"));
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            app.manage(Ctx {
                store: config::Store::load(),
                log: Arc::new(server::Log::default()),
                srv: Arc::new(server::Server::default()),
                ddns: Arc::new(ddns::Ddns::default()),
            });

            // 后台自动 DDNS：每 3 分钟看一次前缀变了没，变了就推。
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut ticker = tokio::time::interval(Duration::from_secs(180));
                ticker.tick().await; // 第一次立刻返回，跳过
                loop {
                    ticker.tick().await;
                    let ctx = handle.state::<Ctx>();
                    let cfg = ctx.store.get();
                    if !cfg.ddns_enabled || cfg.domain.trim().is_empty() {
                        continue;
                    }
                    let log = ctx.log.clone();
                    let ddns = ctx.ddns.clone();
                    let Ok(list) = net::public_ipv6().await else { continue };
                    let Some(addr) = net::best_address(&list).map(|it| it.address.clone()) else { continue };
                    if addr == cfg.last_pushed {
                        continue;
                    }
                    log.push(&handle, "info", format!("检测到 IPv6 变成 {addr}，自动推 DNS"));
                    let report = ddns.push(&cfg, &addr, false).await;
                    log.push(&handle, if report.ok { "ok" } else { "warn" }, format!("自动 DDNS：{}", report.message));
                    if report.ok {
                        ctx.store.patch(|it| it.last_pushed = addr);
                        let _ = handle.emit("ddns", ());
                    }
                }
            });
            // 命令行 `--autostart`：起界面后自动开服（开机自启 / 无人值守自测用）
            if std::env::args().any(|it| it == "--autostart") {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(400)).await;
                    let ctx = handle.state::<Ctx>();
                    let port = ctx.store.get().port;
                    ctx.log.push(&handle, "info", format!("--autostart：自动开服（端口 {port}）"));
                    if let Err(err) = boot_server(&handle, &ctx, port).await {
                        ctx.log.push(&handle, "err", format!("自动开服失败：{err}"));
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            save_config,
            self_check,
            add_firewall,
            remove_firewall,
            start_server,
            stop_server,
            refresh_ddns,
            qr_svg,
            ui_log
        ])
        .run(tauri::generate_context!())
        .expect("failed to run launcher");
}
