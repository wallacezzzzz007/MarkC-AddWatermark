use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSelectedRequest {
    pub source_path: String,
    pub watermark: WatermarkExport,
    #[serde(default)]
    pub output_rules: OutputRules,
    #[serde(default = "default_index")]
    pub index: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatermarkExport {
    pub text: String,
    #[serde(default = "default_font_family")]
    pub font_family: String,
    pub color: String,
    pub opacity: f32,
    pub font_size_percent: f32,
    pub x: f32,
    pub y: f32,
    pub anchor: WatermarkAnchor,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WatermarkAnchor {
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    BottomCenter,
    BottomLeft,
    BottomRight,
    Center,
    CenterRight,
    Custom,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub source_path: String,
    pub output_path: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchExportRequest {
    pub source_paths: Vec<String>,
    pub watermark: WatermarkExport,
    pub output_rules: OutputRules,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchExportResult {
    pub completed: usize,
    pub failed: usize,
    pub results: Vec<BatchExportItemResult>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchExportProgressEvent {
    pub completed: usize,
    pub total: usize,
    pub current_path: Option<String>,
    pub latest: Option<BatchExportItemResult>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchExportItemResult {
    pub source_path: String,
    pub output_path: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputRules {
    #[serde(default)]
    pub output_folder: Option<String>,
    #[serde(default)]
    pub naming_rule: NamingRule,
    #[serde(default = "default_custom_prefix")]
    pub custom_prefix: String,
    #[serde(default)]
    pub metadata_policy: MetadataPolicy,
    #[serde(default)]
    pub description: String,
}

impl Default for OutputRules {
    fn default() -> Self {
        Self {
            output_folder: None,
            naming_rule: NamingRule::NameWatermark,
            custom_prefix: default_custom_prefix(),
            metadata_policy: MetadataPolicy::ClearDescription,
            description: String::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NamingRule {
    NameWatermark,
    WatermarkName,
    WatermarkIndex,
    CustomPrefixIndex,
}

impl Default for NamingRule {
    fn default() -> Self {
        Self::NameWatermark
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MetadataPolicy {
    ClearDescription,
    RewriteDescription,
    Preserve,
}

impl Default for MetadataPolicy {
    fn default() -> Self {
        Self::ClearDescription
    }
}

fn default_custom_prefix() -> String {
    "watermark".to_string()
}

fn default_font_family() -> String {
    "Arial".to_string()
}

fn default_index() -> usize {
    1
}
