# Phirc Mod++

基于 [TeamFlos/phira](https://github.com/TeamFlos/phira) 0.8.2 的社区改版。
以 **GPL-3.0-only** 发布，保留上游版权声明（见 `LICENSE`）。

面向想把准度练上去、想看清每一局表现、想和朋友自己开一局联机的玩家。

## 功能

**判定与表现**
- 判定窗口可分别设置（Perfect / Good / Bad），另有 late 侧补偿（默认 0，完全对称）
- 判定文字：四种出现动画、字号可调、带同色外发光
- 漏键标记：Bad / Miss 在判定线上留一个淡出的叉
- 音符拖影（每音符最多 255 残影）、判定线残影、判定线发光
- 音乐可视化：判定线上按频谱跳动的条（自写 FFT，无额外依赖）
- 全屏判定、黄色键 / 红色键保护；摇一摇重新开始
- 触点标记：样式 / 颜色 / 透明度可调

**实时数据**
- 九宫格可定位的 HUD：ACC、判定计数、最大连击、时间误差、预估 RKS、进度、FPS
- Early / Late 实时提示
- 暂停界面可调判定偏移与音乐 / 音效音量

**成绩**
- 成绩历史页：趋势图、判定分布、PB 对比、JSON 导入导出
- 每日挑战
- 官方成绩上传：默认关闭，开启前需同意免责协议；影响公平性的选项不会上传

**其他**
- `launcher/`：Windows 联机开服器（自动挑选稳定公网 IPv6 + 免费动态域名 + 防火墙规则 + 四项自检）
- 界面字体使用 HarmonyOS Sans，也可在设置里导入自定义字体

## 安装

- Android：直接安装 APK。包名 `org.flos.phira.modded`，与官方客户端可共存
- iOS：IPA 未签名，需用 AltStore / Sideloadly 自签
- 资源（字体 / 图片 / 音效）不随仓库分发，请自行从官方客户端提取

## 编译

```bash
# 桌面版（需要 --no-default-features 以避开 FFmpeg 依赖）
cargo +stable build -p phira-main --no-default-features

# Android（需要 NDK）
cargo +stable check -p phira --no-default-features --target aarch64-linux-android

# iOS：Xcode 打开 phira.xcodeproj（需要 macOS）

# 开服器（Windows，需 WebView2）
cd launcher/src-tauri && cargo +stable build --release
```

## 许可证与致谢

GPL-3.0-only，见 `LICENSE`。二次分发必须同样以 GPL-3.0 开源并保留上游版权声明。

- [TeamFlos/phira](https://github.com/TeamFlos/phira) —— 上游本体
- [Mivik/prpr](https://github.com/Mivik/prpr)、[Mivik/sasa](https://github.com/Mivik/sasa)、
  [Mivik/prpr-macroquad](https://github.com/Mivik/prpr-macroquad) —— 引擎与音频层
- Source Han Sans / Noto / Saira —— OFL 字体
- HarmonyOS Sans —— 界面字体

本项目是社区改版，与 Phira 官方无隶属关系；请支持官方与谱师。