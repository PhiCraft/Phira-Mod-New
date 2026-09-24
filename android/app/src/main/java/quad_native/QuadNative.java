package quad_native;

import android.view.Surface;

// miniquad 的原生回调统一放在 quad_native.QuadNative 里（Java 无法指定符号查找名，
// 所以所有 JNI 符号都挂在固定包名 + 固定类名下）。
//
// 方法名与签名必须和两边严格对上：
//   - miniquad 的 java/QuadNative.java 模板（activityOn* / surfaceOn*）
//   - phira 自己导出的 Java_quad_1native_QuadNative_*（initializeEnvironment / setDataPath / ...）
// 少一个或参数个数不对，Rust 侧那边不是拿到 NULL 就是读到垃圾值。
public class QuadNative {
    // ---- miniquad ----

    public native static void activityOnCreate(Object activity);
    public native static void activityOnResume();
    public native static void activityOnPause();
    public native static void activityOnDestroy();

    public native static void surfaceOnSurfaceCreated(Surface surface);
    public native static void surfaceOnSurfaceDestroyed(Surface surface);
    public native static void surfaceOnSurfaceChanged(Surface surface, int width, int height);

    /**
     * 注意：Rust 侧的签名是 5 个参数 (IIFFJ)，最后一个是事件时间。
     * 模板里只写了 4 个，ART 会按短名解析到同一个符号，但 Rust 会去读不存在的第 5 个参数
     * （拿到寄存器垃圾值），所以这里必须带上 time。
     */
    public native static void surfaceOnTouch(int id, int phase, float x, float y, long time);

    public native static void surfaceOnKeyDown(int keycode);
    public native static void surfaceOnKeyUp(int keycode);
    public native static void surfaceOnCharacter(int character);

    public native static void initializeContext(Object activity);
    public native static void releaseContext();

    // ---- phira ----

    public native static void initializeEnvironment();

    public native static void prprActivityOnPause();
    public native static void prprActivityOnResume();
    public native static void prprActivityOnDestroy();

    public native static void setDataPath(String path);
    public native static void setTempDir(String path);
    public native static void setDpi(int dpi);
    public native static void setChosenFile(String file);
    public native static void markImport();
    public native static void markImportRespack();
    public native static void setInputText(String text);

    /**
     * Android 传感器上报当前摇动强度：线性加速度模长，单位 g（已去掉重力）。
     * 引擎用它做「摇一摇再玩」（静止自动暂停 / 摇动继续）。
     */
    public native static void onMotion(float level);

    /** 谱面导出：Java 侧建好文件后把 fd 交给 Rust（Rust 会接管这个 fd）。 */
    public native static void processExportFd(android.net.Uri uri, int fd);

    /** 兜底启动：万一宿主 miniquad 版本没有在 activityOnCreate 里启动游戏，
     *  由 Java 主动调一次（Rust 侧有幂等保护，不会被启动两次）。 */
    public native static void startApp();
}
