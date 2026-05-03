import { useMemo, useState } from "react";
import type { BatchExportItemResult } from "../types/export";
import type { NamingRule, OutputRules } from "../types/output";

export type ExportStore = {
  outputRules: OutputRules;
  isExporting: boolean;
  results: BatchExportItemResult[];
  completed: number;
  failed: number;
  statusMessage: string;
  setOutputFolder: (outputFolder: string | null) => void;
  setNamingRule: (namingRule: NamingRule) => void;
  setCustomPrefix: (customPrefix: string) => void;
  setMetadataPolicy: (metadataPolicy: OutputRules["metadataPolicy"]) => void;
  setDescription: (description: string) => void;
  applyOutputRules: (outputRules: OutputRules) => void;
  startExport: (total: number) => void;
  finishExport: (results: BatchExportItemResult[]) => void;
  failExport: (message: string) => void;
  recordProgressResult: (result: BatchExportItemResult) => void;
  clearResults: () => void;
};

const DEFAULT_OUTPUT_RULES: OutputRules = {
  outputFolder: null,
  namingRule: "name-watermark",
  customPrefix: "bntxx",
  metadataPolicy: "clear-description",
  description: "",
};

export function useExportStore(): ExportStore {
  const [outputRules, setOutputRules] = useState<OutputRules>(DEFAULT_OUTPUT_RULES);
  const [isExporting, setExporting] = useState(false);
  const [results, setResults] = useState<BatchExportItemResult[]>([]);
  const [statusMessage, setStatusMessage] = useState("Ready to export imported images");

  const completed = useMemo(
    () => results.filter((result) => result.success).length,
    [results],
  );
  const failed = Math.max(0, results.length - completed);

  return {
    outputRules,
    isExporting,
    results,
    completed,
    failed,
    statusMessage,
    setOutputFolder: (outputFolder) =>
      setOutputRules((current) => ({ ...current, outputFolder })),
    setNamingRule: (namingRule) =>
      setOutputRules((current) => ({ ...current, namingRule })),
    setCustomPrefix: (customPrefix) =>
      setOutputRules((current) => ({ ...current, customPrefix })),
    setMetadataPolicy: (metadataPolicy) =>
      setOutputRules((current) => ({ ...current, metadataPolicy })),
    setDescription: (description) =>
      setOutputRules((current) => ({ ...current, description })),
    applyOutputRules: (outputRules) => setOutputRules(outputRules),
    startExport: (total) => {
      setExporting(true);
      setResults([]);
      setStatusMessage(`Exporting 0 of ${total}`);
    },
    finishExport: (nextResults) => {
      const nextCompleted = nextResults.filter((result) => result.success).length;
      const nextFailed = nextResults.length - nextCompleted;
      setExporting(false);
      setResults(nextResults);
      setStatusMessage(`Exported ${nextCompleted} images, ${nextFailed} failed`);
    },
    failExport: (message) => {
      setExporting(false);
      setStatusMessage(message);
    },
    recordProgressResult: (result) => {
      setResults((current) => {
        const withoutCurrent = current.filter(
          (item) => item.sourcePath !== result.sourcePath,
        );
        const next = [...withoutCurrent, result];
        const nextCompleted = next.filter((item) => item.success).length;
        setStatusMessage(`Exporting ${next.length}: ${nextCompleted} succeeded`);
        return next;
      });
    },
    clearResults: () => {
      setResults([]);
      setStatusMessage("Ready to export imported images");
    },
  };
}
