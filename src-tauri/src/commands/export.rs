use std::sync::atomic::{AtomicBool, Ordering};

use crate::domain::export::{
    BatchExportProgressEvent, BatchExportRequest, BatchExportResult, ExportResult,
    ExportSelectedRequest,
};
use crate::processing::exporter;
use tauri::Emitter;

static CANCEL_BATCH_EXPORT: AtomicBool = AtomicBool::new(false);

#[tauri::command]
pub fn export_selected_image(request: ExportSelectedRequest) -> Result<ExportResult, String> {
    exporter::export_selected_image(&request)
}

#[tauri::command]
pub fn cancel_batch_export() {
    CANCEL_BATCH_EXPORT.store(true, Ordering::SeqCst);
}

#[tauri::command]
pub fn export_batch(
    app: tauri::AppHandle,
    request: BatchExportRequest,
) -> Result<BatchExportResult, String> {
    if request.source_paths.is_empty() {
        return Err("Import images before batch export".to_string());
    }
    let has_renderable_watermark = request
        .watermarks
        .iter()
        .any(|watermark| watermark.is_renderable())
        || request
            .watermark
            .as_ref()
            .is_some_and(|watermark| watermark.is_renderable());
    if !has_renderable_watermark {
        return Err("Watermark text is required before export".to_string());
    }

    CANCEL_BATCH_EXPORT.store(false, Ordering::SeqCst);
    let total = request.source_paths.len();
    let mut results = Vec::with_capacity(total);

    for (offset, source_path) in request.source_paths.iter().enumerate() {
        if CANCEL_BATCH_EXPORT.load(Ordering::SeqCst) {
            break;
        }
        let item_request = ExportSelectedRequest {
            source_path: source_path.clone(),
            watermark: request.watermark.clone(),
            watermarks: request.watermarks.clone(),
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

    let cancelled = CANCEL_BATCH_EXPORT.swap(false, Ordering::SeqCst);
    if cancelled && results.len() < total {
        for source_path in request.source_paths.iter().skip(results.len()) {
            results.push(crate::domain::export::BatchExportItemResult {
                source_path: source_path.clone(),
                output_path: None,
                width: None,
                height: None,
                success: false,
                error: Some("Export cancelled".to_string()),
            });
        }
    }
    let completed = results.iter().filter(|result| result.success).count();
    let failed = results.len().saturating_sub(completed);

    Ok(BatchExportResult {
        completed,
        failed,
        results,
    })
}
