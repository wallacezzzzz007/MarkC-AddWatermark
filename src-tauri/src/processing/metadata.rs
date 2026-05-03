use std::fs;
use std::path::Path;

use crate::domain::export::{MetadataPolicy, OutputRules};

const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
const DESCRIPTION_KEYWORD: &[u8] = b"Description";

pub fn apply_metadata_policy(
    output_path: &Path,
    output_rules: &OutputRules,
    source_description: Option<&str>,
) -> Result<(), String> {
    match output_rules.metadata_policy {
        MetadataPolicy::ClearDescription => Ok(()),
        MetadataPolicy::Preserve => {
            if let Some(description) = source_description.filter(|value| !value.trim().is_empty()) {
                write_description(output_path, description)
            } else {
                Ok(())
            }
        }
        MetadataPolicy::RewriteDescription => {
            let description = output_rules.description.trim();
            if description.is_empty() {
                return Err("Description is required when metadata rewrite is selected".to_string());
            }

            write_description(output_path, description)
        }
    }
}

pub fn read_image_description(path: &Path) -> Result<Option<String>, String> {
    match extension(path).as_deref() {
        Some("jpg") | Some("jpeg") => read_jpeg_description(path),
        Some("png") => read_png_description(path),
        _ => Ok(None),
    }
}

fn write_description(path: &Path, description: &str) -> Result<(), String> {
    match extension(path).as_deref() {
        Some("jpg") | Some("jpeg") => rewrite_jpeg_description(path, description),
        Some("png") => rewrite_png_description(path, description),
        _ => Ok(()),
    }
}

fn read_jpeg_description(path: &Path) -> Result<Option<String>, String> {
    let bytes = fs::read(path).map_err(|err| format!("Could not read JPEG metadata: {err}"))?;
    if bytes.len() < 4 || bytes[0] != 0xff || bytes[1] != 0xd8 {
        return Err("Source is not a valid JPEG file".to_string());
    }

    let mut cursor = 2;
    while cursor + 4 <= bytes.len() && bytes[cursor] == 0xff {
        let marker = bytes[cursor + 1];
        if marker == 0xda || marker == 0xd9 || (0xd0..=0xd7).contains(&marker) {
            break;
        }

        let length = u16::from_be_bytes([bytes[cursor + 2], bytes[cursor + 3]]) as usize;
        if length < 2 || cursor + 2 + length > bytes.len() {
            break;
        }

        let payload_start = cursor + 4;
        let segment_end = cursor + 2 + length;
        let payload = &bytes[payload_start..segment_end];
        if marker == 0xe1 && payload.starts_with(b"Exif\0\0") {
            if let Some(description) = read_exif_image_description(&payload[6..]) {
                return Ok(Some(description));
            }
        }
        cursor = segment_end;
    }

    Ok(None)
}

fn read_exif_image_description(tiff: &[u8]) -> Option<String> {
    if tiff.len() < 14 {
        return None;
    }

    let big_endian = match tiff.get(0..2) {
        Some(b"MM") => true,
        Some(b"II") => false,
        _ => return None,
    };
    if read_u16(tiff, 2, big_endian)? != 42 {
        return None;
    }

    let ifd_offset = read_u32(tiff, 4, big_endian)? as usize;
    let entry_count = read_u16(tiff, ifd_offset, big_endian)? as usize;
    let entries_start = ifd_offset + 2;

    for index in 0..entry_count {
        let entry = entries_start + index * 12;
        if entry + 12 > tiff.len() {
            return None;
        }

        let tag = read_u16(tiff, entry, big_endian)?;
        let field_type = read_u16(tiff, entry + 2, big_endian)?;
        let count = read_u32(tiff, entry + 4, big_endian)? as usize;
        if tag != 0x010e || field_type != 2 || count == 0 {
            continue;
        }

        let bytes = if count <= 4 {
            tiff.get(entry + 8..entry + 8 + count)?
        } else {
            let offset = read_u32(tiff, entry + 8, big_endian)? as usize;
            tiff.get(offset..offset + count)?
        };
        let text = bytes.strip_suffix(&[0]).unwrap_or(bytes);
        return Some(String::from_utf8_lossy(text).to_string());
    }

    None
}

fn read_u16(bytes: &[u8], offset: usize, big_endian: bool) -> Option<u16> {
    let value = bytes.get(offset..offset + 2)?;
    Some(if big_endian {
        u16::from_be_bytes([value[0], value[1]])
    } else {
        u16::from_le_bytes([value[0], value[1]])
    })
}

fn read_u32(bytes: &[u8], offset: usize, big_endian: bool) -> Option<u32> {
    let value = bytes.get(offset..offset + 4)?;
    Some(if big_endian {
        u32::from_be_bytes([value[0], value[1], value[2], value[3]])
    } else {
        u32::from_le_bytes([value[0], value[1], value[2], value[3]])
    })
}

fn read_png_description(path: &Path) -> Result<Option<String>, String> {
    let bytes = fs::read(path).map_err(|err| format!("Could not read PNG metadata: {err}"))?;
    if bytes.len() < PNG_SIGNATURE.len() || &bytes[..PNG_SIGNATURE.len()] != PNG_SIGNATURE {
        return Err("Source is not a valid PNG file".to_string());
    }

    let mut cursor = PNG_SIGNATURE.len();
    while cursor + 12 <= bytes.len() {
        let length = u32::from_be_bytes([
            bytes[cursor],
            bytes[cursor + 1],
            bytes[cursor + 2],
            bytes[cursor + 3],
        ]) as usize;
        let chunk_type_start = cursor + 4;
        let data_start = cursor + 8;
        let chunk_end = data_start + length + 4;
        if chunk_end > bytes.len() {
            return Err("PNG metadata chunks are malformed".to_string());
        }

        let chunk_type = &bytes[chunk_type_start..chunk_type_start + 4];
        let data = &bytes[data_start..data_start + length];
        if chunk_type == b"tEXt" && png_text_keyword(data) == DESCRIPTION_KEYWORD {
            let text_start = data
                .iter()
                .position(|byte| *byte == 0)
                .map(|position| position + 1)
                .unwrap_or(data.len());
            return Ok(Some(
                String::from_utf8_lossy(&data[text_start..]).to_string(),
            ));
        }

        cursor = chunk_end;
        if chunk_type == b"IEND" {
            break;
        }
    }

    Ok(None)
}

fn rewrite_jpeg_description(path: &Path, description: &str) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|err| format!("Could not read JPEG metadata: {err}"))?;
    if bytes.len() < 4 || bytes[0] != 0xff || bytes[1] != 0xd8 {
        return Err("Output is not a valid JPEG file".to_string());
    }

    let app1 = jpeg_exif_description_segment(description)?;
    let mut output = Vec::with_capacity(bytes.len() + app1.len());
    output.extend_from_slice(&bytes[..2]);
    output.extend_from_slice(&app1);

    let mut cursor = 2;
    while cursor + 4 <= bytes.len() && bytes[cursor] == 0xff {
        let marker = bytes[cursor + 1];
        if marker == 0xda || marker == 0xd9 || (0xd0..=0xd7).contains(&marker) {
            break;
        }

        let length = u16::from_be_bytes([bytes[cursor + 2], bytes[cursor + 3]]) as usize;
        if length < 2 || cursor + 2 + length > bytes.len() {
            break;
        }

        let payload_start = cursor + 4;
        let segment_end = cursor + 2 + length;
        let is_exif = marker == 0xe1
            && bytes
                .get(payload_start..payload_start + 6)
                .is_some_and(|payload| payload == b"Exif\0\0");

        if !is_exif {
            output.extend_from_slice(&bytes[cursor..segment_end]);
        }
        cursor = segment_end;
    }

    output.extend_from_slice(&bytes[cursor..]);
    fs::write(path, output).map_err(|err| format!("Could not write JPEG metadata: {err}"))
}

fn jpeg_exif_description_segment(description: &str) -> Result<Vec<u8>, String> {
    let mut ascii = description.as_bytes().to_vec();
    ascii.retain(|byte| *byte != 0);
    ascii.push(0);

    let count = ascii.len() as u32;
    let ifd_value_offset = 8 + 2 + 12 + 4;
    let mut tiff = Vec::new();
    tiff.extend_from_slice(b"MM");
    tiff.extend_from_slice(&42_u16.to_be_bytes());
    tiff.extend_from_slice(&8_u32.to_be_bytes());
    tiff.extend_from_slice(&1_u16.to_be_bytes());
    tiff.extend_from_slice(&0x010e_u16.to_be_bytes());
    tiff.extend_from_slice(&2_u16.to_be_bytes());
    tiff.extend_from_slice(&count.to_be_bytes());

    if count <= 4 {
        let mut inline = [0_u8; 4];
        inline[..ascii.len()].copy_from_slice(&ascii);
        tiff.extend_from_slice(&inline);
    } else {
        tiff.extend_from_slice(&(ifd_value_offset as u32).to_be_bytes());
    }
    tiff.extend_from_slice(&0_u32.to_be_bytes());
    if count > 4 {
        tiff.extend_from_slice(&ascii);
    }

    let mut payload = b"Exif\0\0".to_vec();
    payload.extend_from_slice(&tiff);
    let segment_length = payload.len() + 2;
    if segment_length > u16::MAX as usize {
        return Err("Description metadata is too large for JPEG EXIF".to_string());
    }

    let mut segment = vec![0xff, 0xe1];
    segment.extend_from_slice(&(segment_length as u16).to_be_bytes());
    segment.extend_from_slice(&payload);
    Ok(segment)
}

fn rewrite_png_description(path: &Path, description: &str) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|err| format!("Could not read PNG metadata: {err}"))?;
    if bytes.len() < PNG_SIGNATURE.len() || &bytes[..PNG_SIGNATURE.len()] != PNG_SIGNATURE {
        return Err("Output is not a valid PNG file".to_string());
    }

    let mut output = Vec::with_capacity(bytes.len() + description.len() + 32);
    output.extend_from_slice(PNG_SIGNATURE);
    let mut cursor = PNG_SIGNATURE.len();
    let description_chunk = png_text_chunk(DESCRIPTION_KEYWORD, description.as_bytes())?;
    let mut inserted = false;

    while cursor + 12 <= bytes.len() {
        let length = u32::from_be_bytes([
            bytes[cursor],
            bytes[cursor + 1],
            bytes[cursor + 2],
            bytes[cursor + 3],
        ]) as usize;
        let chunk_type_start = cursor + 4;
        let data_start = cursor + 8;
        let chunk_end = data_start + length + 4;
        if chunk_end > bytes.len() {
            return Err("PNG metadata chunks are malformed".to_string());
        }

        let chunk_type = &bytes[chunk_type_start..chunk_type_start + 4];
        let data = &bytes[data_start..data_start + length];
        let is_description_text =
            chunk_type == b"tEXt" && png_text_keyword(data) == DESCRIPTION_KEYWORD;

        if chunk_type == b"IEND" && !inserted {
            output.extend_from_slice(&description_chunk);
            inserted = true;
        }

        if !is_description_text {
            output.extend_from_slice(&bytes[cursor..chunk_end]);
        }

        cursor = chunk_end;
        if chunk_type == b"IEND" {
            break;
        }
    }

    fs::write(path, output).map_err(|err| format!("Could not write PNG metadata: {err}"))
}

fn png_text_chunk(keyword: &[u8], text: &[u8]) -> Result<Vec<u8>, String> {
    if keyword.is_empty() || keyword.len() > 79 || keyword.contains(&0) {
        return Err("PNG metadata keyword is invalid".to_string());
    }

    let mut data = Vec::with_capacity(keyword.len() + 1 + text.len());
    data.extend_from_slice(keyword);
    data.push(0);
    data.extend_from_slice(text);

    let mut chunk = Vec::with_capacity(data.len() + 12);
    chunk.extend_from_slice(&(data.len() as u32).to_be_bytes());
    chunk.extend_from_slice(b"tEXt");
    chunk.extend_from_slice(&data);

    let mut crc_data = b"tEXt".to_vec();
    crc_data.extend_from_slice(&data);
    chunk.extend_from_slice(&crc32(&crc_data).to_be_bytes());
    Ok(chunk)
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff_u32;
    for byte in bytes {
        crc ^= *byte as u32;
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn png_text_keyword(data: &[u8]) -> &[u8] {
    let end = data
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(data.len());
    &data[..end]
}

fn extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use image::{ImageBuffer, Rgb, Rgba};

    use super::{read_image_description, rewrite_jpeg_description, rewrite_png_description};

    #[test]
    fn writes_png_description_text_chunk() {
        let path = test_dir("png_text").join("sample.png");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        ImageBuffer::from_pixel(16, 16, Rgba([1_u8, 2, 3, 255]))
            .save(&path)
            .unwrap();

        rewrite_png_description(&path, "custom description").unwrap();
        let bytes = fs::read(&path).unwrap();

        assert!(bytes
            .windows(b"Description\0custom description".len())
            .any(|window| window == b"Description\0custom description"));
        assert_eq!(
            read_image_description(&path).unwrap(),
            Some("custom description".to_string())
        );
    }

    #[test]
    fn writes_jpeg_image_description_tag() {
        let path = test_dir("jpeg_exif").join("sample.jpg");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        ImageBuffer::from_pixel(16, 16, Rgb([1_u8, 2, 3]))
            .save(&path)
            .unwrap();

        rewrite_jpeg_description(&path, "custom description").unwrap();
        let bytes = fs::read(&path).unwrap();

        assert!(bytes
            .windows(b"custom description".len())
            .any(|window| window == b"custom description"));
        assert_eq!(
            read_image_description(&path).unwrap(),
            Some("custom description".to_string())
        );
    }

    fn test_dir(name: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("watermark_metadata_{name}_{stamp}"))
    }
}
