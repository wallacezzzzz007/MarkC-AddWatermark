use crate::domain::export::{
    BatchExportProgressEvent, BatchExportRequest, BatchExportResult, ExportResult,
    ExportSelectedRequest,
};
use crate::processing::exporter;
use tauri::Emitter;

#[tauri::command]
pub fn export_selected_image(request: ExportSelectedRequest) -> Result<ExportResult, String> {
    exporter::export_selected_image(&request)
}

#[tauri::command]
pub fn export_batch(
    app: tauri::AppHandle,
    request: BatchExportRequest,
) -> Result<BatchExportResult, String> {
    if request.source_paths.is_empty() {
        return Err("Import images before batch export".to_string());
    }
    if request.watermark.text.trim().is_empty() {
        return Err("Watermark text is required before export".to_string());
    }

    let total = request.source_paths.len();
    let mut results = Vec::with_capacity(total);

    for (offset, source_path) in request.source_paths.iter().enumerate() {
        let item_request = ExportSelectedRequest {
            source_path: source_path.clone(),
            watermark: request.watermark.clone(),
            output_rules: request.output_rules.clone(),
            index: offset + 1,
        };
        let item = match exporter::export_selected_image(&item_request) {
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
        results.push(item.clone());

        let _ = app.emit(
            "batch_export_progress",
            BatchExportProgressEvent {
                completed: results.len(),
                total,
                current_path: Some(source_path.clone()),
                latest: Some(item),
            },
        );
    }

    let completed = results.iter().filter(|result| result.success).count();
    let failed = results.len().saturating_sub(completed);

    Ok(BatchExportResult {
        completed,
        failed,
        results,
    })
}
