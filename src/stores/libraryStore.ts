import { useMemo, useState } from "react";
import type { ImageAsset, ImportResult, SkippedImport } from "../types/image";

export type LibraryStore = {
  assets: ImageAsset[];
  skipped: SkippedImport[];
  selectedId: string | null;
  selectedAsset: ImageAsset | null;
  isImporting: boolean;
  importError: string | null;
  addImportResult: (result: ImportResult) => void;
  setSelectedId: (id: string | null) => void;
  removeAsset: (id: string) => void;
  clearLibrary: () => void;
  setImporting: (isImporting: boolean) => void;
  setImportError: (error: string | null) => void;
};

export function useLibraryStore(): LibraryStore {
  const [assets, setAssets] = useState<ImageAsset[]>([]);
  const [skipped, setSkipped] = useState<SkippedImport[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [isImporting, setImporting] = useState(false);
  const [importError, setImportError] = useState<string | null>(null);

  const selectedAsset = useMemo(
    () => assets.find((asset) => asset.id === selectedId) ?? assets[0] ?? null,
    [assets, selectedId],
  );

  function addImportResult(result: ImportResult) {
    setAssets((current) => {
      const byPath = new Map(current.map((asset) => [asset.path, asset]));
      for (const asset of result.accepted) {
        byPath.set(asset.path, asset);
      }
      const next = Array.from(byPath.values());
      setSelectedId((currentSelected) => currentSelected ?? next[0]?.id ?? null);
      return next;
    });

    setSkipped((current) => [...result.skipped, ...current].slice(0, 20));
  }

  function removeAsset(id: string) {
    setAssets((current) => {
      const next = current.filter((asset) => asset.id !== id);
      setSelectedId((currentSelected) => {
        if (currentSelected !== id) return currentSelected;
        return next[0]?.id ?? null;
      });
      return next;
    });
  }

  function clearLibrary() {
    setAssets([]);
    setSkipped([]);
    setSelectedId(null);
    setImportError(null);
  }

  return {
    assets,
    skipped,
    selectedId: selectedAsset?.id ?? null,
    selectedAsset,
    isImporting,
    importError,
    addImportResult,
    setSelectedId,
    removeAsset,
    clearLibrary,
    setImporting,
    setImportError,
  };
}

