import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useEffect, useState, type DragEvent } from "react";
import { basename, formatBytes, formatSkipReason } from "../lib/format";
import { importFiles, importFolder } from "../lib/ipc";
import type { LibraryStore } from "../stores/libraryStore";
import type { BatchExportItemResult } from "../types/export";
import type { ImageAsset, ImportResult } from "../types/image";

type ImportPaneProps = {
  exportResults: BatchExportItemResult[];
  library: LibraryStore;
};

const IMAGE_FILTERS = [
  {
    name: "Images",
    extensions: ["jpg", "jpeg", "png"],
  },
];

export function ImportPane({ exportResults, library }: ImportPaneProps) {
  const [isDragOver, setDragOver] = useState(false);
  const exportResultByPath = new Map(
    exportResults.map((result) => [result.sourcePath, result]),
  );

  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;

    let isMounted = true;
    let unlisten: (() => void) | undefined;

    try {
      getCurrentWebview()
        .onDragDropEvent((event) => {
          if (!isMounted) return;

          if (event.payload.type === "enter") {
            setDragOver(true);
            return;
          }

          if (event.payload.type === "leave") {
            setDragOver(false);
            return;
          }

          if (event.payload.type === "drop") {
            setDragOver(false);
            void importDroppedPaths(event.payload.paths);
          }
        })
        .then((cleanup) => {
          unlisten = cleanup;
        })
        .catch(() => {
          // Browser-only development does not expose Tauri webview drag events.
        });
    } catch {
      return;
    }

    return () => {
      isMounted = false;
      unlisten?.();
    };
  }, []);

  async function runImport(importer: () => Promise<ImportResult | null>) {
    library.setImporting(true);
    library.setImportError(null);

    try {
      const result = await importer();
      if (result) {
        library.addImportResult(result);
      }
    } catch (error) {
      library.setImportError(error instanceof Error ? error.message : String(error));
    } finally {
      library.setImporting(false);
    }
  }

  async function chooseFiles() {
    await runImport(async () => {
      const selection = await open({
        multiple: true,
        directory: false,
        filters: IMAGE_FILTERS,
      });

      if (!selection) return null;
      return importFiles(Array.isArray(selection) ? selection : [selection]);
    });
  }

  async function chooseFolder() {
    await runImport(async () => {
      const selection = await open({
        multiple: false,
        directory: true,
      });

      if (!selection || Array.isArray(selection)) return null;
      return importFolder(selection, false);
    });
  }

  async function importDroppedPaths(paths: string[]) {
    if (paths.length === 0) return;
    await runImport(() => importFiles(paths));
  }

  async function handleBrowserDrop(event: DragEvent<HTMLElement>) {
    event.preventDefault();
    setDragOver(false);

    const paths = Array.from(event.dataTransfer.files)
      .map((file) => (file as File & { path?: string }).path)
      .filter((path): path is string => Boolean(path));

    if (paths.length > 0) {
      await importDroppedPaths(paths);
    }
  }

  return (
    <aside
      className={`pane import-pane ${isDragOver ? "drag-over" : ""}`}
      aria-label="Import queue"
      onDragEnter={(event) => {
        event.preventDefault();
        setDragOver(true);
      }}
      onDragOver={(event) => {
        event.preventDefault();
        setDragOver(true);
      }}
      onDragLeave={() => setDragOver(false)}
      onDrop={(event) => void handleBrowserDrop(event)}
    >
      <div className="pane-header">
        <div>
          <h1>MarkC</h1>
          <p>{library.assets.length} ready</p>
        </div>
        <button
          className="icon-button"
          type="button"
          onClick={library.clearLibrary}
          disabled={library.assets.length === 0 && library.skipped.length === 0}
          title="Clear queue"
        >
          Clear
        </button>
      </div>

      <div className="import-actions">
        <button type="button" onClick={chooseFiles} disabled={library.isImporting}>
          Import Images
        </button>
        <button type="button" onClick={chooseFolder} disabled={library.isImporting}>
          Import Folder
        </button>
      </div>

      {library.importError && (
        <div className="notice error" role="alert">
          {library.importError}
        </div>
      )}

      {library.assets.length === 0 ? (
        <button
          className="drop-zone"
          type="button"
          onClick={chooseFiles}
          disabled={library.isImporting}
        >
          <span>
            {library.isImporting
              ? "Reading images..."
              : isDragOver
                ? "Drop images to import"
                : "Choose or drop images"}
          </span>
          <small>Drop JPG, JPEG, PNG files, or folders here. Source files stay in place.</small>
        </button>
      ) : (
        <div className="asset-list">
          {library.assets.map((asset) => (
            <AssetRow
              asset={asset}
              exportResult={exportResultByPath.get(asset.path)}
              isSelected={library.selectedId === asset.id}
              key={asset.id}
              onSelect={() => library.setSelectedId(asset.id)}
              onRemove={() => library.removeAsset(asset.id)}
            />
          ))}
        </div>
      )}

      {library.skipped.length > 0 && (
        <section className="skipped-list" aria-label="Skipped imports">
          <div className="section-title">Skipped</div>
          {library.skipped.map((item, index) => (
            <div className="skipped-row" key={`${item.path}-${index}`}>
              <strong>{basename(item.path)}</strong>
              <span>{formatSkipReason(item.reason)}</span>
              <small>{item.message}</small>
            </div>
          ))}
        </section>
      )}
    </aside>
  );
}

function AssetRow({
  asset,
  exportResult,
  isSelected,
  onSelect,
  onRemove,
}: {
  asset: ImageAsset;
  exportResult?: BatchExportItemResult;
  isSelected: boolean;
  onSelect: () => void;
  onRemove: () => void;
}) {
  return (
    <div
      className={`asset-row ${isSelected ? "selected" : ""}`}
      onClick={onSelect}
    >
      <button className="asset-select" type="button">
        <span className="thumb">{asset.extension.toUpperCase()}</span>
        <span className="asset-copy">
          <strong>{asset.filename}</strong>
          <small>
            {asset.width} x {asset.height} · {formatBytes(asset.byteSize)}
          </small>
          {exportResult && (
            <small className={exportResult.success ? "exported" : "failed"}>
              {exportResult.success ? "Exported" : exportResult.error ?? "Failed"}
            </small>
          )}
        </span>
      </button>
      <span
        className={`status-dot ${
          exportResult ? (exportResult.success ? "exported" : "failed") : ""
        }`}
        aria-label={exportResult ? (exportResult.success ? "Exported" : "Failed") : "Ready"}
      />
      <button
        className="asset-remove"
        type="button"
        onClick={(event) => {
          event.stopPropagation();
          onRemove();
        }}
        title={`Remove ${asset.filename}`}
      >
        Remove
      </button>
    </div>
  );
}
