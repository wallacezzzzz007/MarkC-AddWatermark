export type WatermarkAnchor =
  | "top-left"
  | "top-center"
  | "top-right"
  | "center-left"
  | "center"
  | "center-right"
  | "bottom-left"
  | "bottom-center"
  | "bottom-right"
  | "custom";

export type WatermarkDraft = {
  text: string;
  fontFamily: string;
  color: string;
  opacity: number;
  fontSizePx: number;
  fontSizePercent: number;
  rotationDegrees: number;
  x: number;
  y: number;
  anchor: WatermarkAnchor;
};
