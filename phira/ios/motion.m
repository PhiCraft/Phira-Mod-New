// iOS 加速度计包装（CoreMotion），给「摇一摇再玩」用。
//
// 只被 phira/build.rs 在 iOS 目标下编译，其它平台不参与构建。
// 对外只暴露三个 C 函数，Rust 侧在 phira/src/lib.rs 里声明同名 extern。

#import <CoreMotion/CoreMotion.h>
#import <math.h>

static CMMotionManager *gManager = nil;

/// 启动采集；返回 1 表示传感器可用，0 表示这台设备没有加速度计（模拟器等）。
int phira_motion_start(void) {
    if (gManager == nil) {
        gManager = [[CMMotionManager alloc] init];
    }
    if (!gManager.isAccelerometerAvailable) {
        return 0;
    }
    gManager.accelerometerUpdateInterval = 1.0 / 60.0;
    [gManager startAccelerometerUpdates];
    return 1;
}

void phira_motion_stop(void) {
    if (gManager == nil) {
        return;
    }
    [gManager stopAccelerometerUpdates];
    [gManager release];
    gManager = nil;
}

/// 当前摇动强度（单位 g，已去掉重力）；返回负数表示传感器不可用。
float phira_motion_level(void) {
    if (gManager == nil || !gManager.isAccelerometerAvailable) {
        return -1.0f;
    }
    CMAccelerometerData *data = gManager.accelerometerData;
    if (data == nil) {
        return 0.0f;
    }
    CMAcceleration a = data.acceleration;
    // CoreMotion 的 acceleration 含重力（静止时模长约 1g），这里减掉当作线性加速度，
    // 和 Android 侧退回原始加速度计时的处理保持一致。
    double magnitude = sqrt(a.x * a.x + a.y * a.y + a.z * a.z) - 1.0;
    if (magnitude < 0.0) {
        magnitude = -magnitude;
    }
    return (float) magnitude;
}
