
label = 设置

general = 通用
audio = 音频
chart = 谱面
debug = 调试
about = 关于

item-lang = 语言
item-fullscreen = 全屏模式
item-offline = 离线模式
item-offline-sub = 在离线模式下将不能上传成绩
item-server-status = 服务器状态
item-server-status-sub = 转到网页查看服务器状态
check-status = 查看
item-mp = 多人游戏
item-mp-sub = 启用多人游戏
item-mp-addr = 多人游戏服务器
item-mp-addr-sub = 服务器地址，'主机:端口'
item-mp-addr-invalid = 无效的服务器地址
item-lowq = 低画质模式
item-lowq-sub = 建议在画面卡顿时启用
item-clear-cache = 清除缓存
item-cache-size-loading = 加载中…
item-cache-size = 缓存大小：{ $size }
item-clear-cache-btn = 清除
item-cache-cleared = 缓存已清除
item-insecure = 不安全模式
item-insecure-sub = 当无法使用在线功能时可尝试该功能。这会使得你的连接不安全！
item-enable-anys = 启用 Anys
item-enable-anys-sub = 使用 Anys 网关以提升连接质量
item-anys-gateway = Anys 网关
item-anys-gateway-sub = Anys 网关地址
item-anys-gateway-invalid = 无效的网关地址

item-adjust = 自动对齐时间
item-adjust-sub = 自动调整延迟以同步音乐和谱面
item-music = 音乐音量
item-sfx = 音效音量
item-bgm = BGM 音量
item-cali = 调整延迟
item-preferred-sample-rate = 首选采样率
preferred-sample-rate-default = 系统默认
item-audio-buffer-size = 音频缓冲区大小

item-show-acc = 显示实时准度
item-show-avg-fps = 显示平均帧率
item-show-avg-fps-sub = 在结算界面显示平均帧率
item-ap-fc-indicator = AP/FC（全完美/全连）指示器
item-ap-fc-indicator-sub = 使用判定线颜色显示全 AP/FC 状态
item-dc-pause = 双击暂停
item-dhint = 双押提示
item-dhint-sub = 同时触线的音符将会被高亮
item-opt = 激进优化
item-opt-sub = 采用激进的优化策略，提升性能但可能导致部分谱面显示出错
item-use-keyboard = 使用键盘游玩
item-use-keyboard-sub = 开启后可以使用键盘进行游戏，但成绩无法上传
item-prefer-reduced-motion = 减少动画效果
item-prefer-reduced-motion-sub = 减少动画和视觉特效
item-judge = 完美判定
item-judge-sub = 完美判定的时间窗口（正负毫秒），默认 ±80ms
item-judge-good = 良好判定
item-judge-good-sub = 良好判定的时间窗口（正负毫秒）；没单独调过时跟随完美判定（2 倍，默认 ±160ms）
item-judge-bad = 失败判定
item-judge-bad-sub = 失败判定的时间窗口（正负毫秒）；没单独调过时跟随完美判定（2.75 倍，默认 ±220ms）
item-drag-protect = 黄键保护
item-drag-protect-sub = 点击（蓝键）不会被叠在附近的黄键（Drag）吃掉
item-flick-protect = 红键保护
item-flick-protect-sub = 点击（蓝键）不会被叠在附近的红键（Flick）吃掉
item-combo-text = 连击文字
item-combo-text-sub = 游戏里连击数下面那行显示的文字（最长 16 个字符）
combo-text-default = 默认
item-line-ref-y-axis = 判定线以 Y 轴为参考
item-line-ref-y-axis-sub = 判定线的长度与厚度按屏幕纵横比换算（官方 PGR 谱面本来就是开着的，这里可以强制打开）
item-debug-line = 判定线调试
item-debug-line-sub = 在每条判定线旁显示编号 / 线高 / z-index，本该隐藏的线以淡影保留（浮点误差大时标红黄）
item-debug-note = 音符调试
item-debug-note-sub = 在音符旁显示线号 / 时间 / 高度 / 类型，并画出它的横向判定范围
item-pause-offset = 暂停界面微调偏移
item-pause-offset-sub = 暂停时显示判定偏移面板（±1ms / ±5ms / 重置），离开对局时自动保存
item-early-late = 太早 / 太晚提示
item-early-late-sub = 游戏内实时显示每次判定的时间误差（EARLY / LATE ±毫秒）
item-judge-chart = 结算判定分布图
item-judge-chart-sub = 结算画面画一张判定时间分布图（早 ← → 晚）
item-health-max = 血条上限
item-health-max-sub = 血条模式的血量上限（10 ~ 10000，默认 100；开局 70%）
item-health-scale = 血条强弱
item-health-scale-sub = 血条模式的回血 / 扣血倍率（默认 1.0x：Perfect +1、Good +0.5、Bad -5、Miss -10）
item-health-color = 血条颜色
item-health-color-sub = 血量条的颜色（点一下换一种）
item-health-len = 血条长度
item-health-len-sub = 血量条占屏幕高度的比例
health-color-white = 白色
health-color-green = 绿色
health-color-blue = 蓝色
health-color-red = 红色
health-color-gold = 金色
health-color-purple = 紫色
item-motion = 摇一摇再玩
item-motion-sub = 设备静止一段时间自动暂停，摇一摇就继续（Android 用加速度计，iOS 用 CoreMotion）
item-motion-threshold = 摇动幅度
item-motion-threshold-sub = 超过这个幅度就算「在摇」（单位 g，越小越灵敏）
item-motion-still = 静止判定时间
item-motion-still-sub = 静止持续多久后自动暂停

# ---- 打击表现 ----
item-in-game-font = 打歌界面字体
item-in-game-font-sub = 打歌文字改用 Phi-Recorder 的字体（Source Han Sans + Saira），字形和它导出的视频一致；关掉用原版 Phigros 字体
item-judge-text = 判定文字
item-judge-text-sub = 显示 PERFECT / GOOD / BAD / MISS
item-judge-text-style = 判定文字动画
item-judge-text-style-sub = 出现方式，点一下切换
judge-text-style-fade = 淡出
judge-text-style-rise = 上浮
judge-text-style-pop = 缩放
judge-text-style-rise-pop = 上浮 + 缩放
item-judge-text-size = 判定文字字号
item-judge-text-size-sub = 判定文字的大小
item-miss-marker = 漏键标记
item-miss-marker-sub = Bad / Miss 的位置在判定线上留一个叉，方便复盘漏在哪
item-miss-marker-time = 漏键标记时长
item-miss-marker-time-sub = 标记停留多久
item-note-trail = 音符拖影
item-note-trail-sub = 音符沿运动方向留下残影
item-note-trail-len = 拖影长度
item-note-trail-len-sub = 残影拖多长
item-note-trail-alpha = 拖影透明度
item-note-trail-alpha-sub = 残影的不透明度
item-note-trail-click = 拖影 · 蓝键
item-note-trail-drag = 拖影 · 黄键
item-note-trail-flick = 拖影 · 红键
item-line-afterimage = 判定线残影
item-line-afterimage-sub = 会动的判定线留下拖尾
item-line-afterimage-count = 残影条数
item-line-afterimage-count-sub = 拖尾由几条组成（越多越顺滑，也越吃性能）
item-line-afterimage-alpha = 残影透明度
item-line-afterimage-alpha-sub = 残影的不透明度
item-line-glow = 判定线发光
item-line-glow-sub = 判定线外圈加一层辉光
item-line-glow-strength = 发光强度
item-line-glow-strength-sub = 辉光的明显程度

# ---- 实时 HUD ----
item-hud = 实时 HUD
item-hud-sub = 打歌时显示实时数据，位置见下面「HUD 位置」
item-hud-acc = HUD · 准确率
item-hud-counts = HUD · 判定计数
item-hud-max-combo = HUD · 最大连击
item-hud-time = HUD · 时间与进度条
item-hud-delta = HUD · 最近偏差
item-hud-rks = HUD · 预估 RKS
item-hud-rks-sub = 近似值：本曲难度 × 当前分数占比
item-hud-fps = HUD · FPS
item-hud-corner = HUD 位置
item-hud-corner-sub = 九宫格位置，点一下换一格
hud-corner-tl = 左上
hud-corner-tc = 上中
hud-corner-tr = 右上
hud-corner-ml = 左中
hud-corner-mc = 居中
hud-corner-mr = 右中
hud-corner-bl = 左下
hud-corner-bc = 下中
hud-corner-br = 右下
item-hud-size = HUD 字号
item-hud-size-sub = HUD 文字大小
item-hud-alpha = HUD 透明度
item-hud-alpha-sub = HUD 的不透明度
item-hud-outline = HUD 描边
item-hud-outline-sub = 给 HUD 加黑描边，亮背景上也看得清
health-max-invalid = 请输入 10 ~ 10000 之间的数字
item-rks = 结算 rks
item-rks-sub = 结算画面显示的 rks（留空 = 跟随账号）
rks-follow = 跟随账号
rks-invalid = 请输入 0 ~ 100 之间的数字
item-challenge-rank = 挑战徽章数字
item-challenge-rank-sub = 结算画面右上角徽章上的数字（0 ~ 999）
challenge-rank-invalid = 请输入 0 ~ 999 之间的整数
item-challenge-color = 挑战徽章颜色
item-challenge-color-sub = 点一下切换：White / Green / Blue / Red / Golden / Rainbow
item-font = 自定义字体
item-font-sub = 导入 ttf/otf 字体文件，结算画面与界面文字都会用它。*重启 App 后生效*
import-font = 导入
import-image = 选择图片
imported = 已导入
reset = 恢复默认
item-font-reset = 恢复默认字体
item-font-reset-sub = 删除已导入的字体，重新使用内置字体。*重启 App 后生效*
font-imported = 字体已导入，重启 App 后生效
font-import-failed = 导入字体失败
font-reset-done = 已恢复默认字体，重启 App 后生效
item-home-char = 主页立绘
item-home-char-sub = 导入一张图片作为主页右侧的人物立绘。*重启 App 后生效*
item-home-char-reset = 恢复默认立绘
item-home-char-reset-sub = 删除已导入的立绘
char-imported = 立绘已导入，重启 App 后生效
char-import-failed = 导入立绘失败
char-reset-done = 已恢复默认立绘，重启 App 后生效
item-reset = 恢复默认设置
item-reset-sub = 把所有设置项恢复为默认值（不影响账号、谱面与存档）
item-reset-confirm = 确定要把所有设置恢复为默认值吗？
reset-done = 设置已恢复默认
item-speed = 速度
item-note-size = 音符大小

item-chart-debug = 谱面调试
item-chart-debug-sub = 显示判定线编号和朝向
item-touch-debug = 触摸调试
item-touch-debug-sub = 游玩过程中显示触摸点

load-cali-failed = 加载音频失败

about-content =
  Phira v{ $version }

  Phira 是一款玩法基于 Phigros 的非商业社区音乐游戏，使用 Rust 开发。

  BiliBili 账号：@Phira官方
  QQ 频道：r48eajexth

  推荐加入 QQ 频道以获取最新消息和获得帮助！

  工作人员名单（字典序）
  开发
  { $development }

  运维
  { $operations }

  文档
  { $documentation }

  美术
  { $art }

  音乐
  { $music }

  音效
  { $audio }

  社区管理
  { $community }

  本地化
  { $localization }

  以及许多志愿谱面审核员！完整列表参见 https://phira.moe/staff
item-history = 成绩历史
item-history-sub = 本地保存的每次游玩记录（列表 / 趋势 / PB 对比 / 判定分布）
item-history-open = 打开
item-late-leniency = 晚按补偿
item-late-leniency-sub = 判定时把「按晚了」的误差减掉这么多；0 = 和早按完全对称（上游默认偷偷给了 70ms，late 会明显偏松）
item-note-trail-count = 残影数量
item-note-trail-count-sub = 每个音符画几层残影（1~255，越多越连续也越吃性能）
item-upload = 上传成绩
item-upload-sub = 【本构建不支持上传】开源构建的上传通道是关的（官方上传要用闭源模块里的成绩编码器），此开关目前无实际作用，成绩只存本机。想要上传请用官方客户端。
item-upload-consent = 成绩上传协议
item-upload-consent-open = 查看
upload-consent-title = 成绩上传：知情同意与免责声明
upload-consent-accept = 我已阅读并同意
upload-consent-deny = 不同意
upload-consent-text =
    开启后，每打完一首官方谱面，会把本局成绩上传到 Phira 官方服务器。

    会上传：谱面 ID、本局成绩数据（分数 / 准确率 / 判定等，官方既有格式）、谱面版本时间戳，
    以及请求头里的账号凭证（用于确认是哪个账号提交）。
    不会上传：设备信息、位置、相册、其它文件；不额外采集任何统计；没有自建服务器，也不发给第三方。

    上传后：成绩进入 Phira 云端排行榜与个人主页，其他玩家可见，RKS / 经验由官方结算。
    关闭后：成绩只存在本机（本地「成绩历史」照常记录），不上榜、不更新云端 RKS。
    已经上传过的旧成绩不会自动删除，需要删除请到 Phira 官网操作。

    免责：本 Mod 是个人自用的非官方改版，与 TeamFlos / Phira 官方没有任何关系。
    本 Mod 修改了判定与表现（判定窗口、全屏判定、血条模式、late 补偿等），其中会显著影响
    分数公平性的项目已标记为 UNRATED、本就不上传；但若你绕开这些限制或组合使用，
    上传的成绩可能与官方客户端不一致，由此产生的后果（成绩被判定异常、被删除、账号被限制等）
    由使用者自行承担，作者不承担任何责任。本 Mod 按现状提供，不附带任何担保。

    你随时可以在设置里关闭这个开关。item-upload-profile = 一键可上传配置
item-upload-profile-sub = 【不支持上传，但本按钮仍会关掉所有不计分 / 影响公平的选项】（自动播放、全屏判定、血条模式、去连击分、变速），判定窗口恢复官方 ±80/160/220、late 补偿归零，并打开成绩上传
item-upload-profile-apply = 一键配置
upload-profile-done = 已切换为可上传成绩配置：关闭了 { $mods } 个不计分 Mod，速度 1.0、判定窗口恢复官方值、late 补偿 0，并已打开成绩上传item-music-spectrum = 音乐可视化
item-music-spectrum-sub = 沿判定线画音频频谱条（对音乐做 FFT，纯本地，吃一点性能）
item-music-spectrum-gain = 频谱放大
item-music-spectrum-gain-sub = 频谱条的灵敏度/高度
item-touch-style = 触点样式
item-touch-style-sub = 触点标记怎么画：圆点 / 命中特效贴图 / 头像
touch-style-circle = 圆点
touch-style-hitfx = 命中特效
touch-style-avatar = 头像
item-touch-color = 触点颜色
item-touch-color-sub = 点一下换一种
touch-color-white = 白色
touch-color-red = 红色
touch-color-green = 绿色
touch-color-blue = 蓝色
touch-color-gold = 金色
touch-color-purple = 紫色
item-touch-alpha = 触点透明度
item-touch-alpha-sub = 触点标记的不透明度，太淡了会看不清
