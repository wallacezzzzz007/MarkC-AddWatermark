# MarkC

MarkC is an open-source desktop app for batch watermarking images. It is built
with Tauri, React, TypeScript, and Rust, so the app can ship as a small native
desktop utility while keeping the editing interface fast and familiar.

## Features

- Import one image, multiple images, or a whole folder.
- Drag images or folders directly into the app.
- Preview the selected image before export with fit, zoom, and before/after controls.
- Add one or more text watermarks to the same image.
- Select an individual watermark layer and tune its text, font, pixel size,
  opacity, rotation, and color.
- Use precise placement presets: top, center, bottom combined with left, center,
  and right.
- Drag each watermark freely, including partially outside the image edge. Export
  clips the watermark naturally at the image boundary.
- Use centerline guides and X/Y numeric controls for precise placement.
- Save reusable templates. Templates include the full watermark layer group,
  each layer's text, style, position, rotation, export naming, and metadata
  settings.
- Rename, duplicate, delete, and set a default template.
- Keep template names unique automatically with `-1`, `-2`, and later suffixes.
- Batch export all imported images with the same watermark layer group.
- Cancel a batch export between images.
- Choose an output folder, or use the default `Watermark export` folder beside
  the source images.
- Choose output naming rules, including original name plus watermark suffix,
  watermark prefix plus original name, indexed names, or a custom prefix.
- Preserve original image dimensions.
- Keep source files read-only.
- Handle JPEG EXIF orientation before writing the final output.
- Manage image description metadata:
  - Clear description metadata.
  - Rewrite description metadata.
  - Preserve description metadata where supported.

## Supported Images

MarkC currently supports:

- JPEG / JPG
- PNG

The app focuses on high-quality output while preserving the original image
dimensions. PNG exports are lossless by format. JPEG exports are re-encoded by
the image processing pipeline, so visual quality is kept high, but JPEG output is
not byte-for-byte identical to the source.

## Download And Install

End users do not need Node.js, pnpm, Rust, or Tauri installed. They should
download the macOS release package and install it like a normal desktop app.

Current release target:

- macOS: `.dmg`

Windows and Linux packages are not provided yet. Tauri can support those
platforms later, but each operating system needs its own build artifact.

### macOS

1. Download the latest MarkC `.dmg` from Releases.
2. Open the `.dmg`.
3. Drag MarkC into Applications.
4. Open MarkC from Applications.

If macOS Gatekeeper warns that the app is from an unidentified developer, the
release may need Apple Developer ID signing and notarization before public
distribution.

## Development

Requirements:

- Node.js
- pnpm
- Rust
- Tauri system prerequisites for your operating system

Install dependencies:

```bash
pnpm install
```

Run the desktop app in development mode:

```bash
pnpm tauri dev
```

Run frontend checks:

```bash
pnpm build
```

Run Rust tests:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

Build release artifacts for the current operating system:

```bash
pnpm tauri build
```

MarkC currently publishes macOS packages only. If Windows or Linux support is
added later, release packages should be built on their target operating systems,
usually through CI:

- macOS runner for `.dmg`
- Windows runner for `.msi` / `.exe`
- Linux runner for `.AppImage`, `.deb`, or `.rpm`

## Metadata Notes

MarkC intentionally limits metadata handling to description-like fields for the
MVP:

- PNG `tEXt Description`
- JPEG EXIF `ImageDescription`

`Clear` removes description metadata through the export pipeline. `Rewrite`
writes the description provided by the user. `Preserve` reads the source
description before export and writes it back to the output where supported.

## License

MarkC is released under the MIT License. See [LICENSE](LICENSE).
