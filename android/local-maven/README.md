# local-maven

本地 Maven 仓库（不是 flatDir！），只放那些**不在 Maven Central 上**、但必须打进 APK 的组件。

## 为什么不用 app/libs + flatDir

flatDir 引入的本地 AAR **不会带传递依赖**，也不会读取 AAR 的 POM 依赖声明。
之前 `inputbox-0.1.4.aar` 就是这么坑的：它内部引用 `androidx.appcompat` 和
`com.google.android.material`，两个类都没进包，一点输入框就闪退。
用「本地 Maven 仓库 + `implementation "group:artifact:version"`」时，
Gradle 会按正常规则解析（POM 里的依赖会一起拉下来），行为和其他仓库一致。

## 现在放了什么

### rustls-platform-verifier（Android 组件）

Rust 侧 reqwest 0.13 的 `rustls` feature 用的是 `rustls-platform-verifier`，
它在 Android 上做证书校验时会：

```rust
CachedClass::new("org/rustls/platformverifier/CertificateVerifier")
… verifyCertificateChain(Context, String, String, String[], byte[], long, byte[][]) -> VerificationResult{code, message}
```

也就是**必须有一个 JVM 组件真的在 APK 里**（crate 文档原话：*"A small Kotlin
component must be included in your app's build"*）。少了它，`load_class` 失败 →
所有 HTTPS 请求都失败 → 登录页一直报「获取服务条款和隐私政策内容失败」，
而这个组件的缺失在启动时毫无征兆。

组件由 `rustls-platform-verifier-android` 这个 crate 以「on-disk Maven 仓库」的
形式随包发布，官方推荐做法（见 crate README 的 Android 一节）是把这个仓库目录
加到 Gradle 的 repositories 里。这里就是把那份文件原样 vendor 进来，
版本（0.1.1）必须和 `Cargo.lock` 里的 `rustls-platform-verifier-android` 一致，
CI 的 “Verify vendored rustls-platform-verifier AAR” 步骤会强制校验。

### 升级步骤

`Cargo.lock` 里的 `rustls-platform-verifier-android` 版本变了之后：

```bash
SRC=$(find ~/.cargo/registry/src -maxdepth 2 -type d -name 'rustls-platform-verifier-android-*' | head -n1)
rm -rf android/local-maven/rustls
cp -r "$SRC/maven/rustls" android/local-maven/
# 同时把 app/build.gradle 里的 implementation "rustls:rustls-platform-verifier:<版本>" 改掉
```

（组件是 Kotlin 写的，所以 `app/build.gradle` 里还需要 `kotlin-stdlib` 依赖。）
