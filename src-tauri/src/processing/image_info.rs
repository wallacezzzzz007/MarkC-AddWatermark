use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::image_asset::{ImageAsset, ImageStatus, ImportSkipReason, SkippedImport};

const SUPPORTED_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png"];

pub fn is_supported_extension(path: &Path) -> bool {
    normalized_extension(path)
        .as_deref()
        .is_some_and(|ext| SUPPORTED_EXTENSIONS.contains(&ext))
}

pub fn read_image_asset(path: &Path) -> Result<ImageAsset, SkippedImport> {
    if !path.is_file() {
        return Err(skip(path, ImportSkipReason::NotFile, "Path is not a file"));
    }

    let extension = normalized_extension(path).ok_or_else(|| {
        skip(
            path,
            ImportSkipReason::UnsupportedFormat,
            "File has no supported extension",
        )
    })?;

    if !SUPPORTED_EXTENSIONS.contains(&extension.as_str()) {
        return Err(skip(
            path,
            ImportSkipReason::UnsupportedFormat,
            "Only JPG, JPEG, and PNG images are supported",
        ));
    }

    let metadata = fs::metadata(path).map_err(|err| {
        skip(
            path,
            ImportSkipReason::ReadError,
            &format!("Could not read file metadata: {err}"),
        )
    })?;

    let (width, height) = image::image_dimensions(path).map_err(|err| {
        skip(
            path,
            ImportSkipReason::CorruptImage,
            &format!("Could not read image dimensions: {err}"),
        )
    })?;

    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Untitled image")
        .to_string();
    let path_string = path.to_string_lossy().to_string();

    Ok(ImageAsset {
        id: stable_id(&path_string),
        path: path_string,
        filename,
        extension,
        width,
        height,
        byte_size: metadata.len(),
        status: ImageStatus::Ready,
    })
}

pub fn collect_folder_images(path: &Path, recursive: bool) -> Result<Vec<PathBuf>, SkippedImport> {
    if !path.is_dir() {
        return Err(skip(
            path,
            ImportSkipReason::NotDirectory,
            "Path is not a folder",
        ));
    }

    let mut paths = Vec::new();

    if recursive {
        for entry in walkdir::WalkDir::new(path)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
        {
            paths.push(entry.path().to_path_buf());
        }
    } else {
        for entry in fs::read_dir(path).map_err(|err| {
            skip(
                path,
                ImportSkipReason::ReadError,
                &format!("Could not read folder: {err}"),
            )
        })? {
            match entry {
                Ok(entry) => paths.push(entry.path()),
                Err(err) => {
                    return Err(skip(
                        path,
                        ImportSkipReason::ReadError,
                        &format!("Could not read folder entry: {err}"),
                    ));
                }
            }
        }
    }

    Ok(paths)
}

pub fn skip(path: &Path, reason: ImportSkipReason, message: &str) -> SkippedImport {
    SkippedImport {
        path: path.to_string_lossy().to_string(),
        reason,
        message: message.to_string(),
    }
}

fn normalized_extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
}

fn stable_id(value: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use image::{ImageBuffer, Rgba};

    use super::{is_supported_extension, read_image_asset};
    use crate::domain::image_asset::ImportSkipReason;

    #[test]
    fn reads_png_dimensions_without_mutating_file() {
        let dir = test_dir("png_dimensions");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sample.png");
        let image = ImageBuffer::from_pixel(24, 12, Rgba([30_u8, 60, 90, 255]));
        image.save(&path).unwrap();
        let before = fs::metadata(&path).unwrap().modified().unwrap();

        let asset = read_image_asset(&path).unwrap();
        let after = fs::metadata(&path).unwrap().modified().unwrap();

        assert_eq!(asset.width, 24);
        assert_eq!(asset.height, 12);
        assert_eq!(asset.extension, "png");
        assert_eq!(before, after);
    }

    #[test]
    fn accepts_uppercase_supported_extensions() {
        assert!(is_supported_extension(
            &test_dir("uppercase").join("PHOTO.JPG")
        ));
        assert!(is_supported_extension(
            &test_dir("uppercase").join("PHOTO.PNG")
        ));
    }

    #[test]
    fn rejects_corrupt_images_with_supported_extension() {
        let dir = test_dir("corrupt");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("broken.jpg");
        fs::write(&path, b"not really an image").unwrap();

        let skipped = read_image_asset(&path).unwrap_err();

        assert!(matches!(skipped.reason, ImportSkipReason::CorruptImage));
    }

    fn test_dir(name: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("watermark_{name}_{stamp}"))
    }
}
