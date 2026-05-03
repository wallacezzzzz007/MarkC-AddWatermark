use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use image::codecs::png::PngEncoder;
use image::{ColorType, ImageEncoder};

use crate::domain::export::WatermarkExport;
use crate::domain::image_asset::{ImportResult, ImportSkipReason};
use crate::processing::exporter::load_source_image;
use crate::processing::image_info::{
    collect_folder_images, is_supported_extension, read_image_asset, skip,
};
use crate::processing::renderer::render_text_watermark;

#[tauri::command]
pub fn import_files(paths: Vec<String>) -> ImportResult {
    import_paths(paths.into_iter().map(PathBuf::from).collect())
}

#[tauri::command]
pub fn import_folder(path: String, recursive: bool) -> ImportResult {
    match collect_folder_images(&PathBuf::from(&path), recursive) {
        Ok(paths) => import_paths(paths),
        Err(skipped) => ImportResult {
            accepted: Vec::new(),
            skipped: vec![skipped],
        },
    }
}

#[tauri::command]
pub fn get_image_info(path: String) -> Result<crate::domain::image_asset::ImageAsset, String> {
    read_image_asset(&PathBuf::from(path)).map_err(|skipped| skipped.message)
}

#[tauri::command]
pub fn get_preview_data_url(path: String) -> Result<String, String> {
    let path = PathBuf::from(path);
    if !path.is_file() {
        return Err("Preview source is not a readable file".to_string());
    }
    if !is_supported_extension(&path) {
        return Err("Only JPG, JPEG, and PNG previews are supported".to_string());
    }

    let bytes = fs::read(&path).map_err(|err| format!("Could not read preview image: {err}"))?;
    let mime = preview_mime_type(&path);
    Ok(format!("data:{mime};base64,{}", encode_base64(&bytes)))
}

#[tauri::command]
pub fn render_preview_data_url(path: String, watermark: WatermarkExport) -> Result<String, String> {
    let path = PathBuf::from(path);
    if !path.is_file() {
        return Err("Preview source is not a readable file".to_string());
    }
    if !is_supported_extension(&path) {
        return Err("Only JPG, JPEG, and PNG previews are supported".to_string());
    }

    let mut image = load_source_image(&path)?.to_rgba8();
    render_text_watermark(&mut image, &watermark)?;

    let mut bytes = Vec::new();
    let encoder = PngEncoder::new(Cursor::new(&mut bytes));
    encoder
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            ColorType::Rgba8.into(),
        )
        .map_err(|err| format!("Could not encode preview image: {err}"))?;

    Ok(format!("data:image/png;base64,{}", encode_base64(&bytes)))
}

fn import_paths(paths: Vec<PathBuf>) -> ImportResult {
    let mut accepted = Vec::new();
    let mut skipped = Vec::new();

    for path in paths {
        if path.is_dir() {
            match collect_folder_images(&path, false) {
                Ok(paths) => {
                    let result = import_paths(paths);
                    accepted.extend(result.accepted);
                    skipped.extend(result.skipped);
                }
                Err(error) => skipped.push(error),
            }
            continue;
        }

        if !path.is_file() {
            skipped.push(skip(&path, ImportSkipReason::NotFile, "Path is not a file"));
            continue;
        }

        if !is_supported_extension(&path) {
            skipped.push(skip(
                &path,
                ImportSkipReason::UnsupportedFormat,
                "Only JPG, JPEG, and PNG images are supported",
            ));
            continue;
        }

        match read_image_asset(&path) {
            Ok(asset) => accepted.push(asset),
            Err(error) => skipped.push(error),
        }
    }

    ImportResult { accepted, skipped }
}

fn preview_mime_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        _ => "image/jpeg",
    }
}

pub(super) fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);

    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = *chunk.get(1).unwrap_or(&0);
        let third = *chunk.get(2).unwrap_or(&0);
        let triple = ((first as u32) << 16) | ((second as u32) << 8) | third as u32;

        output.push(TABLE[((triple >> 18) & 0x3f) as usize] as char);
        output.push(TABLE[((triple >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            output.push(TABLE[((triple >> 6) & 0x3f) as usize] as char);
        } else {
            output.push('=');
        }
        if chunk.len() > 2 {
            output.push(TABLE[(triple & 0x3f) as usize] as char);
        } else {
            output.push('=');
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::encode_base64;

    #[test]
    fn encodes_base64_with_padding() {
        assert_eq!(encode_base64(b""), "");
        assert_eq!(encode_base64(b"f"), "Zg==");
        assert_eq!(encode_base64(b"fo"), "Zm8=");
        assert_eq!(encode_base64(b"foo"), "Zm9v");
        assert_eq!(encode_base64(b"hello"), "aGVsbG8=");
    }
}
