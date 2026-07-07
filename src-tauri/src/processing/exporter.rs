use std::fs;
use std::path::{Path, PathBuf};

use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ImageDecoder, ImageReader};

use crate::domain::export::MetadataPolicy;
use crate::domain::export::{
    ExportResult, ExportSelectedRequest, NamingRule, OutputRules, WatermarkExport,
};
use crate::processing::metadata::{apply_metadata_policy, read_image_description};
use crate::processing::renderer::render_text_watermark;

const DEFAULT_EXPORT_FOLDER: &str = "Watermark export";
const JPEG_QUALITY: u8 = 98;

pub fn export_selected_image(request: &ExportSelectedRequest) -> Result<ExportResult, String> {
    let source_path = PathBuf::from(&request.source_path);
    if !source_path.is_file() {
        return Err("Selected source image is not a readable file".to_string());
    }
    let watermarks = renderable_watermarks(request);
    if watermarks.is_empty() {
        return Err("At least one visible watermark text is required before export".to_string());
    }

    let source_metadata = fs::metadata(&source_path)
        .map_err(|err| format!("Could not read source metadata: {err}"))?;
    let source_modified = source_metadata.modified().ok();
    let source_description = if matches!(
        request.output_rules.metadata_policy,
        MetadataPolicy::Preserve
    ) {
        read_image_description(&source_path).ok().flatten()
    } else {
        None
    };
    let mut image = load_source_image(&source_path)?.to_rgba8();
    let width = image.width();
    let height = image.height();

    for watermark in watermarks {
        render_text_watermark(&mut image, watermark)?;
    }

    let output_path = next_output_path(&source_path, &request.output_rules, request.index)?;
    write_image(&output_path, DynamicImage::ImageRgba8(image))?;
    if let Err(error) = apply_metadata_policy(
        &output_path,
        &request.output_rules,
        source_description.as_deref(),
    ) {
        let _ = fs::remove_file(&output_path);
        return Err(error);
    }

    if let Some(before) = source_modified {
        if let Ok(after) = fs::metadata(&source_path).and_then(|metadata| metadata.modified()) {
            if after != before {
                let _ = fs::remove_file(&output_path);
                return Err("Source image modified timestamp changed during export".to_string());
            }
        }
    }

    Ok(ExportResult {
        source_path: source_path.to_string_lossy().to_string(),
        output_path: output_path.to_string_lossy().to_string(),
        width,
        height,
    })
}

pub fn renderable_watermarks(request: &ExportSelectedRequest) -> Vec<&WatermarkExport> {
    let mut watermarks: Vec<&WatermarkExport> = request
        .watermarks
        .iter()
        .filter(|watermark| watermark.is_renderable())
        .collect();

    if watermarks.is_empty() {
        if let Some(watermark) = request
            .watermark
            .as_ref()
            .filter(|watermark| watermark.is_renderable())
        {
            watermarks.push(watermark);
        }
    }

    watermarks
}

pub(crate) fn load_source_image(source_path: &Path) -> Result<DynamicImage, String> {
    let mut decoder = ImageReader::open(source_path)
        .map_err(|err| format!("Could not open source image: {err}"))?
        .into_decoder()
        .map_err(|err| format!("Could not decode source image: {err}"))?;
    let orientation = decoder
        .orientation()
        .map_err(|err| format!("Could not read source orientation: {err}"))?;
    let mut image = DynamicImage::from_decoder(decoder)
        .map_err(|err| format!("Could not decode source image: {err}"))?;
    image.apply_orientation(orientation);
    Ok(image)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    use image::{ImageBuffer, Rgb, Rgba};

    use super::export_selected_image;
    use crate::domain::export::{
        BatchExportRequest, ExportSelectedRequest, MetadataPolicy, NamingRule, OutputRules,
        WatermarkAnchor, WatermarkExport,
    };
    use crate::processing::metadata::{apply_metadata_policy, read_image_description};

    #[test]
    fn exports_png_to_default_folder_and_preserves_dimensions_and_source() {
        let dir = test_dir("export_png");
        fs::create_dir_all(&dir).unwrap();
        let source = dir.join("sample.png");
        ImageBuffer::from_pixel(80, 120, Rgba([90_u8, 180, 210, 255]))
            .save(&source)
            .unwrap();
        let source_bytes = fs::read(&source).unwrap();
        let modified_before = fs::metadata(&source).unwrap().modified().unwrap();

        let result = export_selected_image(&request(source.to_string_lossy().as_ref())).unwrap();

        assert_eq!(result.width, 80);
        assert_eq!(result.height, 120);
        assert!(result
            .output_path
            .ends_with("Watermark export/sample_watermark.png"));
        assert_eq!(
            image::image_dimensions(&result.output_path).unwrap(),
            (80, 120)
        );
        assert_eq!(fs::read(&source).unwrap(), source_bytes);
        assert_eq!(
            fs::metadata(&source).unwrap().modified().unwrap(),
            modified_before
        );
    }

    #[test]
    fn exports_jpeg_and_uses_collision_safe_filename() {
        let dir = test_dir("export_jpeg");
        fs::create_dir_all(&dir).unwrap();
        let source = dir.join("sample.photo.jpg");
        ImageBuffer::from_pixel(96, 64, Rgb([200_u8, 220, 230]))
            .save(&source)
            .unwrap();

        let first = export_selected_image(&request(source.to_string_lossy().as_ref())).unwrap();
        let second = export_selected_image(&request(source.to_string_lossy().as_ref())).unwrap();

        assert!(first
            .output_path
            .ends_with("Watermark export/sample.photo_watermark.jpg"));
        assert!(second
            .output_path
            .ends_with("Watermark export/sample.photo_watermark-2.jpg"));
        assert_eq!(
            image::image_dimensions(&first.output_path).unwrap(),
            (96, 64)
        );
        assert_eq!(
            image::image_dimensions(&second.output_path).unwrap(),
            (96, 64)
        );
    }

    #[test]
    fn rejects_empty_watermark_text() {
        let dir = test_dir("empty_text");
        fs::create_dir_all(&dir).unwrap();
        let source = dir.join("sample.png");
        ImageBuffer::from_pixel(24, 24, Rgba([10_u8, 20, 30, 255]))
            .save(&source)
            .unwrap();
        let mut request = request(source.to_string_lossy().as_ref());
        request.watermarks[0].text = "   ".to_string();

        let error = export_selected_image(&request).unwrap_err();

        assert!(error.contains("At least one visible watermark text is required"));
    }

    #[test]
    fn supports_selected_output_folder_and_naming_rules() {
        let dir = test_dir("naming_rules");
        let output = dir.join("chosen");
        fs::create_dir_all(&dir).unwrap();
        let source = dir.join("apple.png");
        ImageBuffer::from_pixel(40, 60, Rgba([10_u8, 20, 30, 255]))
            .save(&source)
            .unwrap();

        let mut request = request(source.to_string_lossy().as_ref());
        request.output_rules = OutputRules {
            output_folder: Some(output.to_string_lossy().to_string()),
            naming_rule: NamingRule::WatermarkName,
            custom_prefix: "unused".to_string(),
            metadata_policy: Default::default(),
            description: String::new(),
        };
        let result = export_selected_image(&request).unwrap();
        assert!(result.output_path.ends_with("chosen/watermark_apple.png"));

        request.index = 7;
        request.output_rules.naming_rule = NamingRule::WatermarkIndex;
        let result = export_selected_image(&request).unwrap();
        assert!(result.output_path.ends_with("chosen/watermark_007.png"));

        request.index = 8;
        request.output_rules.naming_rule = NamingRule::CustomPrefixIndex;
        request.output_rules.custom_prefix = "bntxx".to_string();
        let result = export_selected_image(&request).unwrap();
        assert!(result.output_path.ends_with("chosen/bntxx_008.png"));
    }

    #[test]
    fn batch_export_continues_after_missing_file_failure() {
        use crate::domain::export::BatchExportRequest;

        let dir = test_dir("batch_partial");
        fs::create_dir_all(&dir).unwrap();
        let source = dir.join("ready.png");
        let missing = dir.join("missing.png");
        ImageBuffer::from_pixel(36, 36, Rgba([44_u8, 55, 66, 255]))
            .save(&source)
            .unwrap();

        let result = export_batch_for_test(&BatchExportRequest {
            source_paths: vec![
                source.to_string_lossy().to_string(),
                missing.to_string_lossy().to_string(),
            ],
            watermark: None,
            watermarks: request(source.to_string_lossy().as_ref()).watermarks,
            output_rules: Default::default(),
        });

        assert_eq!(result.completed, 1);
        assert_eq!(result.failed, 1);
        assert_eq!(result.results.len(), 2);
        assert!(result.results[0].success);
        assert!(!result.results[1].success);
    }

    #[test]
    fn exports_multiple_visible_watermarks() {
        let dir = test_dir("multi_watermarks");
        fs::create_dir_all(&dir).unwrap();
        let source = dir.join("sample.png");
        ImageBuffer::from_pixel(180, 140, Rgba([12_u8, 16, 20, 255]))
            .save(&source)
            .unwrap();

        let mut request = request(source.to_string_lossy().as_ref());
        request.watermarks = vec![
            WatermarkExport {
                color: "#ff0000".to_string(),
                text: "One".to_string(),
                x: 0.12,
                y: 0.22,
                anchor: WatermarkAnchor::Center,
                font_size_px: 32.0,
                opacity: 1.0,
                ..request.watermarks[0].clone()
            },
            WatermarkExport {
                color: "#0066ff".to_string(),
                text: "Two".to_string(),
                x: 0.78,
                y: 0.75,
                anchor: WatermarkAnchor::Center,
                font_size_px: 32.0,
                opacity: 1.0,
                ..request.watermarks[0].clone()
            },
        ];

        let result = export_selected_image(&request).unwrap();
        let output = image::open(&result.output_path).unwrap().to_rgba8();
        let mut red_pixels = 0;
        let mut blue_pixels = 0;
        for pixel in output.pixels() {
            let [red, green, blue, alpha] = pixel.0;
            if alpha > 0 && red > 140 && green < 90 && blue < 90 {
                red_pixels += 1;
            }
            if alpha > 0 && red < 90 && green < 150 && blue > 140 {
                blue_pixels += 1;
            }
        }

        assert!(red_pixels > 10, "expected red watermark pixels");
        assert!(blue_pixels > 10, "expected blue watermark pixels");
    }

    #[test]
    fn rewrites_png_description_metadata() {
        let dir = test_dir("png_description");
        fs::create_dir_all(&dir).unwrap();
        let source = dir.join("sample.png");
        ImageBuffer::from_pixel(44, 44, Rgba([10_u8, 20, 30, 255]))
            .save(&source)
            .unwrap();
        let mut request = request(source.to_string_lossy().as_ref());
        request.output_rules.metadata_policy =
            crate::domain::export::MetadataPolicy::RewriteDescription;
        request.output_rules.description = "fresh description".to_string();

        let result = export_selected_image(&request).unwrap();
        let output = fs::read(result.output_path).unwrap();

        assert!(output
            .windows(b"Description\0fresh description".len())
            .any(|window| window == b"Description\0fresh description"));
    }

    #[test]
    fn clears_png_description_metadata() {
        let dir = test_dir("png_clear_description");
        fs::create_dir_all(&dir).unwrap();
        let source = dir.join("sample.png");
        ImageBuffer::from_pixel(44, 44, Rgba([10_u8, 20, 30, 255]))
            .save(&source)
            .unwrap();
        let mut source_rules = OutputRules::default();
        source_rules.metadata_policy = MetadataPolicy::RewriteDescription;
        source_rules.description = "source png description".to_string();
        apply_metadata_policy(&source, &source_rules, None).unwrap();
        assert_eq!(
            read_image_description(&source).unwrap(),
            Some("source png description".to_string())
        );

        let mut request = request(source.to_string_lossy().as_ref());
        request.output_rules.metadata_policy = MetadataPolicy::ClearDescription;

        let result = export_selected_image(&request).unwrap();

        assert_eq!(
            read_image_description(Path::new(&result.output_path)).unwrap(),
            None
        );
    }

    #[test]
    fn preserves_png_description_metadata() {
        let dir = test_dir("png_preserve_description");
        fs::create_dir_all(&dir).unwrap();
        let source = dir.join("sample.png");
        ImageBuffer::from_pixel(44, 44, Rgba([10_u8, 20, 30, 255]))
            .save(&source)
            .unwrap();
        let mut source_rules = OutputRules::default();
        source_rules.metadata_policy = MetadataPolicy::RewriteDescription;
        source_rules.description = "source png description".to_string();
        apply_metadata_policy(&source, &source_rules, None).unwrap();

        let mut request = request(source.to_string_lossy().as_ref());
        request.output_rules.metadata_policy = MetadataPolicy::Preserve;

        let result = export_selected_image(&request).unwrap();

        assert_eq!(
            read_image_description(Path::new(&result.output_path)).unwrap(),
            Some("source png description".to_string())
        );
    }

    #[test]
    fn rewrites_jpeg_description_metadata() {
        let dir = test_dir("jpeg_description");
        fs::create_dir_all(&dir).unwrap();
        let source = dir.join("sample.jpg");
        ImageBuffer::from_pixel(44, 44, Rgb([10_u8, 20, 30]))
            .save(&source)
            .unwrap();
        let mut request = request(source.to_string_lossy().as_ref());
        request.output_rules.metadata_policy =
            crate::domain::export::MetadataPolicy::RewriteDescription;
        request.output_rules.description = "fresh jpeg description".to_string();

        let result = export_selected_image(&request).unwrap();
        let output = fs::read(result.output_path).unwrap();

        assert!(output
            .windows(b"fresh jpeg description".len())
            .any(|window| window == b"fresh jpeg description"));
    }

    #[test]
    fn clears_jpeg_description_metadata() {
        let dir = test_dir("jpeg_clear_description");
        fs::create_dir_all(&dir).unwrap();
        let source = dir.join("sample.jpg");
        ImageBuffer::from_pixel(44, 44, Rgb([10_u8, 20, 30]))
            .save(&source)
            .unwrap();
        let mut source_rules = OutputRules::default();
        source_rules.metadata_policy = MetadataPolicy::RewriteDescription;
        source_rules.description = "source jpeg description".to_string();
        apply_metadata_policy(&source, &source_rules, None).unwrap();
        assert_eq!(
            read_image_description(&source).unwrap(),
            Some("source jpeg description".to_string())
        );

        let mut request = request(source.to_string_lossy().as_ref());
        request.output_rules.metadata_policy = MetadataPolicy::ClearDescription;

        let result = export_selected_image(&request).unwrap();

        assert_eq!(
            read_image_description(Path::new(&result.output_path)).unwrap(),
            None
        );
    }

    #[test]
    fn preserves_jpeg_description_metadata() {
        let dir = test_dir("jpeg_preserve_description");
        fs::create_dir_all(&dir).unwrap();
        let source = dir.join("sample.jpg");
        ImageBuffer::from_pixel(44, 44, Rgb([10_u8, 20, 30]))
            .save(&source)
            .unwrap();
        let mut source_rules = OutputRules::default();
        source_rules.metadata_policy = MetadataPolicy::RewriteDescription;
        source_rules.description = "source jpeg description".to_string();
        apply_metadata_policy(&source, &source_rules, None).unwrap();

        let mut request = request(source.to_string_lossy().as_ref());
        request.output_rules.metadata_policy = MetadataPolicy::Preserve;

        let result = export_selected_image(&request).unwrap();

        assert_eq!(
            read_image_description(Path::new(&result.output_path)).unwrap(),
            Some("source jpeg description".to_string())
        );
    }

    #[test]
    fn removes_output_when_metadata_rewrite_fails() {
        let dir = test_dir("metadata_failure_cleanup");
        fs::create_dir_all(&dir).unwrap();
        let source = dir.join("sample.png");
        ImageBuffer::from_pixel(44, 44, Rgba([10_u8, 20, 30, 255]))
            .save(&source)
            .unwrap();
        let mut request = request(source.to_string_lossy().as_ref());
        request.output_rules.metadata_policy =
            crate::domain::export::MetadataPolicy::RewriteDescription;
        request.output_rules.description = "   ".to_string();

        let error = export_selected_image(&request).unwrap_err();

        assert!(error.contains("Description is required"));
        assert!(!dir.join("Watermark export/sample_watermark.png").exists());
    }

    #[test]
    fn applies_jpeg_exif_orientation_before_stripping_metadata() {
        let dir = test_dir("jpeg_orientation");
        fs::create_dir_all(&dir).unwrap();
        let source = dir.join("rotated.jpg");
        ImageBuffer::from_pixel(30, 50, Rgb([200_u8, 210, 220]))
            .save(&source)
            .unwrap();
        insert_jpeg_orientation(&source, 6);

        let result = export_selected_image(&request(source.to_string_lossy().as_ref())).unwrap();

        assert_eq!(result.width, 50);
        assert_eq!(result.height, 30);
        assert_eq!(
            image::image_dimensions(&result.output_path).unwrap(),
            (50, 30)
        );
    }

    fn export_batch_for_test(
        request: &BatchExportRequest,
    ) -> crate::domain::export::BatchExportResult {
        let mut results = Vec::new();

        for (offset, source_path) in request.source_paths.iter().enumerate() {
            let item_request = ExportSelectedRequest {
                source_path: source_path.clone(),
                watermark: request.watermark.clone(),
                watermarks: request.watermarks.clone(),
                output_rules: request.output_rules.clone(),
                index: offset + 1,
            };
            let item = match export_selected_image(&item_request) {
                Ok(result) => crate::domain::export::BatchExportItemResult {
                    source_path: result.source_path,
                    output_path: Some(result.output_path),
                    width: Some(result.width),
                    height: Some(result.height),
                    success: true,
                    error: None,
                },
                Err(error) => crate::domain::export::BatchExportItemResult {
                    source_path: source_path.clone(),
                    output_path: None,
                    width: None,
                    height: None,
                    success: false,
                    error: Some(error),
                },
            };
            results.push(item);
        }

        let completed = results.iter().filter(|result| result.success).count();
        let failed = results.len().saturating_sub(completed);

        crate::domain::export::BatchExportResult {
            completed,
            failed,
            results,
        }
    }

    fn request(source_path: &str) -> ExportSelectedRequest {
        ExportSelectedRequest {
            source_path: source_path.to_string(),
            output_rules: Default::default(),
            index: 1,
            watermark: None,
            watermarks: vec![WatermarkExport {
                visible: true,
                text: "@bntxx_".to_string(),
                font_family: "Arial".to_string(),
                color: "#ffffff".to_string(),
                opacity: 0.85,
                font_size_px: 47.0,
                font_size_percent: 3.2,
                rotation_degrees: 0.0,
                x: 0.5,
                y: 0.92,
                anchor: WatermarkAnchor::BottomCenter,
            }],
        }
    }

    fn test_dir(name: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("watermark_{name}_{stamp}"))
    }

    fn insert_jpeg_orientation(path: &std::path::Path, orientation: u16) {
        let bytes = fs::read(path).unwrap();
        assert_eq!(&bytes[..2], &[0xff, 0xd8]);

        let mut tiff = Vec::new();
        tiff.extend_from_slice(b"MM");
        tiff.extend_from_slice(&42_u16.to_be_bytes());
        tiff.extend_from_slice(&8_u32.to_be_bytes());
        tiff.extend_from_slice(&1_u16.to_be_bytes());
        tiff.extend_from_slice(&0x0112_u16.to_be_bytes());
        tiff.extend_from_slice(&3_u16.to_be_bytes());
        tiff.extend_from_slice(&1_u32.to_be_bytes());
        tiff.extend_from_slice(&orientation.to_be_bytes());
        tiff.extend_from_slice(&0_u16.to_be_bytes());
        tiff.extend_from_slice(&0_u32.to_be_bytes());

        let mut payload = b"Exif\0\0".to_vec();
        payload.extend_from_slice(&tiff);
        let mut app1 = vec![0xff, 0xe1];
        app1.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
        app1.extend_from_slice(&payload);

        let mut output = Vec::new();
        output.extend_from_slice(&bytes[..2]);
        output.extend_from_slice(&app1);
        output.extend_from_slice(&bytes[2..]);
        fs::write(path, output).unwrap();
    }
}

fn next_output_path(
    source_path: &Path,
    output_rules: &OutputRules,
    index: usize,
) -> Result<PathBuf, String> {
    let output_folder = if let Some(output_folder) = output_rules
        .output_folder
        .as_ref()
        .filter(|folder| !folder.trim().is_empty())
    {
        PathBuf::from(output_folder)
    } else {
        let source_folder = source_path
            .parent()
            .ok_or_else(|| "Could not determine source folder".to_string())?;
        source_folder.join(DEFAULT_EXPORT_FOLDER)
    };
    fs::create_dir_all(&output_folder)
        .map_err(|err| format!("Could not create export folder: {err}"))?;

    let stem = source_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("image");
    let extension = source_path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("png");
    let base_name = output_base_name(stem, extension, output_rules, index);
    let mut candidate = output_folder.join(&base_name);
    let mut counter = 2;

    while candidate.exists() {
        candidate = output_folder.join(append_collision_suffix(&base_name, counter));
        counter += 1;
    }

    Ok(candidate)
}

fn output_base_name(
    stem: &str,
    extension: &str,
    output_rules: &OutputRules,
    index: usize,
) -> String {
    let index_value = format!("{index:03}");
    let prefix = sanitize_prefix(&output_rules.custom_prefix);

    match output_rules.naming_rule {
        NamingRule::NameWatermark => format!("{stem}_watermark.{extension}"),
        NamingRule::WatermarkName => format!("watermark_{stem}.{extension}"),
        NamingRule::WatermarkIndex => format!("watermark_{index_value}.{extension}"),
        NamingRule::CustomPrefixIndex => format!("{prefix}_{index_value}.{extension}"),
    }
}

fn append_collision_suffix(filename: &str, counter: usize) -> String {
    let path = Path::new(filename);
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("image");
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("png");

    format!("{stem}-{counter}.{extension}")
}

fn sanitize_prefix(prefix: &str) -> String {
    let trimmed = prefix.trim();
    if trimmed.is_empty() {
        return "watermark".to_string();
    }

    trimmed
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric()
                || character == '-'
                || character == '_'
                || character == '@'
            {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn write_image(path: &Path, image: DynamicImage) -> Result<(), String> {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .unwrap_or_else(|| "png".to_string());

    if extension == "jpg" || extension == "jpeg" {
        let file = fs::File::create(path)
            .map_err(|err| format!("Could not create output image: {err}"))?;
        let mut encoder = JpegEncoder::new_with_quality(file, JPEG_QUALITY);
        encoder
            .encode_image(&image)
            .map_err(|err| format!("Could not encode JPEG output: {err}"))?;
        return Ok(());
    }

    image
        .save(path)
        .map_err(|err| format!("Could not save output image: {err}"))
}
