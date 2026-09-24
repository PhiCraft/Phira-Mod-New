//! 音乐可视化：把音频采样做 FFT，折成对数频段，供判定线画频谱条。
//!
//! 数据来源是 `sasa::AudioClip::frames()`（解码后的 PCM，`Frame(left, right)`），
//! 所以不需要动音频后端，也不会引入新依赖 —— FFT 就是下面这 20 行的 radix-2。
//!
//! 每帧由 `GameScene::render` 调用一次 [`update`]，判定线渲染时读 [`bands`]。

use sasa::Frame;
use std::cell::RefCell;

/// 频段数（画出来的条数）。
pub const BANDS: usize = 24;
/// FFT 窗口大小（必须是 2 的幂）。
const FFT_SIZE: usize = 1024;

/// 就地 radix-2 FFT（Cooley-Tukey）。
fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    if !n.is_power_of_two() || n < 2 {
        return;
    }
    // 位反转置换
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * std::f32::consts::PI / len as f32;
        let (wr, wi) = (ang.cos(), ang.sin());
        let half = len / 2;
        let mut i = 0;
        while i < n {
            let (mut wkr, mut wki) = (1.0f32, 0.0f32);
            for k in 0..half {
                let (ur, ui) = (re[i + k], im[i + k]);
                let (br, bi) = (re[i + k + half], im[i + k + half]);
                let vr = br * wkr - bi * wki;
                let vi = br * wki + bi * wkr;
                re[i + k] = ur + vr;
                im[i + k] = ui + vi;
                re[i + k + half] = ur - vr;
                im[i + k + half] = ui - vi;
                let nwr = wkr * wr - wki * wi;
                wki = wkr * wi + wki * wr;
                wkr = nwr;
            }
            i += len;
        }
        len <<= 1;
    }
}

thread_local! {
    /// 当前的频段能量（0~1，已做攻击/释放平滑）。
    static VALUE: RefCell<[f32; BANDS]> = const { RefCell::new([0.; BANDS]) };
}

/// 当前频段能量（0~1）。
pub fn bands() -> [f32; BANDS] {
    VALUE.with(|it| *it.borrow())
}

/// 关闭可视化时把数据清干净，免得下次打开还残留上次的形状。
pub fn silence() {
    VALUE.with(|it| *it.borrow_mut() = [0.; BANDS]);
}

/// 用 `frames` 里 `time` 秒附近的一小段采样更新频段能量。
///
/// `gain` 是用户可调的放大系数（0.2~4）。
pub fn update(frames: &[Frame], sample_rate: u32, time: f64, gain: f32) {
    let sr = sample_rate as f32;
    if frames.is_empty() || sr <= 0. || !time.is_finite() {
        silence();
        return;
    }
    let start = (time * sample_rate as f64).max(0.) as usize;
    if start + FFT_SIZE > frames.len() {
        // 还没开始或已经放完
        silence();
        return;
    }
    // 8KB 的栈缓冲，别每帧去堆上分配（BUF 那套反而更慢）
    let mut re = [0.0f32; FFT_SIZE];
    let mut im = [0.0f32; FFT_SIZE];
    for i in 0..FFT_SIZE {
        // 单声道 + Hann 窗
        let s = frames[start + i];
        let w = 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (FFT_SIZE - 1) as f32).cos();
        re[i] = (s.0 + s.1) * 0.5 * w;
    }
    fft(&mut re, &mut im);

    let f_lo = 40.0f32;
    let f_hi = (sr / 2.).min(16000.);
    let ratio = f_hi / f_lo;
    let gain = gain.clamp(0.2, 4.);
    let mut band_value = [0.0f32; BANDS];
    for (b, out) in band_value.iter_mut().enumerate() {
        let t0 = b as f32 / BANDS as f32;
        let t1 = (b + 1) as f32 / BANDS as f32;
        let i0 = ((f_lo * ratio.powf(t0) / sr * FFT_SIZE as f32) as usize).max(1);
        let i1 = (((f_lo * ratio.powf(t1) / sr * FFT_SIZE as f32) as usize).max(i0 + 1)).min(FFT_SIZE / 2);
        let mut peak = 0.0f32;
        for i in i0..i1 {
            let mag = (re[i] * re[i] + im[i] * im[i]).sqrt();
            peak = peak.max(mag);
        }
        // 归一化 + 开方压一下动态范围，视觉上更平均
        let v = (peak * 2. / FFT_SIZE as f32 * gain).powf(0.5);
        *out = v.min(1.);
    }
    VALUE.with(|it| {
        let mut cur = it.borrow_mut();
        for (c, v) in cur.iter_mut().zip(band_value) {
            // 攻击快、释放慢：看起来才有「跳」的感觉
            let k = if v > *c { 0.55 } else { 0.16 };
            *c += (v - *c) * k;
        }
    });
}
