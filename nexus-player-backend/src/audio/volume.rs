//! Platform volume policy.
//!
//! On Android the software gain stays at 1.0 so the audible level follows the
//! system media volume. Elsewhere the player keeps its own gain.

/// True when compiled for Android.
pub const IS_ANDROID: bool = cfg!(target_os = "android");

/// Initial software volume for the given platform.
pub const fn default_volume_for(is_android: bool) -> f32 {
    if is_android {
        1.0
    } else {
        0.1
    }
}

/// Initial software volume for the current platform.
pub fn default_volume() -> f32 {
    default_volume_for(IS_ANDROID)
}

/// Volume actually applied for a requested value. Always finite.
pub fn effective_volume(requested: f32, is_android: bool) -> f32 {
    if is_android {
        1.0
    } else if requested.is_nan() {
        0.0
    } else {
        requested.clamp(0.0, 1.0)
    }
}
