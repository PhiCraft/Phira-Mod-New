// Phira 联机开服器 —— 前端逻辑
//   1) 液态玻璃：给每个 .glass 生成 feDisplacementMap 位移图做边缘折射（渐进增强）
//   2) 开服器本体：调 Rust 命令、渲染状态、日志
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const $ = (id) => document.getElementById(id);
let cfg = null;
let lastShare = "";
let busy = false;
/// 已经显示到哪一条日志（后端日志是带序号的增量流）
let lastSeq = -1;

function fmtTime(secs) {
  if (!secs) return "—";
  const d = new Date(secs * 1000);
  const p = (n) => String(n).padStart(2, "0");
  return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}

function escapeHtml(s) {
  return String(s ?? "").replace(/[&<>"']/g, (m) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[m]));
}

function log(level, text) {
  appendLog({ at: Date.now() / 1000, level, text });
}

function setBusy(btn, on) {
  if (!btn) return;
  btn.classList.toggle("busy", on);
  if (on) btn.dataset.wasDisabled = btn.disabled;
  btn.disabled = on ? true : btn.dataset.wasDisabled === "true";
}

async function copyText(text, hintEl) {
  if (!text) return;
  try {
    await navigator.clipboard.writeText(text);
    log("ok", `已复制：${text}`);
    if (hintEl) {
      const old = hintEl.dataset.orig ?? hintEl.textContent;
      hintEl.dataset.orig = old;
      hintEl.textContent = "已复制 ✓";
      setTimeout(() => (hintEl.textContent = hintEl.dataset.orig), 1200);
    }
  } catch {
    log("warn", "复制失败，手动选中复制一下");
  }
}

/* ============================================================
   三、渲染
   ============================================================ */
function setCheck(index, state, detail) {
  const li = $("checklist").children[index];
  if (!li) return;
  li.className = `check ${state}`;
  li.querySelector(".mark").textContent = state === "ok" ? "✓" : state === "warn" ? "!" : state === "err" ? "✕" : "·";
  li.querySelector("em").textContent = detail;
}

function renderServer(snapshot) {
  const running = snapshot.running;
  $("status-pill").className = `pill ${running ? "on" : "off"}`;
  $("status-text").textContent = running ? `运行中 · ${snapshot.port}` : "未启动";
  $("btn-start").disabled = running || busy;
  $("btn-stop").disabled = !running || busy;

  // 对战房间
  const room = snapshot.room ?? {};
  const players = room.players ?? [];
  $("room-code").textContent = room.code || "—";
  $("room-phase").textContent = running ? room.phase || "等待玩家" : "未开服";
  $("room-players").textContent = players.length
    ? players.map((p, i) => `${i + 1}. ${p}${room.energies?.[i] ? ` (能量 ${Math.round(room.energies[i])})` : ""}`).join("　")
    : "还没人进来";
  $("room-score").textContent = players.length === 2 ? `${room.wins?.[0] ?? 0} : ${room.wins?.[1] ?? 0}` : "—";
  const pool = room.pool ?? [];
  $("room-pool").textContent = pool.length
    ? `${pool.length} 张共同谱面${room.banned?.length ? `（已 ban ${room.banned.length}）` : ""}${room.picks?.length ? `（已选 ${room.picks.length}）` : ""}`
    : players.length === 2
      ? "没有共同谱面！双方都得有同一张谱面才能开"
      : "—";
  $("room-note").textContent = running
    ? room.bpTurn
      ? `轮到 ${room.bpTurn}`
      : room.phase === "对局中"
        ? `第 ${(room.round ?? 0) + 1} 局进行中`
        : "等两位玩家进来后自动开始 BP 选曲。"
    : "开服后这里会实时显示房间状态。";

  setCheck(2, running ? "ok" : "warn", running ? `服务端在跑（端口 ${snapshot.port}）` : "服务未启动");
}

function renderAddrs(addresses) {
  const list = $("addrs");
  if (!addresses || !addresses.length) {
    list.innerHTML = `<li class="empty">没找到路由器通告 / DHCPv6 派发的 IPv6</li>`;
    return;
  }
  list.innerHTML = addresses
    .map((a) => {
      const tag = a.gua ? "公网" : "非公网";
      const life = a.valid > 0 ? `存活 ${Math.round(a.valid / 3600)}h` : "存活 —";
      return `<li class="${a.gua ? "gua" : ""}" data-addr="${escapeHtml(a.address)}">${escapeHtml(a.address)} · ${tag} · ${escapeHtml(a.iface)} · ${life}</li>`;
    })
    .join("");
}

function renderDdns(report) {
  if (!report) {
    setCheck(3, "pending", "还没推送过（填好域名和 token 后点「立刻推 DNS」）");
    return;
  }
  setCheck(3, report.ok ? "ok" : "warn", `${fmtTime(report.at)} · ${report.domain || "未填域名"} · ${report.message}`);
}

function renderConfig(c) {
  cfg = c;
  $("in-port").value = c.port;
  $("in-domain").value = c.domain;
  $("in-token").value = c.dnsToken;
  $("in-ddns").checked = c.ddnsEnabled;
  $("in-fw").checked = c.autoFirewall;
  if (!lastShare) $("share-addr").textContent = c.domain ? `${c.domain}:${c.port}` : "—";
}

function appendLog(line) {
  const box = $("log");
  const div = document.createElement("div");
  div.innerHTML = `<span class="t">${fmtTime(line.at)}</span> <span class="${line.level}">${escapeHtml(line.text)}</span>`;
  box.appendChild(div);
  while (box.childElementCount > 400) box.removeChild(box.firstChild);
  box.scrollTop = box.scrollHeight;
  if (typeof line.seq === "number" && line.seq > lastSeq) lastSeq = line.seq;
}

/** 只追加比 lastSeq 新的行（轮询用，避免整块重绘导致滚动位置乱跳） */
function appendNewLogs(lines) {
  for (const line of lines) {
    if (typeof line.seq === "number" && line.seq > lastSeq) appendLog(line);
  }
}

function setShare(text, note) {
  lastShare = text;
  $("share-addr").textContent = text || "—";
  $("share-note").textContent = note || "";
}

/* ============================================================
   四、数据流
   ============================================================ */
async function refresh() {
  const state = await invoke("get_state");
  renderConfig(state.config);
  renderServer(state.server);
  renderDdns(state.ddns);
  const box = $("log");
  box.innerHTML = "";
  lastSeq = -1;
  state.logs.forEach(appendLog);
}

async function collectConfig() {
  const port = Math.max(1, Math.min(65535, parseInt($("in-port").value || "12346", 10)));
  return {
    port,
    domain: $("in-domain").value.trim(),
    dnsToken: $("in-token").value,
    ddnsEnabled: $("in-ddns").checked,
    autoFirewall: $("in-fw").checked,
    lastPushed: cfg?.lastPushed ?? "",
  };
}

async function saveConfig() {
  const next = await collectConfig();
  await invoke("save_config", { config: next });
  cfg = next;
  $("in-port").value = next.port;
  return next;
}

async function action(btn, fn, busyText) {
  if (busy) return;
  busy = true;
  const pill = $("status-text");
  const pillOld = pill.textContent;
  if (busyText) pill.textContent = busyText;
  setBusy(btn, true);
  try {
    await fn();
  } catch (err) {
    log("err", String(err));
  } finally {
    busy = false;
    setBusy(btn, false);
    await refresh();
    if (pill.textContent === busyText) pill.textContent = pillOld;
    syncButtons();
  }
}

function syncButtons() {
  const running = $("status-pill").classList.contains("on");
  $("btn-start").disabled = running || busy;
  $("btn-stop").disabled = !running || busy;
}

/* ============================================================
   五、事件
   ============================================================ */
$("btn-start").addEventListener("click", () =>
  action(
    $("btn-start"),
    async () => {
      const next = await saveConfig();
      await invoke("start_server", { port: next.port });
    },
    "启动中…",
  ),
);

$("btn-stop").addEventListener("click", () => action($("btn-stop"), () => invoke("stop_server"), "停止中…"));

$("btn-check").addEventListener("click", () =>
  action($("btn-check"), async () => {
    const r = await invoke("self_check");
    setCheck(0, r.best ? "ok" : "warn", r.best ? `公网地址 ${r.best}` : "没有公网 IPv6：检查光猫是否开了 IPv6、运营商是否给了 240e:/2408: 地址");
    setCheck(1, r.firewall ? "ok" : "warn", r.firewall ? r.firewallName : "缺少入站规则（点下面按钮加，需要管理员权限）");
    setCheck(2, r.listening ? "ok" : "warn", r.listening ? `正在监听端口 ${r.port}` : "服务未启动");
    renderAddrs(r.addresses);
    setShare(
      r.share,
      r.domain ? "域名固定，别人只要填一次；开机就有，关机时对方会连不上。" : "还没填域名：现在分享的是裸 IPv6 地址，重启 / 切网后会变。",
    );
  }),
);

$("btn-fw").addEventListener("click", () =>
  action($("btn-fw"), async () => {
    const out = await invoke("add_firewall");
    log("info", out || "netsh 无输出");
    const r = await invoke("self_check");
    setCheck(1, r.firewall ? "ok" : "warn", r.firewall ? r.firewallName : "还是缺少入站规则（要用管理员身份运行）");
  }),
);

$("btn-ddns").addEventListener("click", () =>
  action($("btn-ddns"), async () => {
    const next = await saveConfig();
    const report = await invoke("refresh_ddns");
    renderDdns(report);
    if (report.ok && report.address) setShare(`${report.domain}:${next.port}`, `已把 ${report.address} 推到 ${report.domain}`);
  }, "推送中…"),
);

$("btn-save").addEventListener("click", async () => {
  const next = await saveConfig();
  log("ok", `设置已保存（端口 ${next.port}）`);
});

$("btn-copy").addEventListener("click", () => copyText(lastShare));
$("btn-qr").addEventListener("click", async () => {
  if (!lastShare || lastShare === "—") return;
  const svg = await invoke("qr_svg", { text: lastShare });
  $("qr-wrap").innerHTML = svg;
  $("qr-wrap").classList.remove("hidden");
});
$("btn-clear").addEventListener("click", () => ($("log").innerHTML = ""));

$("share-addr").addEventListener("click", () => copyText(lastShare, $("share-addr")));
$("addrs").addEventListener("click", (event) => {
  const li = event.target.closest("li[data-addr]");
  if (li) copyText(li.dataset.addr);
});

/* 深浅色切换（记在 localStorage 里，跟着 WebView 的用户目录持久化） */
function applyTheme(theme) {
  document.documentElement.dataset.theme = theme;
  $("btn-theme").textContent = theme === "dark" ? "☾" : "☀";
  try {
    localStorage.setItem("phira-launcher-theme", theme);
  } catch {}
}
$("btn-theme").addEventListener("click", () => {
  applyTheme(document.documentElement.dataset.theme === "dark" ? "light" : "dark");
});
applyTheme(localStorage.getItem("phira-launcher-theme") || "dark");

/* 鼠标反馈：按钮上跟着光标的一点柔光 */
document.addEventListener("pointermove", (event) => {
  const el = event.target.closest?.(".btn");
  if (!el) return;
  const r = el.getBoundingClientRect();
  if (!r.width || !r.height) return;
  el.style.setProperty("--mx", `${((event.clientX - r.left) / r.width) * 100}%`);
  el.style.setProperty("--my", `${((event.clientY - r.top) / r.height) * 100}%`);
});

listen("log", (event) => appendLog(event.payload));
listen("conn", () => invoke("get_state").then((s) => renderServer(s.server)));
listen("ddns", () => invoke("get_state").then((s) => renderDdns(s.ddns)));

// 服务端在跑的时候每秒拉一次房间状态（对战事件也是靠这个增量刷进日志的）
setInterval(() => {
  if (!document.getElementById("status-pill").classList.contains("on")) return;
  invoke("get_state")
    .then((s) => {
      renderServer(s.server);
      appendNewLogs(s.logs);
    })
    .catch(() => {});
}, 1000);

window.addEventListener("DOMContentLoaded", async () => {
  await refresh();
  $("btn-check").click();
});

/* 前端报错也落到 launcher.log，别让界面的问题在文件里没有任何痕迹 */
window.addEventListener("error", (event) => {
  const text = `${event.message} @ ${event.filename}:${event.lineno}`;
  log("err", text);
  invoke("ui_log", { level: "err", text }).catch(() => {});
});
window.addEventListener("unhandledrejection", (event) => {
  const text = `未处理的 Promise 拒绝：${event.reason}`;
  log("err", text);
  invoke("ui_log", { level: "err", text }).catch(() => {});
});
