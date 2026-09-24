# Phira 0.8.2 修改版（phira-mod）

基于 **[TeamFlos/phira](https://github.com/TeamFlos/phira) 0.8.2** 的修改版。
以 **GPL-3.0-only** 发布（见 [`LICENSE`](LICENSE)，保留上游版权声明）。

> 上游原本的 README 已归档到 [`docs/upstream-README.md`](docs/upstream-README.md)。

## 加了什么

**打歌（游戏内）**

- 判定窗口可分别设置（Perfect / Good / Bad），另有 late 侧补偿（默认 0，完全对称判定）
- 判定文字：PERFECT / GOOD / BAD / MISS，四种出现动画、字号可调，配色带同色外发光
- 漏键标记：Bad / Miss 的位置在判定线上留一个淡出的叉
- 音符拖影（每个音符最多 255 个残影）、判定线残影、判定线发光
- 音乐可视化：判定线上按频谱跳动的条（自写 FFT，无额外依赖）
- 全屏判定、黄色 / 红色键保护
- 实时 HUD：ACC / 判定计数 / 最大连击 / 时间误差 / 预估 RKS / 进度 / FPS，九宫格定位
- Early / Late 实时提示、摇一摇再玩、触点标记（样式 / 颜色 / 透明度可调）
- 打歌界面字体可切换（原版 Phigros 字体 ↔ Phi-Recorder 使用的字体）
- 暂停界面可直接调判定偏移与音乐 / 音效音量

**界面**

- 成绩历史页（趋势图、判定分布、PB 对比、导出 / 导入 JSON）
- 每日挑战（按日期生成，带约束条件）
- 随机选歌、上传成绩开关（默认关，开启前需同意免责协议）

**`launcher/`：Windows 联机开服器（Tauri + Rust，单文件 exe）**

- 自动挑出**稳定的公网 IPv6**（避开隐私扩展的临时地址）、免费动态域名（dynv6）自动更新
- Windows 防火墙入站规则一键配置、四项自检（IPv6 / 防火墙 / 监听 / 域名）
- 内嵌联机服务端，界面实时显示房间状态与事件流
- 磨砂玻璃界面、深浅主题、二维码分享

## 编译

```bash
# 客户端（Windows / Linux / macOS 桌面）
cargo build -p phira-main --no-default-features
#   不加 --no-default-features 需要 FFmpeg 静态库（谱面背景视频），
#   没有 FFmpeg 就用上面这条，代价是放不了背景视频。

# Android（需要 NDK）
cargo check -p phira --no-default-features --target aarch64-linux-android

# iOS：用 .github/workflows/ios.yml，或 Xcode 打开 phira.xcodeproj（需要 macOS）
```

本仓库的 `rust-toolchain.toml` 钉了 nightly；**本地开发用 `cargo +stable` 更省事**
（stable 通常已经装了 android / ios 目标，不用等 nightly 下载）。

## 资源

`assets/` 里只带**字体**（`font.ttf` = Source Han Sans + Saira + Noto，均为 OFL 系）。
**图片与音效请自行从官方客户端提取** —— 官方发布的 APK 里就是这套资源，
CI 的 `.github/workflows/android.yml` 中 `Prepare runtime assets` 步骤做的就是这件事。

运行时必须有 `assets/font.ttf` 和 `assets/background.jpg`，缺任何一个会启动即退出
（CI 里缺 `background.jpg` 时用 `abstract.jpg` 顶替）。

> 授权说明：`assets/` 中的图片、音效以及 `phigros.ttf` 均来自官方客户端，
> 版权归 Phira 官方所有，本项目仅出于兼容性就地使用，**请勿再分发**。
> 发行请走 CI（构建时自动补齐），不要把它们提交进仓库。

## 打包与发布

- `.github/workflows/android.yml` —— 手动触发（Actions → Android → Run workflow），
  ubuntu runner，产出 APK artifact
- `.github/workflows/ios.yml` —— 手动触发，或用 `v*` tag 自动触发，macOS runner，
  产出**无签名 IPA**（用 AltStore / Sideloadly 自签安装）
- 详细步骤见 [`发布到GitHub.md`](发布到GitHub.md)，发布前检查清单见 [`开源步骤.md`](开源步骤.md)

## 联机

开服器在 `launcher/`。玩家侧用本修改版的 App：**设置 → 多人游戏服务器** 填开服器上
显示的地址即可（**客户端不需要任何修改**）。完整教程：
[`launcher/联机使用教程.txt`](launcher/联机使用教程.txt)
（跨网络的三种方案：公网 IPv6 直连 / Tailscale / 主机补公网 IPv4 也都写在里面）。

## 已知问题

- 联机部分只在开发机上验证过服务端逻辑与「客户端能连上」，**没有做过完整的真机双人联机测试**
- 桌面版不带 `video` feature 时无法播放谱面背景视频
- 自定义界面字体若缺少中文字形会回退到内置字体（不会出现豆腐块）

## 致谢

- [TeamFlos/phira](https://github.com/TeamFlos/phira) —— 上游本体
- [TeamFlos/phira-mp](https://github.com/TeamFlos/phira-mp) —— 联机协议设计参考
- [Mivik/prpr](https://github.com/Mivik/prpr)、[Mivik/sasa](https://github.com/Mivik/sasa)、
  [Mivik/prpr-macroquad](https://github.com/Mivik/prpr-macroquad) —— 引擎与音频层
- Source Han Sans / Noto / Saira —— OFL 字体

## 许可证

GPL-3.0-only，见 [`LICENSE`](LICENSE)。本仓库是上游的衍生作品，
**二次分发必须同样以 GPL-3.0 开源并保留上游版权声明**。
