//! Volume policy: Android follows the system volume (gain fixed at 1.0).

use nexus_player_backend::audio::types::AudioState;
use nexus_player_backend::audio::volume::{
    default_volume, default_volume_for, effective_volume, IS_ANDROID,
};

// R1
#[test]
fn android_default_volume_is_unity() {
    assert_eq!(default_volume_for(true), 1.0);
}

// R1
#[test]
fn desktop_default_volume_is_point_one() {
    assert_eq!(default_volume_for(false), 0.1);
}

// R1
#[test]
fn default_volume_matches_platform_constant() {
    assert_eq!(default_volume(), default_volume_for(IS_ANDROID));
    assert_eq!(IS_ANDROID, cfg!(target_os = "android"));
}

// R1
#[test]
fn audio_state_default_uses_platform_default_volume() {
    assert_eq!(AudioState::default().volume, default_volume());
}

// R2
#[test]
fn android_effective_volume_ignores_requests() {
    for requested in [0.0, 0.1, 0.5, 1.0, 2.0, -3.0, f32::NAN, f32::INFINITY] {
        assert_eq!(effective_volume(requested, true), 1.0, "request {requested}");
    }
}

// R2
#[test]
fn desktop_effective_volume_clamps_to_unit_range() {
    assert_eq!(effective_volume(0.5, false), 0.5);
    assert_eq!(effective_volume(0.0, false), 0.0);
    assert_eq!(effective_volume(1.0, false), 1.0);
    assert_eq!(effective_volume(-1.0, false), 0.0);
    assert_eq!(effective_volume(7.5, false), 1.0);
}

// R2
#[test]
fn desktop_effective_volume_is_finite_for_nan() {
    let v = effective_volume(f32::NAN, false);
    assert!(v.is_finite());
    assert!((0.0..=1.0).contains(&v));
}
