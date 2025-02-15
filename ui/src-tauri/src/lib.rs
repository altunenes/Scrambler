use engine::scramble::ScrambleOptions;

#[tauri::command]
async fn scramble_image(image_data: String, options: ScrambleOptions) -> Result<String, String> {
    engine::process_base64_image(&image_data, &options).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![scramble_image])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
