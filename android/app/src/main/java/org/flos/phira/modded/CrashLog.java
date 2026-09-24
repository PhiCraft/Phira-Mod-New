package org.flos.phira.modded;

import android.util.Log;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.PrintWriter;
import java.io.StringWriter;
import java.nio.charset.StandardCharsets;
import java.text.SimpleDateFormat;
import java.util.Date;
import java.util.Locale;

/**
 * 启动阶段日志 / 崩溃记录。
 *
 * 手机上通常没有 adb，一旦启动失败就什么都看不到。这里把「走到了哪一步」和异常
 * 追加写到一个文件，下次启动时由 MainActivity 弹出来显示，用户截图就能定位问题。
 *
 * 路径要和 Rust 侧一致：Rust 的 panic hook 用 `dir::root()`，而 phira 的
 * `dir::root()` = `<DATA_PATH>/data`（DATA_PATH 就是 getFilesDir()），
 * 所以两边都写 `<filesDir>/data/phira-crash.log`。
 */
public final class CrashLog {
    private static final String TAG = "PhiraMod";
    private static final String FILE_NAME = "phira-crash.log";
    private static final String DATA_SUBDIR = "data";

    private static File file = null;
    private static File legacyFile = null;
    /** 额外写一份到外部私有目录（/sdcard/Android/data/<包名>/files/），
     *  它不需要任何权限，且用文件管理器就能直接打开 —— 万一 App 连弹窗都弹不出来，
     *  也一定能把这个文件拿出来。 */
    private static File externalFile = null;

    private CrashLog() {}

    static void init(File dir) {
        init(dir, null);
    }

    static void init(File dir, File externalDir) {
        try {
            File dataDir = new File(dir, DATA_SUBDIR);
            if (!dataDir.exists()) {
                //noinspection ResultOfMethodCallIgnored
                dataDir.mkdirs();
            }
            file = new File(dataDir, FILE_NAME);
            // 旧版本把日志写在 filesDir 根目录，读的时候兼容一下
            legacyFile = new File(dir, FILE_NAME);
            if (externalDir != null) {
                if (!externalDir.exists()) {
                    //noinspection ResultOfMethodCallIgnored
                    externalDir.mkdirs();
                }
                externalFile = new File(externalDir, FILE_NAME);
            }
        } catch (Throwable t) {
            Log.w(TAG, "CrashLog init failed", t);
        }
    }

    /** 记录已经走到哪一步。 */
    public static synchronized void stage(String stage) {
        Log.i(TAG, "stage: " + stage);
        write("[stage] " + stage);
    }

    public static synchronized void error(String what, Throwable t) {
        Log.e(TAG, what, t);
        StringWriter sw = new StringWriter();
        sw.append("[error] ").append(what).append('\n');
        if (t != null) {
            t.printStackTrace(new PrintWriter(sw));
        }
        write(sw.toString());
    }

    private static void write(String text) {
        String line = new SimpleDateFormat("MM-dd HH:mm:ss", Locale.US).format(new Date()) + " " + text + "\n";
        byte[] bytes = line.getBytes(StandardCharsets.UTF_8);
        append(file, bytes);
        append(externalFile, bytes);
    }

    private static void append(File target, byte[] bytes) {
        if (target == null) {
            return;
        }
        try (FileOutputStream out = new FileOutputStream(target, true)) {
            out.write(bytes);
        } catch (Throwable ignored) {
            // 记日志本身失败就算了，绝不能因此影响启动
        }
    }

    /** 读取上一次运行留下的日志；没有则返回 null。 */
    static synchronized String read(File dir) {
        try {
            File f = file != null && file.exists() && file.length() > 0 ? file : legacyFile;
            if (f == null || !f.exists() || f.length() == 0) {
                return null;
            }
            try (FileInputStream in = new FileInputStream(f)) {
                ByteArrayOutputStream buf = new ByteArrayOutputStream();
                byte[] chunk = new byte[8192];
                int n;
                while ((n = in.read(chunk)) > 0) {
                    buf.write(chunk, 0, n);
                }
                return buf.toString(StandardCharsets.UTF_8.name());
            }
        } catch (Throwable t) {
            Log.w(TAG, "CrashLog read failed", t);
            return null;
        }
    }

    /** 判断这份日志是不是「上一次没正常退出」。 */
    static boolean looksLikeFailure(String log) {
        if (log == null) {
            return false;
        }
        if (log.contains("[error]") || log.contains("[rust panic]")) {
            return true;
        }
        // 只有真的渲染出第一帧才算跑起来了；否则一律弹出来，让用户至少能看到内容
        // （之前用「有没有 [exit]」判断，若崩溃发生在 onDestroy 之后就会漏掉、什么都不弹）
        return !log.contains("已渲染第一帧");
    }

    static synchronized void clear(File dir) {
        try {
            for (File f : new File[] { file, legacyFile }) {
                if (f != null && f.exists() && !f.delete()) {
                    Log.w(TAG, "CrashLog clear failed: " + f);
                }
            }
        } catch (Throwable ignored) {
        }
    }
}
