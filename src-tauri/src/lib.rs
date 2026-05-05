mod commands;
mod domain;
mod processing;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::export::export_selected_image,
            commands::export::export_batch,
            commands::export::cancel_batch_export,
            commands::import::import_files,
            commands::import::import_folder,
            commands::import::get_image_info,
            commands::import::get_preview_data_url,
            commands::import::render_preview_data_url
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
