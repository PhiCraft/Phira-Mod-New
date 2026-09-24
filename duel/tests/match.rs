//! 无头集成测试：在进程里跑服务端，塞两个客户端，把一整场比赛走完。
//!
//! 这是「没有两台设备也能验」的关键：协议错、状态机错、轮次错、能量/冷却判断错，
//! 都会在这里被抓住。真机只剩下「握手延迟」和「干扰手感」需要人肉确认。
//!
//! 脚本：建房 → 双方进场 → 求曲池交集 → BP（ban/ban/pick/pick）→ 第 1 局
//!      → A 投干扰（成功）+ B 投干扰（能量不够，被拒）→ 双方交卷 → A 胜
//!      → 第 2 局（B 选曲）→ B 胜 → 1:1 → 决胜局由胜场少的一方选曲 → 第 3 局
//!      → A 胜 → 比赛结束（A 2:1）

use phira_duel::{BpAction, ClientMsg, DuelClient, Interference, NextStep, ServerConfig, ServerMsg};
use std::time::Duration;

fn cfg() -> ServerConfig {
    ServerConfig {
        // 测试里不等 10 秒，直接允许投干扰
        cast_lock_ms: 0,
        cooldown_ms: 8_000,
        start_delay_ms: 0,
        ..Default::default()
    }
}

async fn join(addr: std::net::SocketAddr, room: &str, name: &str, charts: &[u32]) -> DuelClient {
    let mut c = DuelClient::connect(addr).await.expect("连接服务端");
    c.send(&ClientMsg::Hello {
        room: room.to_string(),
        name: name.to_string(),
        charts: charts.to_vec(),
    })
    .await
    .expect("发送 hello");
    c
}

async fn expect_welcome(c: &mut DuelClient, label: &str) -> u8 {
    match c
        .recv_until(|m| matches!(m, ServerMsg::Welcome { .. }), 10)
        .await
        .unwrap_or_else(|e| panic!("[{label}] 等 welcome 失败：{e}"))
    {
        ServerMsg::Welcome { you, .. } => you,
        _ => unreachable!(),
    }
}

/// 等轮到某个人的某个动作
async fn expect_turn(c: &mut DuelClient, actor: u8, action: BpAction) -> Vec<u32> {
    match c
        .recv_until(|m| matches!(m, ServerMsg::BpTurn { actor: a, action: b, .. } if *a == actor && *b == action), 20)
        .await
        .expect("收到 BP 轮次")
    {
        ServerMsg::BpTurn { pool, .. } => pool,
        _ => unreachable!(),
    }
}

async fn expect_round_start(c: &mut DuelClient) -> (u8, u32) {
    match c.recv_until(|m| matches!(m, ServerMsg::RoundStart { .. }), 20).await.expect("收到开局") {
        ServerMsg::RoundStart { index, chart, .. } => (index, chart),
        _ => unreachable!(),
    }
}

async fn expect_round_end(c: &mut DuelClient) -> (Option<u8>, NextStep) {
    match c.recv_until(|m| matches!(m, ServerMsg::RoundEnd { .. }), 30).await.expect("收到本局结算") {
        ServerMsg::RoundEnd { winner, next, .. } => (winner, next),
        _ => unreachable!(),
    }
}

async fn play_round(a: &mut DuelClient, b: &mut DuelClient, score_a: u32, score_b: u32) {
    // 进度上报（同时也是能量来源）
    a.send(&ClientMsg::Progress {
        score: score_a / 2,
        accuracy: 0.99,
        combo: 100,
        energy: 60.,
    })
    .await
    .unwrap();
    b.send(&ClientMsg::Progress {
        score: score_b / 2,
        accuracy: 0.97,
        combo: 80,
        energy: 20.,
    })
    .await
    .unwrap();
    // 对手能看到进度
    let peer = b.recv_until(|m| matches!(m, ServerMsg::PeerProgress { .. }), 10).await.expect("对手进度");
    assert!(matches!(peer, ServerMsg::PeerProgress { player: 0, .. }), "应该是 0 号玩家的进度");

    a.send(&ClientMsg::Finish {
        score: score_a,
        accuracy: 0.99,
        max_combo: 300,
        counts: [500, 10, 2, 1],
    })
    .await
    .unwrap();
    b.send(&ClientMsg::Finish {
        score: score_b,
        accuracy: 0.97,
        max_combo: 200,
        counts: [400, 50, 20, 40],
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn full_match() {
    let addr = phira_duel::serve_on("127.0.0.1:0", cfg()).await.expect("起服务端");

    // ---- 进房 + 曲池交集 ----
    let mut a = join(addr, "TEST", "小明", &[1, 2, 3, 4, 5, 6]).await;
    let mut b = join(addr, "TEST", "小红", &[4, 5, 6, 7, 8]).await;
    assert_eq!(expect_welcome(&mut a, "A").await, 0, "先来的应该是 0 号位");
    assert_eq!(expect_welcome(&mut b, "B").await, 1, "后来的应该是 1 号位");
    // A 会收到「有人进来了」
    a.recv_until(|m| matches!(m, ServerMsg::PeerJoined { .. }), 10)
        .await
        .expect("收到有人进房");

    let pool = match a.recv_until(|m| matches!(m, ServerMsg::Pool { .. }), 10).await.expect("收到曲池") {
        ServerMsg::Pool { charts } => charts,
        _ => unreachable!(),
    };
    assert_eq!(pool, vec![4, 5, 6], "曲池应该是双方本地曲库的交集");
    b.recv_until(|m| matches!(m, ServerMsg::Pool { .. }), 10).await.expect("B 也收到曲池");

    // ---- BP：A ban / B ban / A pick / B pick ----
    let turn = expect_turn(&mut a, 0, BpAction::Ban).await;
    assert_eq!(turn, vec![4, 5, 6]);
    a.send(&ClientMsg::Ban { chart: 4 }).await.unwrap();

    let turn = expect_turn(&mut b, 1, BpAction::Ban).await;
    assert_eq!(turn, vec![5, 6], "被 ban 掉的不该再出现");
    b.send(&ClientMsg::Ban { chart: 5 }).await.unwrap();

    let turn = expect_turn(&mut a, 0, BpAction::Pick).await;
    assert_eq!(turn, vec![6], "ban 了两张之后只剩一张可 pick");
    a.send(&ClientMsg::Pick { chart: 6 }).await.unwrap();

    // 只剩 6 可以选，B 也只能选它 —— 这时候服务端应该拒绝重复选，
    // 所以这里换成用 `Error` 验证一下拒绝逻辑
    b.recv_until(
        |m| {
            matches!(
                m,
                ServerMsg::BpTurn {
                    actor: 1,
                    action: BpAction::Pick,
                    ..
                }
            )
        },
        10,
    )
    .await
    .expect("轮到 B pick");
    b.send(&ClientMsg::Pick { chart: 6 }).await.unwrap();
    let err = b.recv_until(|m| matches!(m, ServerMsg::Error { .. }), 10).await.expect("重复选曲该被拒");
    assert!(matches!(err, ServerMsg::Error { .. }));

    // 曲池太小走不下去是预期内的（测试曲池只有 3 张），换个更大的曲池重来一遍。
    // 服务端在「人都走光」时会把房间整个重置（房间码、曲池、比分都清），
    // 所以这里等一小会儿让断线处理跑完，再用新房间码进。
    drop(a);
    drop(b);
    tokio::time::sleep(Duration::from_millis(200)).await;

    // ---- 正式一场：池子够大 ----
    let mut a = join(addr, "R2", "小明", &[1, 2, 3, 4, 5, 6, 7, 8]).await;
    let mut b = join(addr, "R2", "小红", &[3, 4, 5, 6, 7, 8, 9, 10]).await;
    assert_eq!(expect_welcome(&mut a, "A").await, 0);
    assert_eq!(expect_welcome(&mut b, "B").await, 1);
    let pool = match a.recv_until(|m| matches!(m, ServerMsg::Pool { .. }), 10).await.unwrap() {
        ServerMsg::Pool { charts } => charts,
        _ => unreachable!(),
    };
    assert_eq!(pool, vec![3, 4, 5, 6, 7, 8]);

    expect_turn(&mut a, 0, BpAction::Ban).await;
    a.send(&ClientMsg::Ban { chart: 3 }).await.unwrap();
    expect_turn(&mut b, 1, BpAction::Ban).await;
    b.send(&ClientMsg::Ban { chart: 4 }).await.unwrap();
    expect_turn(&mut a, 0, BpAction::Pick).await;
    a.send(&ClientMsg::Pick { chart: 5 }).await.unwrap();
    expect_turn(&mut b, 1, BpAction::Pick).await;
    b.send(&ClientMsg::Pick { chart: 6 }).await.unwrap();

    let bp_done = a.recv_until(|m| matches!(m, ServerMsg::BpDone { .. }), 10).await.unwrap();
    match bp_done {
        ServerMsg::BpDone { chart_a, chart_b } => {
            assert_eq!((chart_a, chart_b), (5, 6), "第一局打 A 选的，第二局打 B 选的");
        }
        _ => unreachable!(),
    }

    // ---- 第 1 局 ----
    let (index, chart) = expect_round_start(&mut a).await;
    assert_eq!((index, chart), (0, 5));
    let (index_b, chart_b) = expect_round_start(&mut b).await;
    assert_eq!((index_b, chart_b), (0, 5), "两边必须是同一张谱面");

    // 能量不够时投干扰要被拒
    b.send(&ClientMsg::Cast { kind: Interference::Dim }).await.unwrap();
    let denied = b
        .recv_until(|m| matches!(m, ServerMsg::CastDenied { .. }), 10)
        .await
        .expect("能量不够该被拒");
    assert!(matches!(denied, ServerMsg::CastDenied { kind: Interference::Dim, .. }));

    // 有能量就能投，而且只有对手会收到「你被干扰了」
    a.send(&ClientMsg::Progress {
        score: 100,
        accuracy: 0.99,
        combo: 10,
        energy: 80.,
    })
    .await
    .unwrap();
    b.recv_until(|m| matches!(m, ServerMsg::PeerProgress { player: 0, .. }), 10)
        .await
        .unwrap();
    a.send(&ClientMsg::Cast { kind: Interference::Fog }).await.unwrap();
    let ok = a.recv_until(|m| matches!(m, ServerMsg::CastOk { .. }), 10).await.expect("投干扰成功");
    match ok {
        ServerMsg::CastOk { kind, energy } => {
            assert_eq!(kind, Interference::Fog);
            assert!((energy - 50.).abs() < 0.01, "80 - 30 = 50，实际 {energy}");
        }
        _ => unreachable!(),
    }
    let hit = b
        .recv_until(|m| matches!(m, ServerMsg::Attacked { .. }), 10)
        .await
        .expect("被干扰方收到通知");
    match hit {
        ServerMsg::Attacked { kind, from, duration_ms } => {
            assert_eq!(kind, Interference::Fog);
            assert_eq!(from, 0);
            assert_eq!(duration_ms, Interference::Fog.duration_ms());
        }
        _ => unreachable!(),
    }

    // 同一个干扰马上再投一次 → 冷却中
    a.send(&ClientMsg::Cast { kind: Interference::Fog }).await.unwrap();
    let denied = a.recv_until(|m| matches!(m, ServerMsg::CastDenied { .. }), 10).await.unwrap();
    assert!(matches!(denied, ServerMsg::CastDenied { reason, .. } if reason.contains("冷却")));

    play_round(&mut a, &mut b, 900_000, 800_000).await;
    let (winner, next) = expect_round_end(&mut a).await;
    assert_eq!(winner, Some(0), "分高的赢");
    assert_eq!(next, NextStep::Round { index: 1 });
    expect_round_end(&mut b).await;

    // ---- 第 2 局：B 选的谱面，B 赢 ----
    let (index, chart) = expect_round_start(&mut a).await;
    assert_eq!((index, chart), (1, 6));
    expect_round_start(&mut b).await;
    play_round(&mut a, &mut b, 700_000, 950_000).await;
    let (winner, next) = expect_round_end(&mut a).await;
    assert_eq!(winner, Some(1));
    assert_eq!(next, NextStep::Bp { actor: 0 }, "1:1 → 决胜局由上一局的败者（A 输了第 2 局）选曲");

    // ---- 决胜局：由 BP 指定的那一方 pick ----
    let actor = match next {
        NextStep::Bp { actor } => actor,
        _ => unreachable!(),
    };
    let (picker, other) = if actor == 0 { (&mut a, &mut b) } else { (&mut b, &mut a) };
    let turn = match picker
        .recv_until(|m| matches!(m, ServerMsg::BpTurn { decider: true, .. }), 20)
        .await
        .expect("收到决胜局选曲")
    {
        ServerMsg::BpTurn { pool, .. } => pool,
        _ => unreachable!(),
    };
    assert_eq!(turn, vec![7, 8], "决胜曲从剩下的池子里选");
    picker.send(&ClientMsg::Pick { chart: 7 }).await.unwrap();
    let _ = other;

    let (index, chart) = expect_round_start(&mut a).await;
    assert_eq!((index, chart), (2, 7), "第三局打决胜曲");
    expect_round_start(&mut b).await;
    play_round(&mut a, &mut b, 999_000, 500_000).await;

    let (winner, next) = expect_round_end(&mut a).await;
    assert_eq!(winner, Some(0));
    assert_eq!(next, NextStep::End);

    // ---- 比赛结束 ----
    let end = a.recv_until(|m| matches!(m, ServerMsg::MatchEnd { .. }), 20).await.expect("收到比赛结束");
    match end {
        ServerMsg::MatchEnd { winner, reason, scores } => {
            assert_eq!(winner, 0, "A 拿下第 1、3 局");
            assert_eq!(reason, "2:1");
            assert_eq!(scores.len(), 6, "三局 × 两人 = 6 条成绩");
        }
        _ => unreachable!(),
    }
    let end_b = b.recv_until(|m| matches!(m, ServerMsg::MatchEnd { .. }), 20).await.unwrap();
    assert!(matches!(end_b, ServerMsg::MatchEnd { winner: 0, .. }), "双方拿到的结论必须一致");
}

#[tokio::test]
async fn room_state_for_launcher_ui() {
    // 开服器内嵌时就是这条路：自己绑 listener、自己持句柄、界面轮询 state()
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let duel = std::sync::Arc::new(phira_duel::server::Duel::new(cfg()));
    tokio::spawn({
        let duel = duel.clone();
        async move {
            let _ = duel.serve(listener).await;
        }
    });

    // 空房间
    let state = duel.state().await;
    assert_eq!(state.phase, "等待玩家");
    assert!(state.players.is_empty());
    assert!(duel.is_empty().await);

    // 一个人进来
    let mut a = join(addr, "UI", "甲", &[1, 2, 3, 4]).await;
    expect_welcome(&mut a, "A").await;
    let state = duel.state().await;
    assert_eq!(state.code.as_deref(), Some("UI"));
    assert_eq!(state.players, vec!["甲".to_string()]);
    assert!(state.events.iter().any(|e| e.contains("甲") && e.contains("进房")));

    // 第二个人进来 → 自动开 BP，曲池是交集
    let mut b = join(addr, "UI", "乙", &[3, 4, 5]).await;
    expect_welcome(&mut b, "B").await;
    let state = duel.state().await;
    assert_eq!(state.phase, "BP 选曲");
    assert_eq!(state.players, vec!["甲".to_string(), "乙".to_string()]);
    assert_eq!(state.pool, vec![3, 4]);
    assert_eq!(state.bp_turn.as_deref(), Some("玩家1 禁用"));

    // BP 的第一步走掉之后，状态里能看到 banned。
    // 注意：send() 只是把字节丢进 socket，服务端处理完才算数 ——
    // 所以这里要等一条 Notice 当同步点，不然会读到处理前的状态（异步竞态）。
    a.recv_until(|m| matches!(m, ServerMsg::BpTurn { .. }), 20).await.unwrap();
    a.send(&ClientMsg::Ban { chart: 3 }).await.unwrap();
    a.recv_until(|m| matches!(m, ServerMsg::Notice { .. }), 20).await.expect("等 ban 生效");
    let state = duel.state().await;
    assert_eq!(state.banned, vec![3]);
    assert!(state.events.iter().any(|e| e.contains("禁用")), "事件流里应该能看到这次 ban");
}

#[tokio::test]
async fn rejects_wrong_room_code() {
    let addr = phira_duel::serve_on("127.0.0.1:0", cfg()).await.unwrap();
    let mut a = join(addr, "AAAA", "甲", &[1, 2]).await;
    expect_welcome(&mut a, "A").await;
    let mut b = join(addr, "BBBB", "乙", &[1, 2]).await;
    let err = b.recv_until(|m| matches!(m, ServerMsg::Error { .. }), 5).await.expect("房间码不对该被拒");
    assert!(matches!(err, ServerMsg::Error { message } if message.contains("房间码")));
}

#[tokio::test]
async fn leave_mid_match_ends_it() {
    let addr = phira_duel::serve_on("127.0.0.1:0", cfg()).await.unwrap();
    let mut a = join(addr, "LEAVE", "甲", &[1, 2, 3, 4]).await;
    let mut b = join(addr, "LEAVE", "乙", &[1, 2, 3, 4]).await;
    expect_welcome(&mut a, "A").await;
    expect_welcome(&mut b, "B").await;
    // 走完 BP
    for (c, chart) in [(&mut a, 1u32), (&mut b, 2)] {
        c.recv_until(|m| matches!(m, ServerMsg::BpTurn { .. }), 20).await.unwrap();
        c.send(&ClientMsg::Ban { chart }).await.unwrap();
    }
    for (c, chart) in [(&mut a, 3u32), (&mut b, 4)] {
        c.recv_until(|m| matches!(m, ServerMsg::BpTurn { .. }), 20).await.unwrap();
        c.send(&ClientMsg::Pick { chart }).await.unwrap();
    }
    expect_round_start(&mut a).await;

    // B 掉线 → A 直接判胜
    drop(b);
    let end = tokio::time::timeout(Duration::from_secs(5), a.recv_until(|m| matches!(m, ServerMsg::MatchEnd { .. }), 30))
        .await
        .expect("掉线后应该在 5 秒内结算")
        .expect("收到比赛结束");
    assert!(matches!(end, ServerMsg::MatchEnd { winner: 0, reason, .. } if reason.contains("掉线")));
}
