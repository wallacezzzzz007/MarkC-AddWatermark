import { useState } from "react";
import type { WatermarkAnchor, WatermarkDraft } from "../types/watermark";

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
  text: "@bntxx_",
  fontFamily: "Arial",
  color: "#ffffff",
  opacity: 0.82,
  fontSizePercent: 3.2,
  x: ANCHOR_POSITIONS["bottom-center"].x,
  y: ANCHOR_POSITIONS["bottom-center"].y,
  anchor: "bottom-center",
};

export type EditorStore = {
  watermark: WatermarkDraft;
  setText: (text: string) => void;
  setFontFamily: (fontFamily: string) => void;
  setColor: (color: string) => void;
  setOpacity: (opacity: number) => void;
  setFontSizePercent: (fontSizePercent: number) => void;
  setPosition: (x: number, y: number) => void;
  setAnchor: (anchor: WatermarkAnchor) => void;
  applyWatermark: (watermark: WatermarkDraft) => void;
  resetWatermark: () => void;
};

export function useEditorStore(): EditorStore {
  const [watermark, setWatermark] = useState<WatermarkDraft>(DEFAULT_WATERMARK);

  return {
    watermark,
    setText: (text) => setWatermark((current) => ({ ...current, text })),
    setFontFamily: (fontFamily) =>
      setWatermark((current) => ({ ...current, fontFamily })),
    setColor: (color) => setWatermark((current) => ({ ...current, color })),
    setOpacity: (opacity) =>
      setWatermark((current) => ({ ...current, opacity: clamp(opacity, 0, 1) })),
    setFontSizePercent: (fontSizePercent) =>
      setWatermark((current) => ({
        ...current,
        fontSizePercent: clamp(fontSizePercent, 1, 10),
      })),
    setPosition: (x, y) =>
      setWatermark((current) => ({
        ...current,
        x: finitePosition(x, current.x),
        y: finitePosition(y, current.y),
        anchor: "custom",
      })),
    setAnchor: (anchor) =>
      setWatermark((current) => ({
        ...current,
        ...ANCHOR_POSITIONS[anchor],
        anchor,
      })),
    applyWatermark: (watermark) => setWatermark(watermark),
    resetWatermark: () => setWatermark(DEFAULT_WATERMARK),
  };
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}

function finitePosition(value: number, fallback: number): number {
  return Number.isFinite(value) ? value : fallback;
}
