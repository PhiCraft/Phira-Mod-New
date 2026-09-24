package moe.mivik.inputbox;

import android.app.Activity;
import android.app.AlertDialog;
import android.content.DialogInterface;
import android.text.InputType;
import android.util.Log;
import android.util.TypedValue;
import android.view.WindowManager;
import android.widget.EditText;
import android.widget.LinearLayout;

import org.flos.phira.modded.CrashLog;

import java.lang.ref.WeakReference;

/**
 * 输入框的 Java 侧实现。
 *
 * <p>契约来自 Rust 的 inputbox crate（inputbox-0.1.4/src/backend/android.rs）：
 * Rust 只认 {@code moe/mivik/inputbox/InputBox} 这个类名，调用静态方法
 * {@code showInput(long callback, String title, String prompt, String default,
 * String okLabel, String cancelLabel, String mode, boolean autoWrap,
 * boolean scrollToEnd)}，并在返回值非 null 时把它当成错误信息（返回 null 表示对话框已经弹出）；
 * 用户确认/取消后由 Java 调回 {@code inputCallback(callback, text)} —— 它是 native 方法，
 * 实现在 libphira.so 里（{@code Java_moe_mivik_inputbox_InputBox_inputCallback}）。
 *
 * <p>以前这里是官方发的一个 AAR，而它内部用的是
 * {@code androidx.appcompat.app.AlertDialog} 和
 * {@code com.google.android.material.textfield.TextInputEditText}。偏偏
 * app/libs 下的本地 AAR 走的是 flatDir，**不会带传递依赖**，我们又没有显式声明
 * material / appcompat，于是这两个类根本不在 APK 里：点开输入框（登录、改昵称、
 * 搜索……）立刻 NoClassDefFoundError 闪退。这里改成只用系统控件自己画，
 * 既不引入 material/appcompat（AAR 里的 MaterialAlertDialog 还要求主题是
 * Theme.MaterialComponents，会把当前主题也绑死），行为与原来保持一致。
 */
public final class InputBox {
    private static final String TAG = "PhiraInputBox";

    /** 当前 Activity；由 MainActivity 在 onCreate/onResume 里设置。 */
    private static WeakReference<Activity> currentActivity = new WeakReference<>(null);

    private InputBox() {}

    public static void setActivity(Activity activity) {
        currentActivity = new WeakReference<>(activity);
    }

    /** Rust 侧在用户确认/取消后回调（text 为 null 表示取消）。 */
    public static native void inputCallback(long callback, String text);

    /**
     * 弹出输入框。返回 null 表示已经在 UI 线程上排好队；返回字符串表示出错（Rust 会当成 Err）。
     */
    public static String showInput(final long callback, final String title, final String prompt, final String defaultText, final String okLabel,
                                   final String cancelLabel, final String mode, final boolean autoWrap, final boolean scrollToEnd) {
        Activity activity = currentActivity.get();
        if (activity == null) {
            // 和原来的 AAR 一样：非 null 返回值 = 错误信息
            return "no active activity";
        }
        final Activity ctx = activity;
        activity.runOnUiThread(() -> showDialog(ctx, callback, title, prompt, defaultText, okLabel, cancelLabel, mode, autoWrap, scrollToEnd));
        return null;
    }

    private static void showDialog(Activity activity, long callback, String title, String prompt, String defaultText, String okLabel, String cancelLabel,
                                   String mode, boolean autoWrap, boolean scrollToEnd) {
        // Rust 那边是 Box::from_raw(callback)，这个指针必须**恰好**被回传一次，
        // 否则就是重复释放 / 泄漏。确认按钮点下去之后对话框还会走 OnDismiss，
        // 所以两边都要看这个标记（原来的 AAR 也是这么防的）。
        final boolean[] sent = { false };
        try {
            EditText input = new EditText(activity);
            input.setText(defaultText == null ? "" : defaultText);
            if ("password".equals(mode)) {
                input.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_PASSWORD);
            } else if ("multiline".equals(mode)) {
                input.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_FLAG_MULTI_LINE);
                input.setSingleLine(false);
                input.setMaxLines(6);
            } else {
                input.setInputType(InputType.TYPE_CLASS_TEXT);
            }
            if (!autoWrap) {
                input.setSingleLine(true);
            }
            if (prompt != null && !prompt.isEmpty()) {
                input.setHint(prompt);
            }
            if (scrollToEnd) {
                input.setSelection(input.getText().length());
            }

            // 原来 Material 的 TextInputEditText 自带内边距，这里手动补一点，
            // 不然输入框会贴着对话框边缘
            int pad = (int) TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_DIP, 20, activity.getResources().getDisplayMetrics());
            LinearLayout container = new LinearLayout(activity);
            container.setOrientation(LinearLayout.VERTICAL);
            container.setPadding(pad, pad / 2, pad, 0);
            container.addView(input);

            AlertDialog.Builder builder = new AlertDialog.Builder(activity);
            if (title != null) {
                builder.setTitle(title);
            }
            builder.setView(container);
            builder.setNegativeButton(cancelLabel == null ? "Cancel" : cancelLabel, (DialogInterface.OnClickListener) null);
            builder.setPositiveButton(okLabel == null ? "OK" : okLabel, (dialog, which) -> {
                if (sent[0]) {
                    return;
                }
                sent[0] = true;
                sendCallback(callback, String.valueOf(input.getText()));
            });
            builder.setCancelable(true);
            builder.setOnDismissListener(dialog -> {
                // 取消 / 返回键 / 点外面：回传 null，Rust 侧会当作「用户取消」
                if (sent[0]) {
                    return;
                }
                sent[0] = true;
                sendCallback(callback, null);
            });

            AlertDialog dialog = builder.create();
            // 单独 try 一下：万一 getWindow() 为 null，也不能让整个对话框弹不出来
            try {
                if (dialog.getWindow() != null) {
                    dialog.getWindow().setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_STATE_ALWAYS_VISIBLE);
                }
            } catch (Throwable ignored) {
            }
            dialog.show();
            input.requestFocus();
        } catch (Throwable t) {
            Log.e(TAG, "showInput failed", t);
            CrashLog.error("输入框弹不出来", t);
            // 弹不出来也要回一次，否则 Rust 那边会一直等输入
            if (!sent[0]) {
                sent[0] = true;
                sendCallback(callback, null);
            }
        }
    }

    /**
     * 调回 Rust。包一层 try：万一 libphira.so 里没有这个 native 符号（链接被裁掉之类），
     * 抛 UnsatisfiedLinkError 会让整个 App 直接闪退——宁可记一笔日志、让这个输入框
     * 停在那儿，也不要因为一个输入框把游戏带崩。
     */
    private static void sendCallback(long callback, String text) {
        try {
            inputCallback(callback, text);
        } catch (Throwable t) {
            Log.e(TAG, "inputCallback failed", t);
            CrashLog.error("输入框回调失败（libphira.so 里缺 native 符号？）", t);
        }
    }
}
