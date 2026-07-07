use std::fs;

use ab_glyph::{point, Font, FontArc, GlyphId, PxScale, ScaleFont};
use image::{Rgba, RgbaImage};

use crate::domain::export::{WatermarkAnchor, WatermarkExport};

const DEFAULT_FONT_CANDIDATES: &[&str] = &[
    "/System/Library/Fonts/Supplemental/Arial.ttf",
    "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
    "/Library/Fonts/Arial.ttf",
    "C:\\Windows\\Fonts\\arial.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
];

pub fn render_text_watermark(
    image: &mut RgbaImage,
    watermark: &WatermarkExport,
) -> Result<(), String> {
    let text = watermark.text.trim();
    if text.is_empty() {
        return Ok(());
    }

    let font = load_font(&watermark.font_family)?;
    let font_size = resolved_font_size(image.height(), watermark);
    let scale = PxScale::from(font_size);
    let bounds = text_visual_bounds(scale, &font, text)
        .ok_or_else(|| "Watermark has no drawable glyphs".to_string())?;
    let mut metrics = text_layout_metrics(scale, &font, text);
    let (anchor_x, anchor_y) = anchor_point(image.width(), image.height(), watermark);
    let opacity = watermark.opacity.clamp(0.0, 1.0);
    let color = parse_hex_color(&watermark.color)?;
    let layer = rasterize_text_layer(color, scale, &font, text, opacity, bounds, &mut metrics);
    composite_rotated_layer(
        image,
        &layer,
        metrics,
        anchor_x,
        anchor_y,
        &watermark.anchor,
        watermark.rotation_degrees,
    );

    Ok(())
}

const TEXT_LAYER_PADDING: f32 = 4.0;

fn rasterize_text_layer(
    color: [u8; 3],
    scale: PxScale,
    font: &FontArc,
    text: &str,
    opacity: f32,
    bounds: TextBounds,
    metrics: &mut TextMetrics,
) -> RgbaImage {
    let padding = TEXT_LAYER_PADDING;
    let min_x = bounds.min_x.min(0.0) - padding;
    let min_y = bounds.min_y.min(0.0) - padding;
    let max_x = bounds.max_x.max(metrics.width) + padding;
    let max_y = bounds.max_y.max(metrics.height) + padding;
    let width = (max_x - min_x).ceil().max(1.0) as u32;
    let height = (max_y - min_y).ceil().max(1.0) as u32;
    let mut layer = RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 0]));
    let origin_x = (-min_x).round() as i32;
    let origin_y = (-min_y).round() as i32;
    metrics.layer_box_x = -min_x;
    metrics.layer_box_y = -min_y;

    layout_glyphs(scale, font, text, |glyph, glyph_bounds| {
        glyph.draw(|glyph_x, glyph_y, coverage| {
            let layer_x = glyph_x as i32 + origin_x + glyph_bounds.min.x.round() as i32;
            let layer_y = glyph_y as i32 + origin_y + glyph_bounds.min.y.round() as i32;

            if layer_x < 0
                || layer_y < 0
                || layer_x >= layer.width() as i32
                || layer_y >= layer.height() as i32
            {
                return;
            }

            let alpha = text_pixel_alpha(coverage, opacity);
            if alpha <= 0.0 {
                return;
            }
            write_text_layer_pixel(
                layer.get_pixel_mut(layer_x as u32, layer_y as u32),
                color,
                alpha,
            );
        });
    });

    layer
}

fn write_text_layer_pixel(pixel: &mut Rgba<u8>, color: [u8; 3], alpha: f32) {
    let alpha_byte = (alpha * 255.0).round().clamp(0.0, 255.0) as u8;
    pixel.0 = [color[0], color[1], color[2], pixel[3].max(alpha_byte)];
}

fn text_pixel_alpha(coverage: f32, opacity: f32) -> f32 {
    if opacity >= 0.995 {
        coverage.clamp(0.0, 1.0).powf(0.35)
    } else {
        (coverage * opacity).clamp(0.0, 1.0)
    }
}

fn composite_rotated_layer(
    image: &mut RgbaImage,
    layer: &RgbaImage,
    metrics: TextMetrics,
    anchor_x: f32,
    anchor_y: f32,
    anchor: &WatermarkAnchor,
    rotation_degrees: f32,
) {
    let radians = rotation_degrees.to_radians();
    let cos = radians.cos();
    let sin = radians.sin();
    let should_supersample = rotation_degrees.abs() > f32::EPSILON;
    let center_x = metrics.layer_box_x + metrics.width / 2.0;
    let center_y = metrics.layer_box_y + metrics.height / 2.0;
    let Some(rotated_alpha_bounds) =
        rotated_layer_alpha_bounds(layer, center_x, center_y, cos, sin)
    else {
        return;
    };
    let rotated_box_bounds = rotated_box_bounds(metrics.width, metrics.height, cos, sin);
    let anchor_bounds = if matches!(anchor, WatermarkAnchor::Custom) {
        rotated_box_bounds
    } else {
        rotated_alpha_bounds
    };
    let (dest_origin_x, dest_origin_y) = origin_for_anchor(
        anchor_x,
        anchor_y,
        anchor_bounds.width(),
        anchor_bounds.height(),
        anchor,
    );
    let world_center_x = dest_origin_x - anchor_bounds.min_x;
    let world_center_y = dest_origin_y - anchor_bounds.min_y;
    let dest_left = (world_center_x + rotated_alpha_bounds.min_x).floor() as i32;
    let dest_top = (world_center_y + rotated_alpha_bounds.min_y).floor() as i32;
    let dest_width = rotated_alpha_bounds.width().ceil().max(1.0) as i32 + 2;
    let dest_height = rotated_alpha_bounds.height().ceil().max(1.0) as i32 + 2;

    for dest_y in 0..dest_height {
        for dest_x in 0..dest_width {
            let image_x = dest_left + dest_x;
            let image_y = dest_top + dest_y;
            if image_x < 0
                || image_y < 0
                || image_x >= image.width() as i32
                || image_y >= image.height() as i32
            {
                continue;
            }
            let (world_x, world_y) = if should_supersample {
                (image_x as f32 + 0.5, image_y as f32 + 0.5)
            } else {
                (image_x as f32, image_y as f32)
            };

            let source = sample_rotated_layer(
                layer,
                world_x,
                world_y,
                world_center_x,
                world_center_y,
                center_x,
                center_y,
                cos,
                sin,
                should_supersample,
            );
            let alpha = source[3] as f32 / 255.0;
            if alpha <= 0.0 {
                continue;
            }

            let pixel = image.get_pixel_mut(image_x as u32, image_y as u32);
            blend_source_over(pixel, [source[0], source[1], source[2]], alpha);
        }
    }
}

fn sample_rotated_layer(
    layer: &RgbaImage,
    world_x: f32,
    world_y: f32,
    world_center_x: f32,
    world_center_y: f32,
    source_center_x: f32,
    source_center_y: f32,
    cos: f32,
    sin: f32,
    supersample: bool,
) -> [u8; 4] {
    if !supersample {
        return sample_rotated_layer_at(
            layer,
            world_x,
            world_y,
            world_center_x,
            world_center_y,
            source_center_x,
            source_center_y,
            cos,
            sin,
        );
    }

    let offsets = [-0.375_f32, 0.0, 0.375_f32];
    let mut alpha_sum = 0.0;
    let mut color = [0_u8, 0_u8, 0_u8];
    let mut samples = 0.0;

    for offset_y in offsets {
        for offset_x in offsets {
            let sample = sample_rotated_layer_at(
                layer,
                world_x + offset_x,
                world_y + offset_y,
                world_center_x,
                world_center_y,
                source_center_x,
                source_center_y,
                cos,
                sin,
            );
            if sample[3] > 0 {
                color = [sample[0], sample[1], sample[2]];
            }
            alpha_sum += sample[3] as f32;
            samples += 1.0;
        }
    }

    [
        color[0],
        color[1],
        color[2],
        (alpha_sum / samples).round().clamp(0.0, 255.0) as u8,
    ]
}

fn sample_rotated_layer_at(
    layer: &RgbaImage,
    world_x: f32,
    world_y: f32,
    world_center_x: f32,
    world_center_y: f32,
    source_center_x: f32,
    source_center_y: f32,
    cos: f32,
    sin: f32,
) -> [u8; 4] {
    let relative_x = world_x - world_center_x;
    let relative_y = world_y - world_center_y;
    let source_x = relative_x * cos + relative_y * sin + source_center_x;
    let source_y = -relative_x * sin + relative_y * cos + source_center_y;
    sample_layer_bilinear(layer, source_x, source_y)
}

fn sample_layer_bilinear(layer: &RgbaImage, x: f32, y: f32) -> [u8; 4] {
    if x < 0.0 || y < 0.0 || x > (layer.width() - 1) as f32 || y > (layer.height() - 1) as f32 {
        return [0, 0, 0, 0];
    }

    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let x1 = (x0 + 1).min(layer.width() - 1);
    let y1 = (y0 + 1).min(layer.height() - 1);
    let tx = x - x0 as f32;
    let ty = y - y0 as f32;
    let p00 = layer.get_pixel(x0, y0).0;
    let p10 = layer.get_pixel(x1, y0).0;
    let p01 = layer.get_pixel(x0, y1).0;
    let p11 = layer.get_pixel(x1, y1).0;
    let alpha = bilinear_channel(p00[3], p10[3], p01[3], p11[3], tx, ty);
    let color = if p00[3] > 0 {
        [p00[0], p00[1], p00[2]]
    } else if p10[3] > 0 {
        [p10[0], p10[1], p10[2]]
    } else if p01[3] > 0 {
        [p01[0], p01[1], p01[2]]
    } else if p11[3] > 0 {
        [p11[0], p11[1], p11[2]]
    } else {
        [0, 0, 0]
    };

    [color[0], color[1], color[2], alpha]
}

fn bilinear_channel(p00: u8, p10: u8, p01: u8, p11: u8, tx: f32, ty: f32) -> u8 {
    let top = p00 as f32 * (1.0 - tx) + p10 as f32 * tx;
    let bottom = p01 as f32 * (1.0 - tx) + p11 as f32 * tx;
    (top * (1.0 - ty) + bottom * ty).round().clamp(0.0, 255.0) as u8
}

fn rotate_point(x: f32, y: f32, cos: f32, sin: f32) -> (f32, f32) {
    (x * cos - y * sin, x * sin + y * cos)
}

fn rotated_layer_alpha_bounds(
    layer: &RgbaImage,
    center_x: f32,
    center_y: f32,
    cos: f32,
    sin: f32,
) -> Option<TextBounds> {
    let mut bounds: Option<TextBounds> = None;

    for (x, y, pixel) in layer.enumerate_pixels() {
        if pixel[3] == 0 {
            continue;
        }

        let relative_x = x as f32 - center_x;
        let relative_y = y as f32 - center_y;
        let (rotated_x, rotated_y) = rotate_point(relative_x, relative_y, cos, sin);
        let pixel_bounds = TextBounds {
            min_x: rotated_x,
            min_y: rotated_y,
            max_x: rotated_x,
            max_y: rotated_y,
        };

        if let Some(current) = &mut bounds {
            current.min_x = current.min_x.min(pixel_bounds.min_x);
            current.min_y = current.min_y.min(pixel_bounds.min_y);
            current.max_x = current.max_x.max(pixel_bounds.max_x);
            current.max_y = current.max_y.max(pixel_bounds.max_y);
        } else {
            bounds = Some(pixel_bounds);
        }
    }

    bounds
}

fn rotated_box_bounds(width: f32, height: f32, cos: f32, sin: f32) -> TextBounds {
    let half_width = width / 2.0;
    let half_height = height / 2.0;
    let corners = [
        (-half_width, -half_height),
        (half_width, -half_height),
        (half_width, half_height),
        (-half_width, half_height),
    ];
    let mut bounds = TextBounds {
        min_x: f32::INFINITY,
        min_y: f32::INFINITY,
        max_x: f32::NEG_INFINITY,
        max_y: f32::NEG_INFINITY,
    };

    for (x, y) in corners {
        let (rotated_x, rotated_y) = rotate_point(x, y, cos, sin);
        bounds.min_x = bounds.min_x.min(rotated_x);
        bounds.min_y = bounds.min_y.min(rotated_y);
        bounds.max_x = bounds.max_x.max(rotated_x);
        bounds.max_y = bounds.max_y.max(rotated_y);
    }

    bounds
}

fn resolved_font_size(image_height: u32, watermark: &WatermarkExport) -> f32 {
    if watermark.font_size_px.is_finite() && watermark.font_size_px > 0.0 {
        watermark
            .font_size_px
            .clamp(8.0, image_height.max(8) as f32)
    } else {
        ((image_height as f32) * watermark.font_size_percent / 100.0)
            .clamp(8.0, image_height.max(8) as f32)
    }
}

fn text_layout_metrics(scale: PxScale, font: &FontArc, text: &str) -> TextMetrics {
    let scaled = font.as_scaled(scale);
    let mut width = 0.0;
    let mut last: Option<GlyphId> = None;

    for character in text.chars() {
        let glyph_id = scaled.glyph_id(character);
        if let Some(last_id) = last {
            width += scaled.kern(glyph_id, last_id);
        }
        width += scaled.h_advance(glyph_id);
        last = Some(glyph_id);
    }

    TextMetrics {
        width: width.max(1.0),
        height: scale.y.max(1.0),
        layer_box_x: 0.0,
        layer_box_y: 0.0,
    }
}

fn layout_glyphs(
    scale: PxScale,
    font: &FontArc,
    text: &str,
    mut draw: impl FnMut(ab_glyph::OutlinedGlyph, ab_glyph::Rect),
) {
    let scaled = font.as_scaled(scale);
    let mut cursor_x = 0.0;
    let mut last: Option<GlyphId> = None;

    for character in text.chars() {
        let glyph_id = scaled.glyph_id(character);
        if let Some(last_id) = last {
            cursor_x += scaled.kern(glyph_id, last_id);
        }

        let glyph = glyph_id.with_scale_and_position(scale, point(cursor_x, scaled.ascent()));
        cursor_x += scaled.h_advance(glyph_id);
        last = Some(glyph_id);

        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            draw(outlined, bounds);
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct TextBounds {
    min_x: f32,
    min_y: f32,
    max_x: f32,
    max_y: f32,
}

#[derive(Debug, Clone, Copy)]
struct TextMetrics {
    width: f32,
    height: f32,
    layer_box_x: f32,
    layer_box_y: f32,
}

impl TextBounds {
    fn width(self) -> f32 {
        self.max_x - self.min_x
    }

    fn height(self) -> f32 {
        self.max_y - self.min_y
    }

    fn include(&mut self, bounds: ab_glyph::Rect) {
        self.min_x = self.min_x.min(bounds.min.x);
        self.min_y = self.min_y.min(bounds.min.y);
        self.max_x = self.max_x.max(bounds.max.x);
        self.max_y = self.max_y.max(bounds.max.y);
    }
}

fn text_visual_bounds(scale: PxScale, font: &FontArc, text: &str) -> Option<TextBounds> {
    let mut bounds: Option<TextBounds> = None;

    layout_glyphs(scale, font, text, |_glyph, glyph_bounds| {
        if let Some(current) = &mut bounds {
            current.include(glyph_bounds);
        } else {
            bounds = Some(TextBounds {
                min_x: glyph_bounds.min.x,
                min_y: glyph_bounds.min.y,
                max_x: glyph_bounds.max.x,
                max_y: glyph_bounds.max.y,
            });
        }
    });

    bounds
}

fn blend_source_over(pixel: &mut Rgba<u8>, source: [u8; 3], source_alpha: f32) {
    let source_alpha = source_alpha.clamp(0.0, 1.0);
    if source_alpha <= 0.0 {
        return;
    }

    let destination = pixel.0;
    let destination_alpha = destination[3] as f32 / 255.0;
    let out_alpha = source_alpha + destination_alpha * (1.0 - source_alpha);

    if out_alpha <= 0.0 {
        return;
    }

    pixel.0 = [
        blend_channel_with_alpha(
            destination[0],
            destination_alpha,
            source[0],
            source_alpha,
            out_alpha,
        ),
        blend_channel_with_alpha(
            destination[1],
            destination_alpha,
            source[1],
            source_alpha,
            out_alpha,
        ),
        blend_channel_with_alpha(
            destination[2],
            destination_alpha,
            source[2],
            source_alpha,
            out_alpha,
        ),
        (out_alpha * 255.0).round().clamp(0.0, 255.0) as u8,
    ];
}

fn blend_channel_with_alpha(
    destination: u8,
    destination_alpha: f32,
    source: u8,
    source_alpha: f32,
    out_alpha: f32,
) -> u8 {
    (((source as f32 * source_alpha)
        + (destination as f32 * destination_alpha * (1.0 - source_alpha)))
        / out_alpha)
        .round()
        .clamp(0.0, 255.0) as u8
}

fn anchor_point(width: u32, height: u32, watermark: &WatermarkExport) -> (f32, f32) {
    (watermark.x * width as f32, watermark.y * height as f32)
}

fn origin_for_anchor(
    anchor_x: f32,
    anchor_y: f32,
    text_width: f32,
    text_height: f32,
    anchor: &WatermarkAnchor,
) -> (f32, f32) {
    match anchor {
        WatermarkAnchor::TopLeft => (anchor_x, anchor_y),
        WatermarkAnchor::TopRight => (anchor_x - text_width, anchor_y),
        WatermarkAnchor::TopCenter => (anchor_x - text_width / 2.0, anchor_y),
        WatermarkAnchor::CenterLeft => (anchor_x, anchor_y - text_height / 2.0),
        WatermarkAnchor::CenterRight => (anchor_x - text_width, anchor_y - text_height / 2.0),
        WatermarkAnchor::BottomLeft => (anchor_x, anchor_y - text_height),
        WatermarkAnchor::BottomRight => (anchor_x - text_width, anchor_y - text_height),
        WatermarkAnchor::BottomCenter => (anchor_x - text_width / 2.0, anchor_y - text_height),
        WatermarkAnchor::Center | WatermarkAnchor::Custom => {
            (anchor_x - text_width / 2.0, anchor_y - text_height / 2.0)
        }
    }
}

fn parse_hex_color(value: &str) -> Result<[u8; 3], String> {
    let hex = value.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return Err(format!("Invalid color '{value}'. Expected #RRGGBB."));
    }

    let red = u8::from_str_radix(&hex[0..2], 16).map_err(|err| err.to_string())?;
    let green = u8::from_str_radix(&hex[2..4], 16).map_err(|err| err.to_string())?;
    let blue = u8::from_str_radix(&hex[4..6], 16).map_err(|err| err.to_string())?;

    Ok([red, green, blue])
}

fn load_font(font_family: &str) -> Result<FontArc, String> {
    for candidate in font_candidates(font_family) {
        if let Ok(bytes) = fs::read(candidate) {
            return FontArc::try_from_vec(bytes)
                .map_err(|_| format!("Could not parse font at {candidate}"));
        }
    }

    Err("Could not find a usable system font for text rendering".to_string())
}

fn font_candidates(font_family: &str) -> Vec<&'static str> {
    let requested = font_family.trim().to_ascii_lowercase();
    let requested = requested.as_str();
    let mut candidates = match requested {
        "snell roundhand" => vec!["/System/Library/Fonts/Supplemental/SnellRoundhand.ttc"],
        "brush script mt" => vec![
            "/System/Library/Fonts/Supplemental/Brush Script.ttf",
            "C:\\Windows\\Fonts\\brushsci.ttf",
        ],
        "zapfino" => vec!["/System/Library/Fonts/Supplemental/Zapfino.ttf"],
        "georgia" => vec![
            "/System/Library/Fonts/Supplemental/Georgia.ttf",
            "/Library/Fonts/Georgia.ttf",
            "C:\\Windows\\Fonts\\georgia.ttf",
            "/usr/share/fonts/truetype/msttcorefonts/Georgia.ttf",
        ],
        "baskerville" => vec!["/System/Library/Fonts/Supplemental/Baskerville.ttc"],
        "didot" => vec!["/System/Library/Fonts/Supplemental/Didot.ttc"],
        "hoefler text" => vec!["/System/Library/Fonts/Supplemental/Hoefler Text.ttc"],
        "palatino" => vec![
            "/System/Library/Fonts/Palatino.ttc",
            "C:\\Windows\\Fonts\\pala.ttf",
        ],
        "optima" => vec!["/System/Library/Fonts/Optima.ttc"],
        "futura" => vec!["/System/Library/Fonts/Supplemental/Futura.ttc"],
        "copperplate" => vec!["/System/Library/Fonts/Supplemental/Copperplate.ttc"],
        "times new roman" => vec![
            "/System/Library/Fonts/Supplemental/Times New Roman.ttf",
            "/Library/Fonts/Times New Roman.ttf",
            "C:\\Windows\\Fonts\\times.ttf",
            "/usr/share/fonts/truetype/msttcorefonts/Times_New_Roman.ttf",
        ],
        "courier new" => vec![
            "/System/Library/Fonts/Supplemental/Courier New.ttf",
            "/Library/Fonts/Courier New.ttf",
            "C:\\Windows\\Fonts\\cour.ttf",
            "/usr/share/fonts/truetype/msttcorefonts/Courier_New.ttf",
        ],
        "verdana" => vec![
            "/System/Library/Fonts/Supplemental/Verdana.ttf",
            "/Library/Fonts/Verdana.ttf",
            "C:\\Windows\\Fonts\\verdana.ttf",
            "/usr/share/fonts/truetype/msttcorefonts/Verdana.ttf",
        ],
        "trebuchet ms" => vec![
            "/System/Library/Fonts/Supplemental/Trebuchet MS.ttf",
            "/Library/Fonts/Trebuchet MS.ttf",
            "C:\\Windows\\Fonts\\trebuc.ttf",
            "/usr/share/fonts/truetype/msttcorefonts/Trebuchet_MS.ttf",
        ],
        "comic sans ms" => vec![
            "/System/Library/Fonts/Supplemental/Comic Sans MS.ttf",
            "/Library/Fonts/Comic Sans MS.ttf",
            "C:\\Windows\\Fonts\\comic.ttf",
            "/usr/share/fonts/truetype/msttcorefonts/Comic_Sans_MS.ttf",
        ],
        "noteworthy" => vec!["/System/Library/Fonts/Noteworthy.ttc"],
        "marker felt" => vec!["/System/Library/Fonts/MarkerFelt.ttc"],
        "chalkduster" => vec!["/System/Library/Fonts/Supplemental/Chalkduster.ttf"],
        "papyrus" => vec![
            "/System/Library/Fonts/Supplemental/Papyrus.ttc",
            "C:\\Windows\\Fonts\\papyrus.ttf",
        ],
        _ => Vec::new(),
    };
    candidates.extend(DEFAULT_FONT_CANDIDATES.iter().copied());
    candidates
}

#[cfg(test)]
mod tests {
    use image::{Rgba, RgbaImage};

    use super::render_text_watermark;
    use crate::domain::export::{WatermarkAnchor, WatermarkExport};

    #[test]
    fn opacity_zero_leaves_pixels_unchanged() {
        let mut image = RgbaImage::from_pixel(240, 160, Rgba([120, 160, 200, 255]));
        let original = image.clone();
        let mut watermark = watermark();
        watermark.opacity = 0.0;

        render_text_watermark(&mut image, &watermark).unwrap();

        assert_eq!(image, original);
    }

    #[test]
    fn opacity_one_changes_pixels() {
        let mut image = RgbaImage::from_pixel(240, 160, Rgba([120, 160, 200, 255]));
        let original = image.clone();
        let mut watermark = watermark();
        watermark.opacity = 1.0;

        render_text_watermark(&mut image, &watermark).unwrap();

        assert_ne!(image, original);
    }

    #[test]
    fn opacity_one_strengthens_transparent_output_alpha() {
        let original = RgbaImage::from_pixel(240, 160, Rgba([120, 160, 200, 80]));
        let mut image = original.clone();
        let mut watermark = watermark();
        watermark.text = "Alpha".to_string();
        watermark.opacity = 1.0;
        watermark.font_size_px = 42.0;

        render_text_watermark(&mut image, &watermark).unwrap();

        let changed = changed_bounds(&original, &image).unwrap();
        let mut saw_stronger_alpha = false;
        for y in changed.min_y..=changed.max_y {
            for x in changed.min_x..=changed.max_x {
                if image.get_pixel(x, y)[3] > original.get_pixel(x, y)[3] {
                    saw_stronger_alpha = true;
                    break;
                }
            }
        }

        assert!(saw_stronger_alpha);
    }

    #[test]
    fn opacity_one_writes_pure_source_color() {
        let original = RgbaImage::from_pixel(240, 160, Rgba([120, 160, 200, 255]));
        let mut image = original.clone();
        let mut watermark = watermark();
        watermark.text = "Pure".to_string();
        watermark.opacity = 1.0;
        watermark.font_size_px = 42.0;

        render_text_watermark(&mut image, &watermark).unwrap();
        let changed = changed_bounds(&original, &image).unwrap();

        let mut pure_pixels = 0usize;
        let mut smoothed_pixels = 0usize;
        for y in changed.min_y..=changed.max_y {
            for x in changed.min_x..=changed.max_x {
                let pixel = image.get_pixel(x, y).0;
                if pixel == original.get_pixel(x, y).0 {
                    continue;
                }
                if pixel == [255, 255, 255, 255] {
                    pure_pixels += 1;
                } else if pixel[0] > original.get_pixel(x, y)[0]
                    && pixel[1] > original.get_pixel(x, y)[1]
                    && pixel[2] > original.get_pixel(x, y)[2]
                {
                    smoothed_pixels += 1;
                }
            }
        }

        assert!(pure_pixels > 0);
        assert!(smoothed_pixels > 0);
    }

    #[test]
    fn overlapping_text_pixels_keep_the_stronger_alpha() {
        let mut pixel = Rgba([255, 255, 255, 230]);

        super::write_text_layer_pixel(&mut pixel, [255, 255, 255], 0.08);

        assert_eq!(pixel.0, [255, 255, 255, 230]);
    }

    #[test]
    fn render_does_not_add_shadow_pixels() {
        let original = RgbaImage::from_pixel(320, 220, Rgba([180, 180, 180, 255]));
        let mut image = original.clone();
        let mut watermark = watermark();
        watermark.text = "Test".to_string();
        watermark.color = "#ff6600".to_string();
        watermark.opacity = 1.0;
        watermark.font_size_percent = 18.0;

        render_text_watermark(&mut image, &watermark).unwrap();

        for (x, y, pixel) in image.enumerate_pixels() {
            if pixel == original.get_pixel(x, y) {
                continue;
            }

            assert!(
                pixel[0] >= 180,
                "unexpected dark shadow-like pixel at {x},{y}: {:?}",
                pixel.0
            );
        }
    }

    #[test]
    fn font_size_px_overrides_percent() {
        let original = RgbaImage::from_pixel(320, 240, Rgba([20, 30, 40, 255]));
        let mut px_image = original.clone();
        let mut percent_image = original.clone();
        let mut px_watermark = watermark();
        px_watermark.text = "Size".to_string();
        px_watermark.font_size_px = 24.0;
        px_watermark.font_size_percent = 20.0;
        px_watermark.opacity = 1.0;

        let mut percent_watermark = px_watermark.clone();
        percent_watermark.font_size_px = 0.0;

        render_text_watermark(&mut px_image, &px_watermark).unwrap();
        render_text_watermark(&mut percent_image, &percent_watermark).unwrap();

        let px_bounds = changed_bounds(&original, &px_image).unwrap();
        let percent_bounds = changed_bounds(&original, &percent_image).unwrap();

        assert!(
            px_bounds.height() < percent_bounds.height(),
            "px height={} percent height={}",
            px_bounds.height(),
            percent_bounds.height()
        );
    }

    #[test]
    fn rotated_watermark_renders_and_clips() {
        let original = RgbaImage::from_pixel(320, 240, Rgba([20, 30, 40, 255]));
        let mut image = original.clone();
        let mut watermark = watermark();
        watermark.text = "Rotate".to_string();
        watermark.font_size_px = 42.0;
        watermark.rotation_degrees = 45.0;
        watermark.opacity = 1.0;
        watermark.x = 0.5;
        watermark.y = 0.5;
        watermark.anchor = WatermarkAnchor::Center;

        render_text_watermark(&mut image, &watermark).unwrap();

        assert!(changed_bounds(&original, &image).is_some());

        let mut clipped = original.clone();
        watermark.x = 1.02;
        watermark.y = 1.02;
        watermark.anchor = WatermarkAnchor::Custom;
        render_text_watermark(&mut clipped, &watermark).unwrap();

        assert!(changed_bounds(&original, &clipped).is_some());
    }

    #[test]
    fn rotated_opacity_one_keeps_clean_white_core_pixels() {
        let original = RgbaImage::from_pixel(816, 1456, Rgba([154, 139, 177, 255]));
        let mut image = original.clone();
        let mut watermark = watermark();
        watermark.text = "@test".to_string();
        watermark.font_family = "Snell Roundhand".to_string();
        watermark.font_size_px = 86.0;
        watermark.rotation_degrees = -36.0;
        watermark.opacity = 1.0;
        watermark.x = 0.48;
        watermark.y = 0.34;
        watermark.anchor = WatermarkAnchor::Custom;

        render_text_watermark(&mut image, &watermark).unwrap();

        let pure_white_pixels = image
            .pixels()
            .filter(|pixel| pixel.0 == [255, 255, 255, 255])
            .count();

        assert!(
            pure_white_pixels > 40,
            "rotated export should keep solid white text cores, saw {pure_white_pixels}"
        );
    }

    #[test]
    fn rotated_bottom_right_preset_can_touch_visible_edges() {
        let original = RgbaImage::from_pixel(320, 240, Rgba([20, 30, 40, 255]));
        let mut image = original.clone();
        let mut watermark = watermark();
        watermark.text = "@bntxx_".to_string();
        watermark.font_size_px = 42.0;
        watermark.rotation_degrees = 45.0;
        watermark.opacity = 1.0;
        watermark.x = 1.0;
        watermark.y = 1.0;
        watermark.anchor = WatermarkAnchor::BottomRight;

        render_text_watermark(&mut image, &watermark).unwrap();
        let bounds = changed_bounds(&original, &image).unwrap();

        assert!(bounds.max_x >= 316, "max_x={}", bounds.max_x);
        assert!(bounds.max_y >= 236, "max_y={}", bounds.max_y);
    }

    #[test]
    fn bottom_left_preset_can_touch_visible_edges() {
        let original = RgbaImage::from_pixel(320, 240, Rgba([20, 30, 40, 255]));
        let mut image = original.clone();
        let mut watermark = watermark();
        watermark.text = "@bntxx_".to_string();
        watermark.font_size_px = 42.0;
        watermark.opacity = 1.0;
        watermark.x = 0.0;
        watermark.y = 1.0;
        watermark.anchor = WatermarkAnchor::BottomLeft;

        render_text_watermark(&mut image, &watermark).unwrap();
        let bounds = changed_bounds(&original, &image).unwrap();

        assert!(bounds.min_x <= 1, "min_x={}", bounds.min_x);
        assert!(bounds.max_y >= 238, "max_y={}", bounds.max_y);
    }

    #[test]
    fn custom_position_can_clip_at_edges() {
        let original = RgbaImage::from_pixel(320, 240, Rgba([20, 30, 40, 255]));
        let mut image = original.clone();
        let mut watermark = watermark();
        watermark.text = "@bntxx_".to_string();
        watermark.font_size_px = 42.0;
        watermark.rotation_degrees = -45.0;
        watermark.opacity = 1.0;
        watermark.x = 0.0;
        watermark.y = 1.0;
        watermark.anchor = WatermarkAnchor::Custom;

        render_text_watermark(&mut image, &watermark).unwrap();
        let bounds = changed_bounds(&original, &image).unwrap();

        assert!(bounds.min_x <= 1, "min_x={}", bounds.min_x);
        assert!(bounds.max_y >= 238, "max_y={}", bounds.max_y);
    }

    #[test]
    fn top_left_preset_can_touch_visible_edges() {
        let original = RgbaImage::from_pixel(320, 220, Rgba([20, 30, 40, 255]));
        let mut image = original.clone();
        let mut watermark = watermark();
        watermark.text = "Test".to_string();
        watermark.font_size_percent = 18.0;
        watermark.opacity = 1.0;
        watermark.x = 0.0;
        watermark.y = 0.0;
        watermark.anchor = WatermarkAnchor::TopLeft;

        render_text_watermark(&mut image, &watermark).unwrap();
        let bounds = changed_bounds(&original, &image).unwrap();

        assert!(bounds.min_y <= 2, "min_y={}", bounds.min_y);
    }

    #[test]
    fn center_anchor_keeps_layout_box_center_near_target() {
        let original = RgbaImage::from_pixel(320, 220, Rgba([20, 30, 40, 255]));
        let mut image = original.clone();
        let mut watermark = watermark();
        watermark.text = "Test".to_string();
        watermark.font_size_percent = 18.0;
        watermark.opacity = 1.0;
        watermark.x = 0.5;
        watermark.y = 0.5;
        watermark.anchor = WatermarkAnchor::Center;

        render_text_watermark(&mut image, &watermark).unwrap();
        let bounds = changed_bounds(&original, &image).unwrap();
        let center_x = (bounds.min_x + bounds.max_x) / 2;
        let center_y = (bounds.min_y + bounds.max_y) / 2;

        assert!((center_x as i32 - 160).abs() <= 12, "center_x={center_x}");
        assert!((center_y as i32 - 110).abs() <= 12, "center_y={center_y}");
    }

    fn watermark() -> WatermarkExport {
        WatermarkExport {
            visible: true,
            text: "@bntxx_".to_string(),
            font_family: "Arial".to_string(),
            color: "#ffffff".to_string(),
            opacity: 1.0,
            font_size_px: 0.0,
            font_size_percent: 18.0,
            rotation_degrees: 0.0,
            x: 0.5,
            y: 0.6,
            anchor: WatermarkAnchor::Center,
        }
    }

    #[derive(Debug)]
    struct PixelBounds {
        min_x: u32,
        min_y: u32,
        max_x: u32,
        max_y: u32,
    }

    impl PixelBounds {
        fn height(&self) -> u32 {
            self.max_y.saturating_sub(self.min_y) + 1
        }
    }

    fn changed_bounds(original: &RgbaImage, image: &RgbaImage) -> Option<PixelBounds> {
        let mut bounds: Option<PixelBounds> = None;

        for (x, y, pixel) in image.enumerate_pixels() {
            if pixel == original.get_pixel(x, y) {
                continue;
            }

            if let Some(current) = &mut bounds {
                current.min_x = current.min_x.min(x);
                current.min_y = current.min_y.min(y);
                current.max_x = current.max_x.max(x);
                current.max_y = current.max_y.max(y);
            } else {
                bounds = Some(PixelBounds {
                    min_x: x,
                    min_y: y,
                    max_x: x,
                    max_y: y,
                });
            }
        }

        bounds
    }
}
