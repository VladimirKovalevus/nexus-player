fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new().plugin(
            "vkLogin",
            tauri_build::InlinedPlugin::new().commands(&["open"]),
        ),
    )
    .expect("Failed to run tauri_build");
}
