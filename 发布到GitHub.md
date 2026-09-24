# 发布到 GitHub（逐项步骤）

这份文档写给**第一次发布的人**。跟着做就行，每一步都有具体点哪里、敲什么命令。
下面假设你发布的是**已经准备好的干净仓库**（`publish/phira-mod/src`，已经 `git init`
并提交过一次，分支是 `main`）。

---

## 0. 先准备好这些东西

| 需要 | 怎么弄 |
|---|---|
| Git for Windows | <https://git-scm.com/download/win>，装完在 PowerShell 里 `git --version` 能出版本号 |
| GitHub 账号 | <https://github.com/signup>，**邮箱必须验证**（不验证不能建仓库） |
| 推送凭据 | 二选一：**个人访问令牌（PAT）** 或 **SSH 密钥**，见第 2 节 |

> 我没有你的 GitHub 凭据，所以**建仓库和 push 这两步必须你自己做**。
> 本地那一步（初始化、提交）我已经做完了。

---

## 1. 在 GitHub 上建仓库

1. 右上角 **+** → **New repository**
2. 逐项填：
   - **Repository name**：`phira-mod`（或你喜欢的名字，和本地目录名无关）
   - **Description**：例如 `Phira 0.8.2 修改版：判定窗口可调、打击表现、成绩历史、每日挑战、联机开服器`
   - **Public / Private**：要开源就选 **Public**
   - ⚠️ **Add a README file / Add .gitignore / Choose a license 这三个都不要勾** ——
     本地已经有 `README.md`、`.gitignore`、`LICENSE` 了，勾了会产生冲突，第一次 push 会报错
3. 点 **Create repository**
4. 建完会看到一个空仓库页面，上面有 "…or push an existing repository from the command line"
   —— 我们要的就是那段命令（第 3 节）

---

## 2. 配置推送凭据（二选一）

### 方式 A：个人访问令牌 PAT（推荐，最简单）

1. GitHub 右上角头像 → **Settings**
2. 左侧最下面 → **Developer settings**
3. **Personal access tokens** → **Tokens (classic)** → **Generate new token (classic)**
4. 填：
   - Note：`phira-mod push`
   - Expiration：90 天（或自定义；过期后要重新生成）
   - 勾选 **repo**（整项打勾即可，包含读写仓库）
5. 点 **Generate token**，**立刻复制**那串 `ghp_...`（离开页面就再也看不到）
6. push 时用户名填你的 GitHub 用户名，**密码处粘贴这个 token**（不是账号密码）

Windows 上第一次 push 会弹凭据窗口，粘贴一次之后系统会记住。

### 方式 B：SSH 密钥（更省心，但步骤多一点）

```powershell
ssh-keygen -t ed25519 -C "你的邮箱"        # 一路回车即可
Get-Content $env:USERPROFILE\.ssh\id_ed25519.pub   # 复制输出
```

GitHub → 头像 → **Settings** → **SSH and GPG keys** → **New SSH key** → 粘贴 → 保存。
然后测一下：`ssh -T git@github.com`，出现 "Hi 用户名!" 就成了。
用这种方式时，仓库地址要用 `git@github.com:用户名/仓库名.git`。

---

## 3. 推送（在本地仓库目录里执行）

```powershell
cd E:\Code\Rust\Phigros\publish\phira-mod\src

git remote add origin https://github.com/你的用户名/phira-mod.git
git branch -M main          # 已经是 main，这句只是保险
git push -u origin main
```

推完刷新 GitHub 页面，应该能看到全部代码。**第一次 push 内容较多（约 15 MB），耐心等一会儿。**

---

## 4. 推送完立刻核对这几件事

- [ ] 仓库里**没有** `target/` 目录（占了就会几十上百 MB；本仓库 `.gitignore` 已挡住）
- [ ] 仓库里**没有** `launcher.json` / `launcher.log`（里面有你的 dynv6 token 和本机日志）
- [ ] 仓库里**没有** `*.jks` / `*.keystore` / `.env`
- [ ] 仓库里**没有** `PingFang` 字体（授权问题，已换成自带的 `font.ttf`）
- [ ] 右侧 **About** 旁边 GitHub 自动识别出 **GPL-3.0 license**（识别不到说明 `LICENSE` 没推上去）
- [ ] `assets/` 里没有官方安装包提取的图片/音效（干净版已排除，见 README 模板）

如果发现推了不该推的：`git rm --cached 文件` 再提交一次（历史里仍有，彻底清理要用
`git filter-repo`；**内容不多的话最省事的办法是删掉仓库重建**）。

---

## 5. 补一个 README（重要）

GitHub 首页就是 README，开源项目没有 README 基本没人看。模板（复制后按需改）：

```markdown
# Phira 0.8.2 修改版

基于 [TeamFlos/phira](https://github.com/TeamFlos/phira) 0.8.2 的修改版，
以 **GPL-3.0-only** 发布（见 `LICENSE`，保留上游版权声明）。

## 加了什么
- 判定窗口可调（Perfect / Good / Bad 单独设置），可选 late 侧补偿
- 打击表现：判定文字、漏键标记、音符拖影、判定线残影与发光、音乐可视化
- 实时 HUD（ACC / 判定计数 / 最大连击 / 时间误差 / RKS 近似 / FPS）
- 成绩历史页、每日挑战、随机选歌
- 打歌界面字体可切换（原版 Phigros 字体 / Phi-Recorder 使用的字体）
- `launcher/`：Windows 联机开服器（IPv6 直连 + 免费动态域名 + 防火墙规则 + 自检）

## 编译
```bash
# 客户端（桌面）
cargo build -p phira-main --no-default-features   # 不带 --no-default-features 需要 FFmpeg
# Android
cargo check -p phira --no-default-features --target aarch64-linux-android
# 开服器（Windows，需 WebView2）
cd launcher/src-tauri && cargo build --release
```
本仓库的 `rust-toolchain` 钉了 nightly，本地用 `cargo +stable` 更省事。

## 资源
`assets/` 里只带字体（Source Han Sans + Saira + Noto，OFL 系）。
**图片与音效请自行从官方客户端提取**（`.github/workflows/android.yml` 里的
`Prepare runtime assets` 步骤就是这么做的），运行时缺 `assets/font.ttf` 与
`assets/background.jpg` 会启动即退出。

## 已知问题
（按实际情况写）
```

顺便在仓库 **About**（右侧齿轮）里填 Description 和 Topics，例如：
`rust` `phira` `rhythm-game` `mod` `tauri`

---

## 6. 打开 CI（自动打包 APK / IPA）

仓库里有两个 workflow，都是**手动触发**（`workflow_dispatch`）：

1. **Actions** 标签页 → 左侧选 **Android** → 右上 **Run workflow** → 选 `main` → Run
   - 跑在 ubuntu，首次约 20–30 分钟。**公开仓库的 Actions 分钟数免费、不计额度**
   - 结束后在这个 workflow 的页面底部 **Artifacts** 里下载 APK
2. 左侧选 **iOS** → Run workflow
   - 跑在 macOS（**公开仓库同样免费**；只有私有仓库才按 10 倍扣每月额度）
   - 产物是**无签名 IPA**，使用者要用 AltStore / Sideloadly 自己签

两个 workflow 都会在构建时**自动补齐缺失资源**（从官方 APK 提取字体和背景图），
所以 `assets/` 里没图片也能出包。

**关于签名**：`android/app/build.gradle` 里的 keystore 口令目前是明文
（`storePassword "phiramod"`，keystore 文件本身没有进仓库）。开源后建议改成从
仓库的 **Settings → Secrets and variables → Actions** 里读，否则别人看到明文口令
会以为可以直接用。

---

## 7. 发版本（Release）

```powershell
git tag v0.8.2-mod
git push origin v0.8.2-mod
```

- **iOS 的 workflow 会在推送 `v*` tag 时自动跑**，并上传无签名 IPA 作为 artifact
- Android 仍要手动 Run workflow
- 然后在 GitHub 的 **Releases** → **Draft a new release** → 选那个 tag →
  写更新说明 → 把 APK / IPA 拖进附件 → Publish
- 安装说明建议一起写清楚：Android 直接装 APK；iOS 用 AltStore/Sideloadly 自签

---

## 8. 以后更新代码的流程

```powershell
# 在开发仓库（phira-0.8.2）改完、验证过之后：
cd E:\Code\Rust\Phigros\phira-0.8.2
git add -A
git commit -m "feat: 说明这次改了什么"
git push origin HEAD          # 开发仓库

# 发布仓库同步（如果两个仓库分开维护）：
cd E:\Code\Rust\Phigros\publish\phira-mod\src
git remote add dev E:\Code\Rust\Phigros\phira-0.8.2   # 只做一次
git fetch dev
git merge dev/main            # 或 git cherry-pick 具体提交
git push origin main
```

> 更简单的做法：**只维护一个仓库**。把 `publish` 目录当成正式仓库，
> 以后都在它里面开发（把开发仓库的改动搬过去），避免两边不同步。

---

## 9. 常见报错对照

| 现象 | 原因 / 解决 |
|---|---|
| `remote: Support for password authentication was removed` | 用了账号密码。改用 PAT（第 2 节方式 A） |
| `remote: Repository not found` | 仓库名/用户名写错，或 token 没有 `repo` 权限 |
| `failed to push some refs (non-fast-forward)` | 远端有本地没有的提交（比如建仓库时勾了 README）。先 `git pull --rebase origin main` 再 push |
| push 卡住或很慢 | 换 SSH（方式 B），或 `git config --global http.postBuffer 524288000` |
| `GH001: Large files detected` / 超过 100MB | 有文件超过 GitHub 单文件上限。检查是不是误提交了 `target/` 或大资源 |
| GitHub 页面看不到 LICENSE 徽标 | `LICENSE` 没推上去，或文件名不是 `LICENSE` |
| CI 报 `assets/font.ttf` 缺失 | 正常，workflow 会自动补；如果补不上会明确失败（不会出坏包） |
