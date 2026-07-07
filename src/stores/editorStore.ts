import { useMemo, useState } from "react";
import type { WatermarkAnchor, WatermarkDraft, WatermarkLayer } from "../types/watermark";

const ANCHOR_POSITIONS: Record<WatermarkAnchor, Pick<WatermarkDraft, "x" | "y">> = {
  "top-left": { x: 0, y: 0 },
  "top-center": { x: 0.5, y: 0 },
  "top-right": { x: 1, y: 0 },
  "center-left": { x: 0, y: 0.5 },
  center: { x: 0.5, y: 0.5 },
  "center-right": { x: 1, y: 0.5 },
  "bottom-left": { x: 0, y: 1 },
  "bottom-center": { x: 0.5, y: 1 },
  "bottom-right": { x: 1, y: 1 },
  custom: { x: 0.5, y: 0.5 },
};

const DEFAULT_WATERMARK: WatermarkDraft = {
  text: "@test",
  fontFamily: "Arial",
  color: "#ffffff",
  opacity: 0.82,
  fontSizePx: 47,
  fontSizePercent: 3.2,
  rotationDegrees: 0,
  x: ANCHOR_POSITIONS["bottom-center"].x,
  y: ANCHOR_POSITIONS["bottom-center"].y,
  anchor: "bottom-center",
};

export type EditorStore = {
  watermark: WatermarkLayer;
  watermarks: WatermarkLayer[];
  selectedWatermarkId: string;
  addTextWatermark: () => void;
  duplicateSelectedWatermark: () => void;
  removeWatermark: (id: string) => void;
  selectWatermark: (id: string) => void;
  setLayerName: (id: string, name: string) => void;
  toggleWatermarkVisibility: (id: string) => void;
  setText: (text: string) => void;
  setFontFamily: (fontFamily: string) => void;
  setColor: (color: string) => void;
  setOpacity: (opacity: number) => void;
  setFontSizePx: (fontSizePx: number, imageHeight?: number) => void;
  setFontSizePercent: (fontSizePercent: number) => void;
  setRotationDegrees: (rotationDegrees: number) => void;
  setPosition: (x: number, y: number) => void;
  setWatermarkPosition: (id: string, x: number, y: number) => void;
  setAnchor: (anchor: WatermarkAnchor) => void;
  applyWatermarks: (watermarks: WatermarkLayer[]) => void;
  applyWatermark: (watermark: WatermarkDraft) => void;
  resetWatermark: () => void;
};

export function useEditorStore(): EditorStore {
  const [watermarks, setWatermarks] = useState<WatermarkLayer[]>(() => [
    createWatermarkLayer(DEFAULT_WATERMARK, 1),
  ]);
  const [selectedWatermarkId, setSelectedWatermarkId] = useState(() => watermarks[0].id);
  const watermark = useMemo(
    () => watermarks.find((item) => item.id === selectedWatermarkId) ?? watermarks[0],
    [selectedWatermarkId, watermarks],
  );

  function updateSelected(update: (current: WatermarkLayer) => WatermarkLayer) {
    setWatermarks((current) =>
      current.map((item) => (item.id === selectedWatermarkId ? update(item) : item)),
    );
  }

  function addTextWatermark() {
    const layer = createWatermarkLayer(
      {
        ...DEFAULT_WATERMARK,
        text: `@test${watermarks.length + 1}`,
        x: 0.5,
        y: 0.5,
        anchor: "center",
      },
      watermarks.length + 1,
    );
    setWatermarks((current) => [...current, layer]);
    setSelectedWatermarkId(layer.id);
  }

  function duplicateSelectedWatermark() {
    const source = watermark;
    const layer = {
      ...source,
      id: crypto.randomUUID(),
      name: uniqueLayerName(`${source.name} copy`, watermarks.map((item) => item.name)),
      x: source.x + 0.03,
      y: source.y + 0.03,
    };
    setWatermarks((current) => [...current, layer]);
    setSelectedWatermarkId(layer.id);
  }

  function removeWatermark(id: string) {
    if (watermarks.length <= 1) return;
    const next = watermarks.filter((item) => item.id !== id);
    setWatermarks(next);
    if (id === selectedWatermarkId) {
      setSelectedWatermarkId(next[0].id);
    }
  }

  function applyWatermarks(nextWatermarks: WatermarkLayer[]) {
    const normalized = normalizeLayers(nextWatermarks);
    setWatermarks(normalized);
    setSelectedWatermarkId(normalized[0].id);
  }

  return {
    watermark,
    watermarks,
    selectedWatermarkId,
    addTextWatermark,
    duplicateSelectedWatermark,
    removeWatermark,
    selectWatermark: setSelectedWatermarkId,
    setLayerName: (id, name) =>
      setWatermarks((current) =>
        current.map((item) =>
          item.id === id ? { ...item, name: name.trim() || item.name } : item,
        ),
      ),
    toggleWatermarkVisibility: (id) =>
      setWatermarks((current) =>
        current.map((item) =>
          item.id === id ? { ...item, visible: !item.visible } : item,
        ),
      ),
    setText: (text) => updateSelected((current) => ({ ...current, text })),
    setFontFamily: (fontFamily) =>
      updateSelected((current) => ({ ...current, fontFamily })),
    setColor: (color) => updateSelected((current) => ({ ...current, color })),
    setOpacity: (opacity) =>
      updateSelected((current) => ({ ...current, opacity: clamp(opacity, 0, 1) })),
    setFontSizePx: (fontSizePx, imageHeight) =>
      updateSelected((current) => {
        const nextPx = Math.round(clamp(fontSizePx, 8, 512));
        return {
          ...current,
          fontSizePx: nextPx,
          fontSizePercent:
            imageHeight && imageHeight > 0
              ? clamp((nextPx / imageHeight) * 100, 0.1, 50)
              : current.fontSizePercent,
        };
      }),
    setFontSizePercent: (fontSizePercent) =>
      updateSelected((current) => ({
        ...current,
        fontSizePercent: clamp(fontSizePercent, 1, 10),
      })),
    setRotationDegrees: (rotationDegrees) =>
      updateSelected((current) => ({
        ...current,
        rotationDegrees: clamp(rotationDegrees, -180, 180),
      })),
    setPosition: (x, y) =>
      updateSelected((current) => ({
        ...current,
        x: finitePosition(x, current.x),
        y: finitePosition(y, current.y),
        anchor: "custom",
      })),
    setWatermarkPosition: (id, x, y) =>
      setWatermarks((current) =>
        current.map((item) =>
          item.id === id
            ? {
                ...item,
                x: finitePosition(x, item.x),
                y: finitePosition(y, item.y),
                anchor: "custom",
              }
            : item,
        ),
      ),
    setAnchor: (anchor) =>
      updateSelected((current) => ({
        ...current,
        ...ANCHOR_POSITIONS[anchor],
        anchor,
      })),
    applyWatermarks,
    applyWatermark: (nextWatermark) => applyWatermarks([createWatermarkLayer(nextWatermark, 1)]),
    resetWatermark: () => applyWatermarks([createWatermarkLayer(DEFAULT_WATERMARK, 1)]),
  };
}

function createWatermarkLayer(
  watermark: WatermarkDraft,
  index: number,
  name = `Watermark ${index}`,
): WatermarkLayer {
  return {
    ...watermark,
    id: crypto.randomUUID(),
    name,
    visible: true,
  };
}

function normalizeLayers(layers: WatermarkLayer[]): WatermarkLayer[] {
  const normalized = layers.length > 0 ? layers : [createWatermarkLayer(DEFAULT_WATERMARK, 1)];
  return normalized.map((layer, index) => ({
    ...DEFAULT_WATERMARK,
    ...layer,
    id: layer.id || crypto.randomUUID(),
    name: layer.name?.trim() || `Watermark ${index + 1}`,
    visible: layer.visible ?? true,
  }));
}

function uniqueLayerName(name: string, existingNames: string[]): string {
  const existing = new Set(existingNames);
  if (!existing.has(name)) return name;

  let suffix = 1;
  while (existing.has(`${name}-${suffix}`)) {
    suffix += 1;
  }

  return `${name}-${suffix}`;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}

function finitePosition(value: number, fallback: number): number {
  return Number.isFinite(value) ? value : fallback;
}
