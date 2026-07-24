mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::fs::read_text_file,
            commands::secrets::secret_get,
            commands::secrets::secret_set,
            commands::secrets::secret_remove,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
