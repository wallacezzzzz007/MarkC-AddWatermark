import type { WatermarkDraft } from "./watermark";
import type { OutputRules } from "./output";

export type ExportSelectedRequest = {
  sourcePath: string;
  watermark: WatermarkDraft;
  outputRules?: OutputRules;
  index?: number;
};

export type ExportResult = {
  sourcePath?: string;
  outputPath: string;
  width: number;
  height: number;
};

export type BatchExportRequest = {
  sourcePaths: string[];
  watermark: WatermarkDraft;
  outputRules: OutputRules;
};

export type BatchExportItemResult = {
  sourcePath: string;
  outputPath?: string;
  width?: number;
  height?: number;
  success: boolean;
  error?: string;
};

export type BatchExportResult = {
  completed: number;
  failed: number;
  results: BatchExportItemResult[];
};
