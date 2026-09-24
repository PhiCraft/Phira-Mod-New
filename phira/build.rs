fn git_stdout(args: &[&str]) -> Option<String> {
    let output = std::process::Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = std::str::from_utf8(&output.stdout).ok()?.trim().to_string();
    if stdout.is_empty() {
        None
    } else {
        Some(stdout)
    }
}

fn main() {
    dotenv_build::output(dotenv_build::Config::default()).unwrap();

    let git_dir = git_stdout(&["rev-parse", "--git-dir"]).unwrap_or_else(|| ".git".to_string());
    println!("cargo:rerun-if-changed={}/HEAD", git_dir);
    println!("cargo:rerun-if-changed={}/packed-refs", git_dir);

    if let Some(ref_path) = git_stdout(&["symbolic-ref", "-q", "HEAD"]) {
        println!("cargo:rerun-if-changed={}/{}", git_dir, ref_path);
    }

    let git_hash = git_stdout(&["rev-parse", "--short=7", "HEAD"]).unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=GIT_HASH={}", git_hash);

    build_ios_motion();
}

/// iOS：把 CoreMotion 包一层极小的 Objective-C（读加速度计给「摇一摇再玩」用）。
///
/// 只在 iOS 目标下编，其它平台直接跳过，所以本地 Windows / Android 构建不受影响。
/// CoreMotion 是系统框架，用 `cargo:rustc-link-lib=framework=CoreMotion` 让链接器带上即可，
/// 不需要改 Xcode 工程（最终链接是 cargo 做的）。
fn build_ios_motion() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "ios" {
        return;
    }
    println!("cargo:rerun-if-changed=ios/motion.m");
    // 不启用 ARC：ARC 不允许「无可属限定符的全局可保持对象指针」，
    // motion.m 里的 static 管理器在 MRR 下自己释放，更省事。
    cc::Build::new().file("ios/motion.m").compile("phira_motion");
    println!("cargo:rustc-link-lib=framework=CoreMotion");
    println!("cargo:rustc-link-lib=framework=Foundation");
}
