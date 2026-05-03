import { invoke } from "@tauri-apps/api/core";
import type {
  BatchExportRequest,
  BatchExportResult,
  ExportResult,
  ExportSelectedRequest,
} from "../types/export";
import type { ImageAsset, ImportResult } from "../types/image";
import type { WatermarkDraft } from "../types/watermark";

export function importFiles(paths: string[]): Promise<ImportResult> {
  return invoke<ImportResult>("import_files", { paths });
}

export function importFolder(
  path: string,
  recursive = false,
): Promise<ImportResult> {
  return invoke<ImportResult>("import_folder", { path, recursive });
}

export function getImageInfo(path: string): Promise<ImageAsset> {
  return invoke<ImageAsset>("get_image_info", { path });
}

export function getPreviewDataUrl(path: string): Promise<string> {
  return invoke<string>("get_preview_data_url", { path });
}

export function renderPreviewDataUrl(
  path: string,
  watermark: WatermarkDraft,
): Promise<string> {
  return invoke<string>("render_preview_data_url", { path, watermark });
}

export function exportSelectedImage(
  request: ExportSelectedRequest,
): Promise<ExportResult> {
  return invoke<ExportResult>("export_selected_image", { request });
}

export function exportBatch(request: BatchExportRequest): Promise<BatchExportResult> {
  return invoke<BatchExportResult>("export_batch", { request });
}
