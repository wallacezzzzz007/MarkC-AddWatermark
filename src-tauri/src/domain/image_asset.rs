use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageAsset {
    pub id: String,
    pub path: String,
    pub filename: String,
    pub extension: String,
    pub width: u32,
    pub height: u32,
    pub byte_size: u64,
    pub status: ImageStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageStatus {
    Ready,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedImport {
    pub path: String,
    pub reason: ImportSkipReason,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportSkipReason {
    UnsupportedFormat,
    CorruptImage,
    NotFile,
    NotDirectory,
    ReadError,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub accepted: Vec<ImageAsset>,
    pub skipped: Vec<SkippedImport>,
}
