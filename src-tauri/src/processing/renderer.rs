use std::fs;

use ab_glyph::{point, Font, FontArc, GlyphId, PxScale, ScaleFont};
use image::RgbaImage;

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
    let font_size = ((image.height() as f32) * watermark.font_size_percent / 100.0)
        .clamp(8.0, image.height() as f32);
    let scale = PxScale::from(font_size);
    let bounds = text_visual_bounds(scale, &font, text)
        .ok_or_else(|| "Watermark has no drawable glyphs".to_string())?;
    let text_width = bounds.width();
    let text_height = bounds.height();
    let (anchor_x, anchor_y) = anchor_point(image.width(), image.height(), watermark);
    let (visual_origin_x, visual_origin_y) = origin_for_anchor(
        anchor_x,
        anchor_y,
        text_width,
        text_height,
        &watermark.anchor,
    );

    let origin_x = (visual_origin_x - bounds.min_x).round() as i32;
    let origin_y = (visual_origin_y - bounds.min_y).round() as i32;
    let opacity = watermark.opacity.clamp(0.0, 1.0);
    let color = parse_hex_color(&watermark.color)?;

    draw_text_alpha_mut(
        image, color, origin_x, origin_y, scale, &font, text, opacity,
    );

    Ok(())
}

fn draw_text_alpha_mut(
    image: &mut RgbaImage,
    color: [u8; 3],
    x: i32,
    y: i32,
    scale: PxScale,
    font: &FontArc,
    text: &str,
    opacity: f32,
) {
    let opacity = opacity.clamp(0.0, 1.0);
    if opacity <= 0.0 {
        return;
    }

    layout_glyphs(scale, font, text, |glyph, bounds| {
        glyph.draw(|glyph_x, glyph_y, coverage| {
            let image_x = glyph_x as i32 + x + bounds.min.x.round() as i32;
            let image_y = glyph_y as i32 + y + bounds.min.y.round() as i32;

            if image_x < 0
                || image_y < 0
                || image_x >= image.width() as i32
                || image_y >= image.height() as i32
            {
                return;
            }

            let alpha = (coverage * opacity).clamp(0.0, 1.0);
            let pixel = image.get_pixel_mut(image_x as u32, image_y as u32);
            let dst = pixel.0;
            pixel.0 = [
                blend_channel(dst[0], color[0], alpha),
                blend_channel(dst[1], color[1], alpha),
                blend_channel(dst[2], color[2], alpha),
                dst[3],
            ];
        });
    });
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

fn blend_channel(destination: u8, source: u8, alpha: f32) -> u8 {
    ((destination as f32 * (1.0 - alpha)) + (source as f32 * alpha))
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
    fn preset_anchors_align_to_visual_text_bounds() {
        let anchors = [
            (WatermarkAnchor::TopLeft, 0.0, 0.0, Edge::TopLeft),
            (WatermarkAnchor::TopCenter, 0.5, 0.0, Edge::Top),
            (WatermarkAnchor::TopRight, 1.0, 0.0, Edge::TopRight),
            (WatermarkAnchor::CenterLeft, 0.0, 0.5, Edge::Left),
            (WatermarkAnchor::Center, 0.5, 0.5, Edge::Center),
            (WatermarkAnchor::CenterRight, 1.0, 0.5, Edge::Right),
            (WatermarkAnchor::BottomLeft, 0.0, 1.0, Edge::BottomLeft),
            (WatermarkAnchor::BottomCenter, 0.5, 1.0, Edge::Bottom),
            (WatermarkAnchor::BottomRight, 1.0, 1.0, Edge::BottomRight),
        ];

        for (anchor, x, y, edge) in anchors {
            let original = RgbaImage::from_pixel(320, 220, Rgba([20, 30, 40, 255]));
            let mut image = original.clone();
            let mut watermark = watermark();
            watermark.text = "Test".to_string();
            watermark.font_size_percent = 18.0;
            watermark.opacity = 1.0;
            watermark.x = x;
            watermark.y = y;
            watermark.anchor = anchor;

            render_text_watermark(&mut image, &watermark).unwrap();
            let bounds = changed_bounds(&original, &image).unwrap();

            match edge {
                Edge::Top => assert!(bounds.min_y <= 2, "{edge:?} min_y={}", bounds.min_y),
                Edge::Bottom => {
                    assert!(bounds.max_y >= 217, "{edge:?} max_y={}", bounds.max_y)
                }
                Edge::Left => assert!(bounds.min_x <= 2, "{edge:?} min_x={}", bounds.min_x),
                Edge::Right => assert!(bounds.max_x >= 317, "{edge:?} max_x={}", bounds.max_x),
                Edge::TopLeft => {
                    assert!(bounds.min_x <= 2, "{edge:?} min_x={}", bounds.min_x);
                    assert!(bounds.min_y <= 2, "{edge:?} min_y={}", bounds.min_y);
                }
                Edge::TopRight => {
                    assert!(bounds.max_x >= 317, "{edge:?} max_x={}", bounds.max_x);
                    assert!(bounds.min_y <= 2, "{edge:?} min_y={}", bounds.min_y);
                }
                Edge::BottomLeft => {
                    assert!(bounds.min_x <= 2, "{edge:?} min_x={}", bounds.min_x);
                    assert!(bounds.max_y >= 217, "{edge:?} max_y={}", bounds.max_y);
                }
                Edge::BottomRight => {
                    assert!(bounds.max_x >= 317, "{edge:?} max_x={}", bounds.max_x);
                    assert!(bounds.max_y >= 217, "{edge:?} max_y={}", bounds.max_y);
                }
                Edge::Center => {
                    let center_x = (bounds.min_x + bounds.max_x) / 2;
                    let center_y = (bounds.min_y + bounds.max_y) / 2;
                    assert!((center_x as i32 - 160).abs() <= 3, "center_x={center_x}");
                    assert!((center_y as i32 - 110).abs() <= 3, "center_y={center_y}");
                }
            }
        }
    }

    fn watermark() -> WatermarkExport {
        WatermarkExport {
            text: "@bntxx_".to_string(),
            font_family: "Arial".to_string(),
            color: "#ffffff".to_string(),
            opacity: 1.0,
            font_size_percent: 18.0,
            x: 0.5,
            y: 0.6,
            anchor: WatermarkAnchor::Center,
        }
    }

    #[derive(Debug, Clone, Copy)]
    enum Edge {
        Top,
        Bottom,
        Left,
        Right,
        TopLeft,
        TopRight,
        BottomLeft,
        BottomRight,
        Center,
    }

    #[derive(Debug)]
    struct PixelBounds {
        min_x: u32,
        min_y: u32,
        max_x: u32,
        max_y: u32,
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
