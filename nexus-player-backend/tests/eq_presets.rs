//! EQ presets use stable ids; legacy Russian names migrate on deserialize (R1).

use nexus_player_backend::audio::eq::{builtin_presets, EqSettings};
use nexus_player_backend::config::AppConfig;
use serde_json::json;

const IDS: [&str; 7] = [
    "flat",
    "rock",
    "pop",
    "jazz",
    "classical",
    "bass_boost",
    "vocal",
];

// Legacy names, escaped so no Cyrillic literal appears in first-party Rust.
const LEGACY: [(&str, &str); 7] = [
    ("\u{41F}\u{43B}\u{43E}\u{441}\u{43A}\u{438}\u{439}", "flat"),
    ("\u{420}\u{43E}\u{43A}", "rock"),
    ("\u{41F}\u{43E}\u{43F}", "pop"),
    ("\u{414}\u{436}\u{430}\u{437}", "jazz"),
    (
        "\u{41A}\u{43B}\u{430}\u{441}\u{441}\u{438}\u{43A}\u{430}",
        "classical",
    ),
    (
        "\u{411}\u{430}\u{441}-\u{431}\u{443}\u{441}\u{442}",
        "bass_boost",
    ),
    ("\u{412}\u{43E}\u{43A}\u{430}\u{43B}", "vocal"),
];

const GAINS: [f32; 10] = [1.0, 2.0, 3.0, 4.0, 5.0, -1.0, -2.0, -3.0, -4.0, -5.0];

// R1
#[test]
fn eq_default_preset_is_flat() {
    assert_eq!(EqSettings::default().preset_name.as_deref(), Some("flat"));
}

// R1
#[test]
fn eq_builtin_preset_ids_are_the_seven_in_order() {
    let ids: Vec<&str> = builtin_presets().iter().map(|(id, _)| *id).collect();
    assert_eq!(ids, IDS);
}

// R1
#[test]
fn eq_all_legacy_names_migrate_to_ids() {
    for (legacy, id) in LEGACY {
        let s: EqSettings = serde_json::from_value(
            json!({ "enabled": true, "gains": GAINS, "presetName": legacy }),
        )
        .unwrap();
        assert_eq!(s.preset_name.as_deref(), Some(id), "legacy {legacy:?}");
        assert_eq!(s.gains, GAINS);
    }
}

// R1
#[test]
fn eq_known_id_is_kept() {
    for id in IDS {
        let s: EqSettings = serde_json::from_value(json!({ "presetName": id })).unwrap();
        assert_eq!(s.preset_name.as_deref(), Some(id));
    }
}

// R1
#[test]
fn eq_unknown_name_becomes_custom_with_gains_kept() {
    let s: EqSettings = serde_json::from_value(
        json!({ "enabled": false, "gains": GAINS, "presetName": "My custom" }),
    )
    .unwrap();
    assert_eq!(s.preset_name, None);
    assert_eq!(s.gains, GAINS);
    assert!(!s.enabled);
}

// R1
#[test]
fn eq_explicit_null_stays_none() {
    let s: EqSettings =
        serde_json::from_value(json!({ "gains": GAINS, "presetName": null })).unwrap();
    assert_eq!(s.preset_name, None);
    assert_eq!(s.gains, GAINS);
}

// R1
#[test]
fn eq_missing_preset_field_gives_flat() {
    let s: EqSettings = serde_json::from_value(json!({ "gains": GAINS })).unwrap();
    assert_eq!(s.preset_name.as_deref(), Some("flat"));
    let s: EqSettings = serde_json::from_value(json!({})).unwrap();
    assert_eq!(s.preset_name.as_deref(), Some("flat"));
}

// R1
#[test]
fn eq_migrated_value_round_trips() {
    let (legacy, _) = LEGACY[5];
    let s: EqSettings =
        serde_json::from_value(json!({ "gains": GAINS, "presetName": legacy })).unwrap();
    let text = serde_json::to_string(&s).unwrap();
    assert!(text.contains("\"presetName\":\"bass_boost\""), "{text}");
    let back: EqSettings = serde_json::from_str(&text).unwrap();
    assert_eq!(back.preset_name, s.preset_name);
    assert_eq!(back.gains, s.gains);
    assert_eq!(back.enabled, s.enabled);
}

// R1
#[test]
fn eq_old_app_config_migrates_eq_object() {
    let (legacy, _) = LEGACY[1];
    let cfg: AppConfig =
        serde_json::from_value(json!({ "eq": { "gains": GAINS, "presetName": legacy } })).unwrap();
    assert_eq!(cfg.eq.preset_name.as_deref(), Some("rock"));
    assert_eq!(cfg.eq.gains, GAINS);
}

// R1
#[test]
fn eq_source_has_no_cyrillic() {
    let src = include_str!("../src/audio/eq.rs");
    let bad: Vec<char> = src
        .chars()
        .filter(|c| ('\u{0400}'..='\u{04FF}').contains(c))
        .collect();
    assert!(bad.is_empty(), "Cyrillic left in eq.rs: {bad:?}");
}
