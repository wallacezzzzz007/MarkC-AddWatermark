import { open } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { basename } from "../lib/format";
import { cancelBatchExport, exportBatch } from "../lib/ipc";
import type { EditorStore } from "../stores/editorStore";
import type { ExportStore } from "../stores/exportStore";
import type { LibraryStore } from "../stores/libraryStore";
import type { TemplateStore } from "../stores/templateStore";
import type { BatchExportItemResult } from "../types/export";
import type { ImageAsset } from "../types/image";
import type { MetadataPolicy, NamingRule } from "../types/output";
import type { WatermarkAnchor } from "../types/watermark";

type ControlPaneProps = {
  editor: EditorStore;
  exportStore: ExportStore;
  library: LibraryStore;
  templateStore: TemplateStore;
};

const ANCHORS: Array<{ id: WatermarkAnchor; label: string }> = [
  { id: "top-left", label: "Top left" },
  { id: "top-center", label: "Top center" },
  { id: "top-right", label: "Top right" },
  { id: "center-left", label: "Center left" },
  { id: "center", label: "Center" },
  { id: "center-right", label: "Center right" },
  { id: "bottom-left", label: "Bottom left" },
  { id: "bottom-center", label: "Bottom center" },
  { id: "bottom-right", label: "Bottom right" },
];

const NAMING_RULES: Array<{ id: NamingRule; label: string }> = [
  { id: "name-watermark", label: "{name}_watermark.{ext}" },
  { id: "watermark-name", label: "watermark_{name}.{ext}" },
  { id: "watermark-index", label: "watermark_{index}.{ext}" },
  { id: "custom-prefix-index", label: "Custom prefix + sequence" },
];

const METADATA_POLICIES: Array<{ id: MetadataPolicy; label: string }> = [
  { id: "clear-description", label: "Clear" },
  { id: "rewrite-description", label: "Rewrite" },
  { id: "preserve", label: "Preserve" },
];

const FONT_OPTIONS = [
  "Arial",
  "Snell Roundhand",
  "Brush Script MT",
  "Zapfino",
  "Georgia",
  "Times New Roman",
  "Baskerville",
  "Didot",
  "Hoefler Text",
  "Palatino",
  "Optima",
  "Futura",
  "Copperplate",
  "Courier New",
  "Verdana",
  "Trebuchet MS",
  "Comic Sans MS",
  "Noteworthy",
  "Marker Felt",
  "Chalkduster",
  "Papyrus",
];

export function ControlPane({
  editor,
  exportStore,
  library,
  templateStore,
}: ControlPaneProps) {
  const { watermark } = editor;
  const [templateName, setTemplateName] = useState("Default watermark");
  const [exportNotice, setExportNotice] = useState<string | null>(null);
  const realAssets = library.assets.filter((item) => !item.previewSrc);
  const asset = library.selectedAsset?.previewSrc
    ? realAssets[0] ?? library.selectedAsset
    : library.selectedAsset;
  const outputName = useMemo(() => {
    return previewOutputName(asset, exportStore.outputRules.namingRule, exportStore.outputRules.customPrefix);
  }, [asset, exportStore.outputRules.customPrefix, exportStore.outputRules.namingRule]);
  const trimmedText = watermark.text.trim();
  const imageHeight = asset?.height ?? 1456;
  const sizePercent =
    imageHeight > 0 ? (watermark.fontSizePx / imageHeight) * 100 : watermark.fontSizePercent;
  const minFontPx = 8;
  const maxFontPx = Math.max(24, Math.round(imageHeight * 0.16));
  const canExport =
    realAssets.length > 0 && trimmedText.length > 0 && !exportStore.isExporting;
  const changeSignature = JSON.stringify({
    assetId: asset?.id ?? null,
    assetCount: realAssets.length,
    outputRules: exportStore.outputRules,
    templateName,
    watermark,
  });

  useEffect(() => {
    if (exportStore.outputRules.customPrefix !== watermark.text) {
      exportStore.setCustomPrefix(watermark.text);
    }
  }, [exportStore, watermark.text]);

  useEffect(() => {
    setExportNotice(null);
  }, [changeSignature]);

  useEffect(() => {
    if (!isTauriRuntime()) return;

    let isMounted = true;
    let unlisten: (() => void) | undefined;

    listen<{ latest?: BatchExportItemResult }>("batch_export_progress", (event) => {
      if (!isMounted || !event.payload.latest) return;
      exportStore.recordProgressResult(event.payload.latest);
    })
      .then((cleanup) => {
        unlisten = cleanup;
      })
      .catch(() => {
        // The browser-only preview has no Tauri event bridge.
      });

    return () => {
      isMounted = false;
      unlisten?.();
    };
  }, [exportStore]);

  async function chooseOutputFolder() {
    const selection = await open({
      directory: true,
      multiple: false,
    });

    if (!selection || Array.isArray(selection)) return;
    exportStore.setOutputFolder(selection);
  }

  async function handleBatchExport() {
    if (!canExport) return;

    setExportNotice(null);
    exportStore.startExport(realAssets.length);

    try {
      const result = await exportBatch({
        outputRules: { ...exportStore.outputRules, customPrefix: watermark.text },
        sourcePaths: realAssets.map((item) => item.path),
        watermark,
      });
      exportStore.finishExport(result.results);
      setExportNotice(
        `Export complete: ${result.completed} succeeded, ${result.failed} failed.`,
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      exportStore.failExport(message);
      setExportNotice(`Export failed: ${message}`);
    }
  }

  function handleSelectTemplate(id: string) {
    templateStore.selectTemplate(id || null);
    const template = templateStore.templates.find((item) => item.id === id);
    if (template) {
      editor.applyWatermark(template.watermark);
      exportStore.applyOutputRules(template.outputRules);
      setTemplateName(template.name);
    }
  }

  function handleSaveTemplate() {
    const template = templateStore.saveTemplate(
      templateName,
      watermark,
      { ...exportStore.outputRules, customPrefix: watermark.text },
    );
    setTemplateName(template.name);
  }

  function handleDeleteTemplate() {
    if (!templateStore.selectedTemplateId) return;
    templateStore.deleteTemplate(templateStore.selectedTemplateId);
    templateStore.selectTemplate(null);
  }

  function handleRenameTemplate() {
    if (!templateStore.selectedTemplateId) return;
    templateStore.renameTemplate(templateStore.selectedTemplateId, templateName);
  }

  function handleSetDefaultTemplate() {
    templateStore.setDefaultTemplate(templateStore.selectedTemplateId);
  }

  async function handleCancelExport() {
    await cancelBatchExport();
    exportStore.markCancelling();
  }

  return (
    <aside className="pane control-pane" aria-label="Watermark controls">
      <div className="pane-header">
        <div>
          <h2>Controls</h2>
        </div>
      </div>

      <InspectorSection title="Template">
        <label>
          Saved templates
          <select
            value={templateStore.selectedTemplateId ?? ""}
            onChange={(event) => handleSelectTemplate(event.currentTarget.value)}
          >
            <option value="">Current unsaved watermark</option>
            {templateStore.templates.map((template) => (
              <option key={template.id} value={template.id}>
                {template.name}
              </option>
            ))}
          </select>
        </label>
        <label>
          Name
          <input
            value={templateName}
            onChange={(event) => setTemplateName(event.currentTarget.value)}
          />
        </label>
        <div className="template-actions">
          <button type="button" onClick={handleSaveTemplate}>
            Save
          </button>
          <button
            type="button"
            onClick={handleRenameTemplate}
            disabled={!templateStore.selectedTemplateId}
          >
            Rename
          </button>
          <button
            type="button"
            onClick={handleSetDefaultTemplate}
            disabled={!templateStore.selectedTemplateId}
          >
            Set default
          </button>
          <button
            type="button"
            onClick={handleDeleteTemplate}
            disabled={!templateStore.selectedTemplateId}
          >
            Delete
          </button>
        </div>
        <div className="collision-note">
          Default:{" "}
          {templateStore.templates.find((item) => item.id === templateStore.defaultTemplateId)
            ?.name ?? "None"}
        </div>
      </InspectorSection>

      <InspectorSection title="Watermark">
        <label>
          Text
          <input
            value={watermark.text}
            onChange={(event) => editor.setText(event.currentTarget.value)}
            placeholder="@test"
          />
        </label>
      </InspectorSection>

      <InspectorSection title="Style">
        <label>
          Font
          <select
            value={watermark.fontFamily}
            onChange={(event) => editor.setFontFamily(event.currentTarget.value)}
          >
            {FONT_OPTIONS.map((font) => (
              <option key={font} value={font}>
                {font}
              </option>
            ))}
          </select>
        </label>
        <label>
          Size
          <span
            className="range-row size-row"
            title={`${sizePercent.toFixed(1)}% of image height`}
          >
            <input
              max={maxFontPx}
              min={minFontPx}
              step="1"
              type="range"
              value={watermark.fontSizePx}
              onChange={(event) =>
                editor.setFontSizePx(Number(event.currentTarget.value), imageHeight)
              }
            />
            <span className="number-with-unit">
              <input
                aria-label="Watermark size in pixels"
                inputMode="numeric"
                max={maxFontPx}
                min={minFontPx}
                step="1"
                type="number"
                value={watermark.fontSizePx}
                onChange={(event) =>
                  editor.setFontSizePx(Number(event.currentTarget.value), imageHeight)
                }
              />
              <strong>px</strong>
            </span>
          </span>
        </label>
        <label>
          Opacity
          <span className="range-row">
            <input
              max="1"
              min="0"
              step="0.01"
              type="range"
              value={watermark.opacity}
              onChange={(event) => editor.setOpacity(Number(event.currentTarget.value))}
            />
            <strong>{Math.round(watermark.opacity * 100)}%</strong>
          </span>
        </label>
        <label>
          Rotation
          <span className="range-row size-row">
            <input
              max="180"
              min="-180"
              step="1"
              type="range"
              value={watermark.rotationDegrees}
              onChange={(event) =>
                editor.setRotationDegrees(Number(event.currentTarget.value))
              }
            />
            <span className="number-with-unit">
              <input
                aria-label="Watermark rotation in degrees"
                max="180"
                min="-180"
                step="1"
                type="number"
                value={watermark.rotationDegrees}
                onChange={(event) =>
                  editor.setRotationDegrees(Number(event.currentTarget.value))
                }
              />
              <strong>deg</strong>
            </span>
          </span>
        </label>
        <div className="rotation-presets">
          {[-45, 0, 45, 90].map((degrees) => (
            <button
              key={degrees}
              type="button"
              className={watermark.rotationDegrees === degrees ? "active" : ""}
              onClick={() => editor.setRotationDegrees(degrees)}
            >
              {degrees}°
            </button>
          ))}
        </div>
        <label>
          Color
          <span className="color-row">
            <input
              type="color"
              value={watermark.color}
              onChange={(event) => editor.setColor(event.currentTarget.value)}
            />
            <strong>{watermark.color.toUpperCase()}</strong>
          </span>
        </label>
      </InspectorSection>

      <InspectorSection title="Position">
        <div className="anchor-buttons">
          {ANCHORS.map((anchor) => (
            <button
              className={watermark.anchor === anchor.id ? "active" : ""}
              key={anchor.id}
              type="button"
              onClick={() => editor.setAnchor(anchor.id)}
            >
              {anchor.label}
            </button>
          ))}
        </div>
        {watermark.anchor === "custom" && <div className="custom-position-label">Custom</div>}
        <div className="coordinate-inputs">
          <label>
            X %
            <input
              type="number"
              step="0.1"
              value={(watermark.x * 100).toFixed(1)}
              onChange={(event) =>
                editor.setPosition(
                  Number(event.currentTarget.value) / 100,
                  watermark.y,
                )
              }
            />
          </label>
          <label>
            Y %
            <input
              type="number"
              step="0.1"
              value={(watermark.y * 100).toFixed(1)}
              onChange={(event) =>
                editor.setPosition(
                  watermark.x,
                  Number(event.currentTarget.value) / 100,
                )
              }
            />
          </label>
        </div>
      </InspectorSection>

      <InspectorSection title="Export">
        <label>
          Output folder
          <span className="folder-row">
            <input
              value={exportStore.outputRules.outputFolder ?? "Default: Watermark export beside each source"}
              readOnly
            />
            <button type="button" onClick={chooseOutputFolder} disabled={exportStore.isExporting}>
              Choose
            </button>
          </span>
        </label>
        <label>
          Naming
          <select
            value={exportStore.outputRules.namingRule}
            onChange={(event) =>
              exportStore.setNamingRule(event.currentTarget.value as NamingRule)
            }
            disabled={exportStore.isExporting}
          >
            {NAMING_RULES.map((rule) => (
              <option key={rule.id} value={rule.id}>
                {rule.label}
              </option>
            ))}
          </select>
        </label>
        {exportStore.outputRules.namingRule === "custom-prefix-index" && (
          <label>
            Prefix
            <input
              value={exportStore.outputRules.customPrefix}
              readOnly
            />
          </label>
        )}
        <label>
          Preview
          <input value={outputName} readOnly />
        </label>
        <div className="collision-note">Existing files get -2, -3, ...</div>
        <div className="progress-track">
          <span
            style={{
              width:
                exportStore.results.length > 0 || exportStore.isExporting
                  ? `${Math.min(100, (exportStore.results.length / Math.max(1, realAssets.length)) * 100)}%`
                  : "0%",
            }}
          />
        </div>
        <p className="export-status">{exportStore.statusMessage}</p>
      </InspectorSection>

      <InspectorSection title="Metadata">
        <div className="metadata-buttons">
          {METADATA_POLICIES.map((policy) => (
            <button
              className={exportStore.outputRules.metadataPolicy === policy.id ? "active" : ""}
              key={policy.id}
              type="button"
              onClick={() => exportStore.setMetadataPolicy(policy.id)}
            >
              {policy.label}
            </button>
          ))}
        </div>
        {exportStore.outputRules.metadataPolicy === "rewrite-description" && (
          <label>
            Description
            <textarea
              value={exportStore.outputRules.description}
              onChange={(event) => exportStore.setDescription(event.currentTarget.value)}
              placeholder="Custom image description"
            />
          </label>
        )}
        <div className="collision-note">
          {metadataCopy(exportStore.outputRules.metadataPolicy)}
        </div>
      </InspectorSection>

      <button
        className="export-button"
        type="button"
        disabled={!canExport}
        onClick={handleBatchExport}
      >
        Export
      </button>
      {exportNotice && (
        <div
          className={`export-toast ${
            exportNotice.startsWith("Export failed") ? "error" : "success"
          }`}
          role="status"
        >
          {exportNotice}
        </div>
      )}
      {exportStore.isExporting && (
        <div className="export-overlay" role="status" aria-live="polite">
          <div className="export-spinner" />
          <strong>Exporting images...</strong>
          <span>{exportStore.statusMessage}</span>
          <button type="button" onClick={handleCancelExport}>
            Cancel
          </button>
        </div>
      )}
    </aside>
  );
}

function isTauriRuntime(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

function metadataCopy(policy: MetadataPolicy): string {
  switch (policy) {
    case "rewrite-description":
      return "Writes a new description where supported.";
    case "preserve":
      return "Preserves metadata where possible.";
    default:
      return "Clears prompt-like description fields when possible.";
  }
}

function previewOutputName(
  asset: ImageAsset | null,
  namingRule: NamingRule,
  customPrefix: string,
): string {
  const extension = asset?.extension ?? "jpg";
  const filename = asset ? basename(asset.path) : "image.jpg";
  const dotIndex = filename.lastIndexOf(".");
  const stem = dotIndex <= 0 ? filename : filename.slice(0, dotIndex);
  const index = "001";
  const prefix = sanitizePrefix(customPrefix);

  switch (namingRule) {
    case "watermark-name":
      return `watermark_${stem}.${extension}`;
    case "watermark-index":
      return `watermark_${index}.${extension}`;
    case "custom-prefix-index":
      return `${prefix}_${index}.${extension}`;
    default:
      return `${stem}_watermark.${extension}`;
  }
}

function sanitizePrefix(prefix: string): string {
  const trimmed = prefix.trim();
  if (!trimmed) return "watermark";

  return Array.from(trimmed)
    .map((character) =>
      /[A-Za-z0-9_@-]/.test(character) ? character : "_",
    )
    .join("");
}

function InspectorSection({
  title,
  children,
}: {
  title: string;
  children: ReactNode;
}) {
  return (
    <section className="inspector-section">
      <div className="section-title">
        <span>{title}</span>
      </div>
      {children}
    </section>
  );
}
