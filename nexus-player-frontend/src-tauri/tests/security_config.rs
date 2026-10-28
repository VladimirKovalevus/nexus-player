use serde_json::Value;
use std::path::PathBuf;

const CSP: &str = "default-src 'self'; script-src 'self' 'unsafe-eval'; img-src 'self' asset: http://asset.localhost https: http: data:; video-src 'self' asset: http://asset.localhost https: http:; connect-src 'self' https: http:; frame-src https: http:";

fn load(rel: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    let text = std::fs::read_to_string(&path).expect("config file readable");
    serde_json::from_str(&text).expect("config file is valid JSON")
}

/// All paths granted by `fs:scope` permission entries (allow and deny lists).
fn fs_scope_paths(caps: &Value) -> Vec<String> {
    let mut out = Vec::new();
    for perm in caps["permissions"].as_array().expect("permissions array") {
        if perm["identifier"] != "fs:scope" {
            continue;
        }
        for key in ["allow", "deny"] {
            for entry in perm[key].as_array().into_iter().flatten() {
                let p = entry
                    .as_str()
                    .or_else(|| entry["path"].as_str())
                    .expect("scope entry has a path");
                out.push(p.to_string());
            }
        }
    }
    out
}

#[test]
fn asset_protocol_scope_is_only_the_artwork_cache() {
    let conf = load("tauri.conf.json");
    let scope = conf["app"]["security"]["assetProtocol"]["scope"]
        .as_array()
        .expect("assetProtocol.scope is an array");
    let scope: Vec<&str> = scope.iter().map(|v| v.as_str().expect("string")).collect();
    assert_eq!(scope, vec!["$APPDATA/cache/artwork/**"]);
    assert!(scope.iter().all(|p| p.starts_with('$')));
    assert!(!scope.contains(&"**/*"));
}

#[test]
fn capabilities_have_no_unrestricted_fs_scope() {
    let caps = load("capabilities/default.json");
    let paths = fs_scope_paths(&caps);
    assert!(
        !paths.iter().any(|p| p == "**/*" || p == "**" || p == "/**"),
        "unrestricted fs scope: {paths:?}"
    );
    let has_read = caps["permissions"]
        .as_array()
        .expect("permissions array")
        .iter()
        .any(|p| p.as_str() == Some("fs:allow-read"));
    if has_read {
        assert!(
            !paths.is_empty(),
            "fs:allow-read without any scope is unrestricted"
        );
    }
}

#[test]
fn remaining_fs_scope_paths_stay_in_app_directories() {
    let caps = load("capabilities/default.json");
    for p in fs_scope_paths(&caps) {
        assert!(
            p.starts_with("$AUDIO")
                || p.starts_with("$APPDATA")
                || p.starts_with("$CACHE/nexus-downloads"),
            "fs scope path outside allowed bases: {p}"
        );
    }
}

#[test]
fn csp_is_unchanged() {
    let conf = load("tauri.conf.json");
    assert_eq!(conf["app"]["security"]["csp"].as_str(), Some(CSP));
}
