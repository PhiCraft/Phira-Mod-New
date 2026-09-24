package org.flos.phira.modded;

import android.app.Activity;
import android.app.AlertDialog;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Context;
import android.content.Intent;
import android.hardware.Sensor;
import android.hardware.SensorEvent;
import android.hardware.SensorEventListener;
import android.hardware.SensorManager;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.os.ParcelFileDescriptor;
import android.util.DisplayMetrics;
import android.util.Log;
import android.view.KeyEvent;
import android.view.MotionEvent;
import android.view.SurfaceHolder;
import android.view.SurfaceView;
import android.view.View;
import android.view.Window;
import android.view.WindowManager;
import android.view.inputmethod.InputMethodManager;

import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;

import moe.mivik.inputbox.InputBox;
import quad_native.QuadNative;

/**
 * Phira（改版）的 Android 外壳。
 *
 * 契约（对着 miniquad 的 java/ 模板、src/native/android.rs 和 phira 自己导出的
 * Java_quad_1native_QuadNative_* 核过）：
 *
 *  Java → Rust（QuadNative 里的 native 方法）：
 *    - miniquad 的 13 个回调（activityOn* / surfaceOn* / initializeContext / releaseContext）
 *    - phira 自己的：initializeEnvironment / setDataPath / setTempDir / setDpi /
 *      setChosenFile / markImport / markImportRespack / setInputText / processExportFd /
 *      prprActivityOn* / startApp
 *    少一个或参数个数不对，Rust 侧不是 NULL 就是垃圾值。
 *
 *  Rust → Java（Rust 用 GetMethodID 按名字找，找不到就是 NULL 然后 abort）：
 *    - finish()         Activity 自带
 *    - getAssets()      Activity 自带
 *    - setFullScreen(boolean)    见下
 *    - showKeyboard(boolean)     见下
 *    - copy(String)              见下（剪贴板）
 *    - openUrl(String)           见下（prpr::ext::open_url，点链接用）
 *    - showExportDialog(String)  见下（谱面导出）
 *    - deleteUri(Uri)            见下（导出结束后清理 SAF 文档）
 *    - chooseFile()              见下（选文件导入）
 *
 *  启动顺序：initializeContext（把 Activity 绑到 ndk_context，读 APK 内 assets 靠它）
 *  → setDataPath / setTempDir / setDpi → initializeEnvironment（inputbox 后端）
 *  → 建 SurfaceView → activityOnCreate。
 */
class QuadSurface extends SurfaceView implements View.OnTouchListener, View.OnKeyListener, SurfaceHolder.Callback {
    public QuadSurface(android.content.Context context) {
        super(context);
        getHolder().addCallback(this);
        setFocusable(true);
        setFocusableInTouchMode(true);
        requestFocus();
        setOnTouchListener(this);
        setOnKeyListener(this);
    }

    @Override
    public void surfaceCreated(SurfaceHolder holder) {
        CrashLog.stage("surfaceCreated");
        QuadNative.surfaceOnSurfaceCreated(holder.getSurface());
    }

    @Override
    public void surfaceDestroyed(SurfaceHolder holder) {
        CrashLog.stage("surfaceDestroyed");
        QuadNative.surfaceOnSurfaceDestroyed(holder.getSurface());
    }

    @Override
    public void surfaceChanged(SurfaceHolder holder, int format, int width, int height) {
        // 游戏线程会一直等这条消息才开始建 EGL 上下文，记一笔方便确认「到底有没有画面」
        CrashLog.stage("surfaceChanged " + width + "x" + height);
        QuadNative.surfaceOnSurfaceChanged(holder.getSurface(), width, height);
    }

    @Override
    public boolean onTouch(View v, MotionEvent event) {
        final int pointerCount = event.getPointerCount();
        final long time = event.getEventTime();
        switch (event.getActionMasked()) {
            case MotionEvent.ACTION_MOVE:
                for (int i = 0; i < pointerCount; i++) {
                    QuadNative.surfaceOnTouch(event.getPointerId(i), 0, event.getX(i), event.getY(i), time);
                }
                break;
            case MotionEvent.ACTION_UP:
                QuadNative.surfaceOnTouch(event.getPointerId(0), 1, event.getX(0), event.getY(0), time);
                break;
            case MotionEvent.ACTION_DOWN:
                QuadNative.surfaceOnTouch(event.getPointerId(0), 2, event.getX(0), event.getY(0), time);
                break;
            case MotionEvent.ACTION_POINTER_UP: {
                final int i = event.getActionIndex();
                QuadNative.surfaceOnTouch(event.getPointerId(i), 1, event.getX(i), event.getY(i), time);
                break;
            }
            case MotionEvent.ACTION_POINTER_DOWN: {
                final int i = event.getActionIndex();
                QuadNative.surfaceOnTouch(event.getPointerId(i), 2, event.getX(i), event.getY(i), time);
                break;
            }
            case MotionEvent.ACTION_CANCEL:
                for (int i = 0; i < pointerCount; i++) {
                    QuadNative.surfaceOnTouch(event.getPointerId(i), 3, event.getX(i), event.getY(i), time);
                }
                break;
            default:
                break;
        }
        return true;
    }

    // getCharacters 已废弃，但非拉丁输入时 KeyEvent 的 keyCode 常常是 0，只能靠它拿字符
    @SuppressWarnings("deprecation")
    @Override
    public boolean onKey(View v, int keyCode, KeyEvent event) {
        if (event.getAction() == KeyEvent.ACTION_DOWN && keyCode != 0) {
            QuadNative.surfaceOnKeyDown(keyCode);
        }
        if (event.getAction() == KeyEvent.ACTION_UP && keyCode != 0) {
            QuadNative.surfaceOnKeyUp(keyCode);
        }
        if (event.getAction() == KeyEvent.ACTION_UP || event.getAction() == KeyEvent.ACTION_MULTIPLE) {
            int character = event.getUnicodeChar();
            if (character == 0) {
                String characters = event.getCharacters();
                if (characters != null && characters.length() > 0) {
                    character = characters.charAt(0);
                }
            }
            if (character != 0) {
                QuadNative.surfaceOnCharacter(character);
            }
        }
        return true;
    }
}

public class MainActivity extends Activity {
    private static final String TAG = "PhiraMod";
    private static final int PICK_FILE = 0x51A1;
    private static final int EXPORT_FILE = 0x51A2;
    /** 弹窗里最多显示多少字符（完整内容可用「复制」拿走）。 */
    private static final int LOG_PREVIEW_CHARS = 4000;

    private QuadSurface view;
    private static String libLoadError = null;
    /** libphira.so 是否已经加载好。加载之前调任何 native 方法都只会抛
     *  UnsatisfiedLinkError —— 而传感器回调每秒几十次，绝不能在库加载前注册：
     *  之前那样做会往日志里灌几百条完整堆栈（每条还要同步写两个文件），
     *  既拖慢启动，又把真正有用的那几行挤出弹窗的预览范围。 */
    private static volatile boolean nativeReady = false;
    /** 游戏主循环是否已经启动（activityOnCreate 返回之后才算）。 */
    private static volatile boolean gameStarted = false;
    /** 同一个 native 方法反复失败时只记第一次，避免把日志刷爆。 */
    private static final java.util.Set<String> reportedFailures =
        java.util.Collections.synchronizedSet(new java.util.HashSet<String>());

    private SensorManager sensorManager;
    private Sensor motionSensor;
    /** 监听器是否在注册状态（unregister 之后还能再注册回来） */
    private boolean motionRegistered = false;
    /** 平滑后的摇动强度（g），避免单帧抖动导致误判 */
    private float motionSmoothed = 0f;
    private final SensorEventListener motionListener = new SensorEventListener() {
        @Override
        public void onSensorChanged(SensorEvent event) {
            float x = event.values[0];
            float y = event.values[1];
            float z = event.values[2];
            float magnitude = (float) Math.sqrt(x * x + y * y + z * z);
            if (event.sensor.getType() == Sensor.TYPE_ACCELEROMETER) {
                // 没有线性加速度传感器时用原始加速度，减掉重力得到近似线性加速度
                magnitude = Math.abs(magnitude - SensorManager.GRAVITY_EARTH);
            }
            motionSmoothed = motionSmoothed * 0.8f + magnitude * 0.2f;
            call("onMotion", () -> QuadNative.onMotion(motionSmoothed / SensorManager.GRAVITY_EARTH));
        }

        @Override
        public void onAccuracyChanged(Sensor sensor, int accuracy) {}
    };

    /**
     * 加载 Rust 的 libphira.so。不要放在 static 块里：静态初始化抛出的异常会变成
     * ExceptionInInitializerError 直接闪退，什么都看不到。
     */
    private static synchronized boolean ensureLibrary() {
        if (libLoadError != null) {
            return false;
        }
        try {
            System.loadLibrary("phira");
            nativeReady = true;
            return true;
        } catch (Throwable t) {
            CrashLog.error("System.loadLibrary(\"phira\") 失败", t);
            libLoadError = t.toString();
            return false;
        }
    }

    /** 调用一个 native 方法，失败就记录下来（不静默吞掉，但同一个方法只记第一次）。 */
    private static void call(String name, Runnable r) {
        try {
            r.run();
        } catch (Throwable t) {
            if (reportedFailures.add(name)) {
                CrashLog.error("native 调用失败: " + name, t);
            }
        }
    }

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        CrashLog.init(getFilesDir(), getExternalFilesDir(null));
        // 输入框（moe.mivik.inputbox.InputBox）需要一个 Activity 才能弹对话框
        InputBox.setActivity(this);

        Thread.setDefaultUncaughtExceptionHandler((thread, throwable) ->
            CrashLog.error("未捕获异常（Java 侧）", throwable));

        requestWindowFeature(Window.FEATURE_NO_TITLE);
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);

        // 先把上次的日志读走并清掉，再开始写本次的 —— 否则读到的就是本次刚写的行，
        // 每次启动都会弹「上次失败了」。
        final String previous = CrashLog.read(getFilesDir());
        CrashLog.clear(getFilesDir());
        CrashLog.stage("onCreate 进入");
        if (CrashLog.looksLikeFailure(previous)) {
            showPreviousLog(previous);
            return;
        }
        startGame();
    }

    /** 显示上次启动留下的日志（没有 adb 也能看到原因）。 */
    private void showPreviousLog(final String log) {
        CrashLog.stage("检测到上次的日志，显示弹窗");
        final String body = log.length() > LOG_PREVIEW_CHARS
            ? log.substring(log.length() - LOG_PREVIEW_CHARS) + "\n…（已截断，点「复制并继续」拿完整内容）"
            : log;
        new AlertDialog.Builder(this)
            .setTitle("上次启动可能失败了")
            .setMessage(body)
            .setPositiveButton("清除并继续", (d, w) -> startGame())
            .setNeutralButton("复制并继续", (d, w) -> {
                copyToClipboard(log);
                startGame();
            })
            .setNegativeButton("退出", (d, w) -> finish())
            .setCancelable(false)
            .show();
    }

    private void copyToClipboard(String text) {
        try {
            ClipboardManager cm = (ClipboardManager) getSystemService(Context.CLIPBOARD_SERVICE);
            if (cm != null) {
                cm.setPrimaryClip(ClipData.newPlainText("phira-crash", text));
            }
        } catch (Throwable t) {
            Log.w(TAG, "copyToClipboard failed", t);
        }
    }

    private void startGame() {
        CrashLog.stage("准备加载 libphira.so");
        if (!ensureLibrary()) {
            new AlertDialog.Builder(this)
                .setTitle("Phira Mod")
                .setMessage("无法加载 libphira.so：\n\n" + libLoadError
                    + "\n\n（常见原因：APK 里缺少 libc++_shared.so、或 ABI 不匹配、或 JNI 符号被裁掉）")
                .setPositiveButton("退出", (d, w) -> finish())
                .setCancelable(false)
                .show();
            return;
        }
        CrashLog.stage("libphira.so 加载成功");
        // 库好了才注册传感器（onResume 那次会因为 nativeReady=false 直接跳过）
        startMotionSensor();

        final MainActivity self = this;

        // 顺序要紧：initializeContext 负责把 Activity 绑到 ndk_context（读 APK 内 assets、
        // 取目录都依赖它），Rust 侧 initialize_raw 又是 .unwrap()，所以它必须最先。
        call("initializeContext", () -> QuadNative.initializeContext(self));
        CrashLog.stage("initializeContext 完成（ndk_context + rustls）");
        call("setDataPath", () -> QuadNative.setDataPath(getFilesDir().getAbsolutePath()));
        call("setTempDir", () -> QuadNative.setTempDir(getCacheDir().getAbsolutePath()));
        call("setDpi", () -> {
            DisplayMetrics dm = getResources().getDisplayMetrics();
            QuadNative.setDpi(dm.densityDpi);
        });
        CrashLog.stage("数据目录 / 临时目录 / DPI 已交给 Rust");
        call("initializeEnvironment", () -> QuadNative.initializeEnvironment());
        CrashLog.stage("initializeEnvironment 完成（inputbox 后端）");

        view = new QuadSurface(this);
        setContentView(view);
        CrashLog.stage("SurfaceView 就绪");

        // miniquad 的 activityOnCreate 会启动游戏（内部起线程后返回，游戏在别的线程跑）
        CrashLog.stage("调用 activityOnCreate");
        call("activityOnCreate", () -> QuadNative.activityOnCreate(self));
        // 走到这里说明 activityOnCreate 已经返回了。正常情况游戏已经在自己的线程上跑；
        // 如果宿主版本没有在 activityOnCreate 里启动，就兜底再来一次（Rust 侧幂等）。
        CrashLog.stage("activityOnCreate 已返回，startApp 兜底（Rust 侧幂等）");
        call("startApp", () -> QuadNative.startApp());
        gameStarted = true;
        CrashLog.stage("startApp 已返回（游戏线程已在运行，或已经结束）");
    }

    @Override
    protected void onResume() {
        super.onResume();
        InputBox.setActivity(this);
        // 库还没加载完就别调 native：onResume 通常早于 startGame（弹「上次日志」时更是
        // 早得多），那时调用只会抛 UnsatisfiedLinkError —— 记进日志的 [error] 会让
        // 下次启动误报「上次启动失败了」，还会把真正的错误淹掉。
        if (nativeReady) {
            call("activityOnResume", () -> QuadNative.activityOnResume());
            call("prprActivityOnResume", () -> QuadNative.prprActivityOnResume());
        }
        startMotionSensor();
    }

    /** 注册加速度计：摇一摇再玩要靠它（引擎在设置里开关）。 */
    private void startMotionSensor() {
        try {
            if (!nativeReady || motionRegistered) {
                // 库还没加载完（onResume 常常早于 startGame）或已经注册过了
                return;
            }
            if (sensorManager == null) {
                sensorManager = (SensorManager) getSystemService(Context.SENSOR_SERVICE);
            }
            if (sensorManager == null) {
                return;
            }
            if (motionSensor == null) {
                // 优先用线性加速度（系统已经去掉重力），没有就退回原始加速度计
                motionSensor = sensorManager.getDefaultSensor(Sensor.TYPE_LINEAR_ACCELERATION);
                if (motionSensor == null) {
                    motionSensor = sensorManager.getDefaultSensor(Sensor.TYPE_ACCELEROMETER);
                }
            }
            if (motionSensor == null) {
                CrashLog.stage("没有可用的加速度计，摇一摇不可用");
                return;
            }
            sensorManager.registerListener(motionListener, motionSensor, SensorManager.SENSOR_DELAY_GAME);
            motionRegistered = true;
            CrashLog.stage("加速度计已注册: " + motionSensor.getName());
        } catch (Throwable t) {
            CrashLog.error("startMotionSensor failed", t);
        }
    }

    private void stopMotionSensor() {
        try {
            if (sensorManager != null && motionSensor != null && motionRegistered) {
                sensorManager.unregisterListener(motionListener, motionSensor);
                motionRegistered = false;
            }
        } catch (Throwable ignored) {
        }
    }

    @Override
    protected void onPause() {
        super.onPause();
        stopMotionSensor();
        // 游戏没启动过就没什么可暂停的（miniquad 的消息通道还不存在）
        if (gameStarted) {
            call("prprActivityOnPause", () -> QuadNative.prprActivityOnPause());
            call("activityOnPause", () -> QuadNative.activityOnPause());
        }
    }

    @Override
    protected void onDestroy() {
        super.onDestroy();
        InputBox.setActivity(null);
        CrashLog.stage("[exit] onDestroy（正常退出）");
        if (gameStarted) {
            call("prprActivityOnDestroy", () -> QuadNative.prprActivityOnDestroy());
            call("activityOnDestroy", () -> QuadNative.activityOnDestroy());
        }
        if (nativeReady) {
            call("releaseContext", () -> QuadNative.releaseContext());
        }
    }

    // ------------------------------------------------------------------
    // 以下方法是 Rust 侧按名字回调的（GetMethodID），缺任何一个都可能直接 abort
    // ------------------------------------------------------------------

    /** miniquad: set_fullscreen() */
    @SuppressWarnings("deprecation")
    public void setFullScreen(final boolean fullscreen) {
        runOnUiThread(() -> {
            try {
                Window window = getWindow();
                View decor = window.getDecorView();
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                    window.setDecorFitsSystemWindows(!fullscreen);
                }
                if (fullscreen) {
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
                        WindowManager.LayoutParams lp = window.getAttributes();
                        lp.layoutInDisplayCutoutMode =
                            WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES;
                        window.setAttributes(lp);
                    }
                    decor.setSystemUiVisibility(
                        View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                            | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
                            | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
                            | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                            | View.SYSTEM_UI_FLAG_FULLSCREEN
                            | View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY);
                } else {
                    decor.setSystemUiVisibility(View.SYSTEM_UI_FLAG_VISIBLE);
                }
            } catch (Throwable t) {
                CrashLog.error("setFullScreen failed", t);
            }
        });
    }

    /** miniquad: show_keyboard() */
    public void showKeyboard(final boolean show) {
        runOnUiThread(() -> {
            try {
                InputMethodManager imm = (InputMethodManager) getSystemService(Context.INPUT_METHOD_SERVICE);
                if (imm == null) {
                    return;
                }
                if (show) {
                    if (view != null) {
                        view.requestFocus();
                        imm.showSoftInput(view, 0);
                    }
                } else if (view != null) {
                    imm.hideSoftInputFromWindow(view.getWindowToken(), 0);
                }
            } catch (Throwable t) {
                CrashLog.error("showKeyboard failed", t);
            }
        });
    }

    /** miniquad: clipboard_set()，在 context（也就是这个 Activity）上按名字调用 */
    public void copy(final String data) {
        copyToClipboard(data);
    }

    /** prpr::ext::open_url()，点应用内的链接（账号设置、公告、谱面作者链接等） */
    public void openUrl(final String url) {
        runOnUiThread(() -> {
            try {
                Intent intent = new Intent(Intent.ACTION_VIEW, Uri.parse(url));
                intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
                startActivity(intent);
            } catch (Throwable t) {
                // 没有浏览器 / 无法处理这个 scheme 时不要崩
                CrashLog.error("openUrl failed: " + url, t);
            }
        });
    }

    /** phira::page::library::request_export()，导出谱面时弹出「保存到哪」 */
    public void showExportDialog(final String suggestedName) {
        runOnUiThread(() -> {
            try {
                Intent intent = new Intent(Intent.ACTION_CREATE_DOCUMENT);
                intent.addCategory(Intent.CATEGORY_OPENABLE);
                intent.setType("application/zip");
                intent.putExtra(Intent.EXTRA_TITLE, suggestedName);
                startActivityForResult(intent, EXPORT_FILE);
            } catch (Throwable t) {
                CrashLog.error("showExportDialog failed", t);
            }
        });
    }

    /** phira::page::library::delete_uri()，导出流程结束后清掉 SAF 文档 */
    public void deleteUri(final Uri uri) {
        try {
            if (uri != null) {
                getContentResolver().delete(uri, null, null);
            }
        } catch (Throwable t) {
            CrashLog.error("deleteUri failed", t);
        }
    }

    /** 由 Rust 通过 JNI 调用（prpr::scene::request_file）。 */
    public void chooseFile() {
        runOnUiThread(() -> {
            try {
                Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
                intent.addCategory(Intent.CATEGORY_OPENABLE);
                intent.setType("*/*");
                intent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
                startActivityForResult(intent, PICK_FILE);
            } catch (Throwable t) {
                CrashLog.error("chooseFile failed", t);
                call("setChosenFile(empty)", () -> QuadNative.setChosenFile(""));
            }
        });
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        if (requestCode == EXPORT_FILE) {
            if (resultCode != RESULT_OK || data == null || data.getData() == null) {
                CrashLog.stage("导出被取消");
                return;
            }
            final Uri uri = data.getData();
            try {
                ParcelFileDescriptor pfd = getContentResolver().openFileDescriptor(uri, "w");
                if (pfd == null) {
                    CrashLog.error("导出：openFileDescriptor 返回 null", null);
                    return;
                }
                // detachFd 把 fd 的所有权交出去，Rust 侧会用 from_raw_fd 接管
                final int fd = pfd.detachFd();
                call("processExportFd", () -> QuadNative.processExportFd(uri, fd));
                CrashLog.stage("导出：fd 已交给 Rust");
            } catch (Throwable t) {
                CrashLog.error("导出失败", t);
            }
            return;
        }
        if (requestCode != PICK_FILE) {
            return;
        }
        if (resultCode != RESULT_OK || data == null || data.getData() == null) {
            call("setChosenFile(empty)", () -> QuadNative.setChosenFile(""));
            return;
        }
        final Uri uri = data.getData();
        new Thread(() -> {
            File dst = new File(getCacheDir(), "chosen_" + System.currentTimeMillis());
            try (InputStream in = getContentResolver().openInputStream(uri); FileOutputStream out = new FileOutputStream(dst)) {
                byte[] buf = new byte[1 << 16];
                int n;
                while ((n = in.read(buf)) > 0) {
                    out.write(buf, 0, n);
                }
                final String path = dst.getAbsolutePath();
                call("setChosenFile", () -> QuadNative.setChosenFile(path));
            } catch (Throwable t) {
                CrashLog.error("复制所选文件失败", t);
                call("setChosenFile(empty)", () -> QuadNative.setChosenFile(""));
            }
        }).start();
    }

    /**
     * singleTask 启动模式下，第三方文件管理器可能把结果投递到这里而不是 onActivityResult。
     * 目前 manifest 没有注册 VIEW/SEND 的 intent-filter，所以这里只更新 intent，不做导入。
     */
    @Override
    protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        setIntent(intent);
    }
}
