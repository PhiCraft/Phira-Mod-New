
label = SETTINGS

general = General
audio = Audio
chart = Chart
debug = Debug
about = Info

item-lang = Language
item-fullscreen = Fullscreen Mode
item-offline = Offline Mode
item-offline-sub = Disable all online functionality.
item-server-status = Server Status
item-server-status-sub = Open the server status page in your browser.
check-status = Open
item-mp = Multiplayer
item-mp-sub = Enable multiplayer functionality.
item-mp-addr = Multiplayer Server
item-mp-addr-sub = Connect to a custom multiplayer server.
item-mp-addr-invalid = Invalid server address.
item-lowq = Low Resolution Mode
item-lowq-sub = Lower the quality of the UI, increasing peformance.
item-clear-cache = Clear Cache
item-cache-size-loading = Loading…
item-cache-size = Cache size: { $size }
item-clear-cache-btn = Clear
item-cache-cleared = Cache cleared
item-insecure = Insecure Connection
item-insecure-sub = Enable old devices to use online functionality.
item-enable-anys = Enable Anys
item-enable-anys-sub = Use an Anys gateway to improve network stability.
item-anys-gateway = Anys Gateway
item-anys-gateway-sub = Use a custom Anys gateway address.
item-anys-gateway-invalid = Invalid gateway address.

item-adjust = Automatic Time Adjustment
item-adjust-sub = Adjust the audio and chart offset dynamically.
item-music = Music Volume
item-sfx = SFX Volume
item-bgm = BGM Volume
item-cali = Adjust Offset
item-preferred-sample-rate = Preferred Sample Rate
preferred-sample-rate-default = System Default
item-audio-buffer-size = Audio Buffer Size

item-show-acc = Real-Time Accuracy
item-show-avg-fps = Show AVG FPS
item-show-avg-fps-sub = Display the average FPS on the results screen.
item-ap-fc-indicator = AP/FC Indicator
item-ap-fc-indicator-sub = Use line color to indicate AP/FC status.
item-dc-pause = Double-Tap to Pause
item-dhint = Simultaneous Hint
item-dhint-sub = Highlight notes that are meant to be hit at the same time.
item-opt = Chart Optimization
item-opt-sub = Significantly increase peformance while playing. (If unintended behavior arises, disable this.)
item-use-keyboard = Use Keyboard
item-use-keyboard-sub = Enable keyboard input for gameplay. Scores cannot be uploaded when enabled.
item-prefer-reduced-motion = Prefer Reduced Motion
item-prefer-reduced-motion-sub = Reduce animations and visual effects
item-judge = Perfect Window
item-judge-sub = The time window of a Perfect hit (± milliseconds), ±80ms by default
item-judge-good = Good Window
item-judge-good-sub = The time window of a Good hit (± milliseconds); follows Perfect (×2, ±160ms by default) until you change it here
item-judge-bad = Bad Window
item-judge-bad-sub = The time window of a Bad hit (± milliseconds); follows Perfect (×2.75, ±220ms by default) until you change it here
item-drag-protect = Drag Protection
item-drag-protect-sub = A tap is no longer eaten by an overlapping Drag (yellow) note.
item-flick-protect = Flick Protection
item-flick-protect-sub = A tap is no longer eaten by an overlapping Flick (red) note.
item-combo-text = Combo Label
item-combo-text-sub = The text shown under the combo counter in game (up to 16 characters)
combo-text-default = Default
item-line-ref-y-axis = Judge Line Refers to Y Axis
item-line-ref-y-axis-sub = Compute the judge line's length and thickness from the screen aspect ratio (always on for official PGR charts; this forces it on)
item-debug-line = Line Debug
item-debug-line-sub = Show each judge line's id / height / z-index, keep hidden lines as faint ghosts (red/yellow when floating point error is large)
item-debug-note = Note Debug
item-debug-note-sub = Show each note's line / time / height / kind and draw its horizontal judge range
item-pause-offset = Adjust Offset While Paused
item-pause-offset-sub = Show an offset panel (±1ms / ±5ms / reset) while paused; saved automatically when you leave the play
item-early-late = Early / Late Hint
item-early-late-sub = Show the timing error of every judgement in game (EARLY / LATE ±ms)
item-judge-chart = Result Timing Chart
item-judge-chart-sub = Draw a judgement timing distribution chart (early ← → late) on the result screen
item-health-max = Health Maximum
item-health-max-sub = Maximum health of health mode (10 ~ 10000, 100 by default; you start at 70%)
item-health-scale = Health Strength
item-health-scale-sub = Heal / damage multiplier of health mode (1.0x by default: Perfect +1, Good +0.5, Bad -5, Miss -10)
item-health-color = Health Bar Color
item-health-color-sub = Color of the health bar (tap to cycle)
item-health-len = Health Bar Length
item-health-len-sub = How much of the screen height the health bar takes
health-color-white = White
health-color-green = Green
health-color-blue = Blue
health-color-red = Red
health-color-gold = Gold
health-color-purple = Purple
item-motion = Shake to Play
item-motion-sub = Pause automatically when the device is still, continue when you shake it (accelerometer on Android, CoreMotion on iOS)
item-motion-threshold = Shake Threshold
item-motion-threshold-sub = Motion above this counts as shaking (in g; smaller = more sensitive)
item-motion-still = Still Time
item-motion-still-sub = How long the device must stay still before auto-pausing

# ---- hit feedback ----
item-in-game-font = In-Game Font
item-in-game-font-sub = Use Phi-Recorder's font (Source Han Sans + Saira) for in-game text; off keeps the original Phigros font
item-judge-text = Judgement Text
item-judge-text-sub = Show PERFECT / GOOD / BAD / MISS
item-judge-text-style = Judgement Text Animation
item-judge-text-style-sub = Appearance style, tap to cycle
judge-text-style-fade = Fade
judge-text-style-rise = Rise
judge-text-style-pop = Pop
judge-text-style-rise-pop = Rise + Pop
item-judge-text-size = Judgement Text Size
item-judge-text-size-sub = Size of the judgement text
item-miss-marker = Miss Marker
item-miss-marker-sub = Leaves a cross on the line where a Bad / Miss happened, so you can review it
item-miss-marker-time = Miss Marker Duration
item-miss-marker-time-sub = How long the marker stays
item-note-trail = Note Trail
item-note-trail-sub = Notes leave a trail along their movement direction
item-note-trail-len = Trail Length
item-note-trail-len-sub = How far the trail stretches
item-note-trail-alpha = Trail Opacity
item-note-trail-alpha-sub = Opacity of the trail
item-note-trail-click = Trail · Click
item-note-trail-drag = Trail · Drag
item-note-trail-flick = Trail · Flick
item-line-afterimage = Line Afterimage
item-line-afterimage-sub = Moving judge lines leave a trailing afterimage
item-line-afterimage-count = Afterimage Count
item-line-afterimage-count-sub = How many copies make up the trail (smoother, but heavier)
item-line-afterimage-alpha = Afterimage Opacity
item-line-afterimage-alpha-sub = Opacity of the afterimage
item-line-glow = Line Glow
item-line-glow-sub = Adds a glow around the judge line
item-line-glow-strength = Glow Strength
item-line-glow-strength-sub = How pronounced the glow is

# ---- realtime HUD ----
item-hud = Realtime HUD
item-hud-sub = Shows live stats while playing; position is "HUD Position" below
item-hud-acc = HUD · Accuracy
item-hud-counts = HUD · Judge Counts
item-hud-max-combo = HUD · Max Combo
item-hud-time = HUD · Time & Progress
item-hud-delta = HUD · Latest Offset
item-hud-rks = HUD · Estimated RKS
item-hud-rks-sub = Approximation: chart difficulty × current score ratio
item-hud-fps = HUD · FPS
item-hud-corner = HUD Position
item-hud-corner-sub = 3×3 grid position, tap to cycle
hud-corner-tl = Top Left
hud-corner-tc = Top Center
hud-corner-tr = Top Right
hud-corner-ml = Middle Left
hud-corner-mc = Center
hud-corner-mr = Middle Right
hud-corner-bl = Bottom Left
hud-corner-bc = Bottom Center
hud-corner-br = Bottom Right
item-hud-size = HUD Size
item-hud-size-sub = Size of the HUD text
item-hud-alpha = HUD Opacity
item-hud-alpha-sub = Opacity of the HUD
item-hud-outline = HUD Outline
item-hud-outline-sub = Adds a dark outline so the HUD stays readable on bright backgrounds
health-max-invalid = Please enter a number between 10 and 10000
item-rks = Result RKS
item-rks-sub = The rks shown on the result screen (leave empty to follow your account)
rks-follow = Account
rks-invalid = Please enter a number between 0 and 100
item-challenge-rank = Challenge Badge Number
item-challenge-rank-sub = The number on the badge at the top right of the result screen (0 ~ 999)
challenge-rank-invalid = Please enter an integer between 0 and 999
item-challenge-color = Challenge Badge Color
item-challenge-color-sub = Tap to cycle: White / Green / Blue / Red / Golden / Rainbow
item-font = Custom Font
item-font-sub = Import a ttf/otf font file; the result screen and the UI will use it. *Takes effect after restarting the app*
import-font = Import
import-image = Choose Image
imported = Imported
reset = Reset
item-font-reset = Reset Font
item-font-reset-sub = Delete the imported font and use the built-in one again. *Takes effect after restarting the app*
font-imported = Font imported. Restart the app to apply.
font-import-failed = Failed to import the font
font-reset-done = Font reset. Restart the app to apply.
item-home-char = Home Illustration
item-home-char-sub = Import an image to use as the character illustration on the home page. *Takes effect after restarting the app*
item-home-char-reset = Reset Illustration
item-home-char-reset-sub = Delete the imported illustration
char-imported = Illustration imported. Restart the app to apply.
char-import-failed = Failed to import the illustration
char-reset-done = Illustration reset. Restart the app to apply.
item-reset = Reset Settings
item-reset-sub = Restore every setting to its default value (account, charts and records are untouched)
item-reset-confirm = Reset all settings to their default values?
reset-done = Settings restored to defaults
item-speed = Speed
item-note-size = Note Size

item-chart-debug = Show Line ID
item-chart-debug-sub = Display the IDs and orientation of lines.
item-touch-debug = Show Touch Points
item-touch-debug-sub = Display user touch points.

load-cali-failed = Failed to load calibration audio.

about-content =
  Phira v{ $version }

  Phira is a non-commercial community-driven rhythm game inspired by Phigros.

  BiliBili Account: @Phira官方
  QQ Guild: r48eajexth
  Discord Server: discord.gg/gqpR3bTSsP

  We recommend joining either the QQ guild or the Discord server to get live updates and receive assistance.

  Staff List (sorted lexicographically)
  Development
  { $development }

  Operations
  { $operations }

  Documentation
  { $documentation }

  Art
  { $art }

  Music
  { $music }

  Audio
  { $audio }

  Community Management
  { $community }

  Localization
  { $localization }

  And many more voluntary chart reviewers. For a full list please refer to https://phira.moe/staff .
item-history = Score History
item-history-sub = Every play saved locally (list / trend / PB / judge distribution)
item-history-open = Open
item-late-leniency = Late Leniency
item-late-leniency-sub = Subtracts this much from late hits when judging; 0 = perfectly symmetric with early (upstream silently allowed 70ms, making late hits too forgiving)
item-note-trail-count = Afterimage Count
item-note-trail-count-sub = Layers drawn per note (1-255; smoother but heavier)
item-upload = Upload Scores
item-upload-sub = Upload each play of an official chart to Phira's official server (off by default; when off, scores stay on this device)
item-upload-consent = Score Upload Agreement
item-upload-consent-open = View
upload-consent-title = Score upload: consent and disclaimer
upload-consent-accept = I have read and agree
upload-consent-deny = Disagree
upload-consent-text =
    When enabled, each finished play of an official chart is uploaded to Phira's official server.

    Uploaded: chart ID, this play's result data (score / accuracy / judgements, in the official format),
    chart revision timestamp, and the account credential in the request header (to identify the account).
    Not uploaded: device info, location, photos, other files; no analytics; no server of our own, nothing sent to third parties.

    After upload: the score appears on Phira's leaderboards and your profile (visible to others);
    RKS / EXP are settled by the server.
    When disabled: scores stay local only (the local Score History page still records them),
    no leaderboard entry, no cloud RKS update. Previously uploaded scores are NOT deleted automatically.

    Disclaimer: this Mod is an unofficial personal build, not affiliated with TeamFlos / Phira.
    It changes judging and presentation (judge windows, full-screen judge, health mode, late leniency, ...);
    the ones that clearly affect fairness are marked UNRATED and are already not uploaded. But if you bypass
    those limits or combine them, uploaded results may differ from the official client, and any consequence
    (flagged/deleted scores, restricted account, ...) is your own responsibility; the author takes no liability.
    Provided as-is, without warranty of any kind.

    You can turn this switch off at any time in settings.item-upload-profile = One-click Uploadable Setup
item-upload-profile-sub = Turns off everything unrated / fairness-affecting (autoplay, full-screen judge, health mode, no-combo score, speed change), restores official judge windows (±80/160/220) and zero late leniency, and enables score upload
item-upload-profile-apply = Apply
upload-profile-done = Switched to an uploadable setup: disabled { $mods } unrated mod(s), speed 1.0, official judge windows, late leniency 0, upload enableditem-music-spectrum = Music Visualizer
item-music-spectrum-sub = Draws FFT spectrum bars along the judge line (computed locally; slight perf cost)
item-music-spectrum-gain = Spectrum Gain
item-music-spectrum-gain-sub = Sensitivity / height of the bars
item-touch-style = Touch Marker Style
item-touch-style-sub = How touch points are drawn: dot / hit-effect texture / avatar
touch-style-circle = Dot
touch-style-hitfx = Hit Effect
touch-style-avatar = Avatar
item-touch-color = Touch Marker Color
item-touch-color-sub = Tap to cycle
touch-color-white = White
touch-color-red = Red
touch-color-green = Green
touch-color-blue = Blue
touch-color-gold = Gold
touch-color-purple = Purple
item-touch-alpha = Touch Marker Opacity
item-touch-alpha-sub = Opacity of the touch markers
