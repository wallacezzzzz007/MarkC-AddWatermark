import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type PointerEvent as ReactPointerEvent,
} from "react";
import { formatBytes } from "../lib/format";
import { getPreviewDataUrl } from "../lib/ipc";
import type { EditorStore } from "../stores/editorStore";
import type { ImageAsset } from "../types/image";

type PreviewPaneProps = {
  asset: ImageAsset | null;
  editor: EditorStore;
  isImporting: boolean;
};

export function PreviewPane({ asset, editor, isImporting }: PreviewPaneProps) {
  const stageRef = useRef<HTMLDivElement | null>(null);
  const frameRef = useRef<HTMLDivElement | null>(null);
  const overlayRef = useRef<HTMLSpanElement | null>(null);
  const dragRafRef = useRef<number | null>(null);
  const zoomFocusRef = useRef(false);
  const pendingPositionRef = useRef<{ x: number; y: number } | null>(null);
  const activeGuidesRef = useRef({ horizontal: false, vertical: false });
  const [stageSize, setStageSize] = useState({ width: 0, height: 0 });
  const [previewSrc, setPreviewSrc] = useState<string | null>(asset?.previewSrc ?? null);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [activeGuides, setActiveGuides] = useState({ horizontal: false, vertical: false });
  const [manualZoomPercent, setManualZoomPercent] = useState<number | null>(null);
  const [showWatermark, setShowWatermark] = useState(true);
  const watermarkText = editor.watermark.text.trim();
  const fitPercent = asset
    ? fitZoomPercent(asset.width, asset.height, stageSize.width, stageSize.height)
    : 100;
  const zoomPercent = manualZoomPercent ?? fitPercent;
  const frameSize = asset
    ? zoomSize(asset.width, asset.height, zoomPercent)
    : { width: 0, height: 0 };
  const previewScale = asset && asset.height > 0 ? frameSize.height / asset.height : 1;
  const canPan = frameSize.width > stageSize.width || frameSize.height > stageSize.height;

  useEffect(() => {
    let isCurrent = true;
    setPreviewError(null);

    if (!asset) {
      setPreviewSrc(null);
      return () => {
        isCurrent = false;
      };
    }

    if (asset.previewSrc) {
      setPreviewSrc(asset.previewSrc);
      return () => {
        isCurrent = false;
      };
    }

    setPreviewSrc(null);
    getPreviewDataUrl(asset.path)
      .then((dataUrl) => {
        if (isCurrent) setPreviewSrc(dataUrl);
      })
      .catch((error) => {
        if (!isCurrent) return;
        setPreviewError(error instanceof Error ? error.message : String(error));
      });

    return () => {
      isCurrent = false;
    };
  }, [asset?.id, asset?.path, asset?.previewSrc]);

  useEffect(() => {
    setManualZoomPercent(null);
  }, [asset?.id]);

  useLayoutEffect(() => {
    if (!zoomFocusRef.current) return;
    zoomFocusRef.current = false;
    centerStageOnWatermark(stageRef.current, frameRef.current, editor.watermark.x, editor.watermark.y);
  }, [frameSize.height, frameSize.width, editor.watermark.x, editor.watermark.y]);

  useEffect(() => {
    return () => {
      if (dragRafRef.current !== null) {
        window.cancelAnimationFrame(dragRafRef.current);
      }
    };
  }, []);

  useEffect(() => {
    const activeStage = stageRef.current;
    if (!activeStage) return;

    const observer = new ResizeObserver(([entry]) => {
      setStageSize({
        width: entry.contentRect.width,
        height: entry.contentRect.height,
      });
    });

    observer.observe(activeStage);
    const rect = activeStage.getBoundingClientRect();
    setStageSize({ width: rect.width, height: rect.height });

    return () => observer.disconnect();
  }, []);

  function handlePointerDown(event: ReactPointerEvent<HTMLButtonElement>) {
    const activeFrame = frameRef.current;
    if (!activeFrame) return;
    const frameElement = activeFrame;

    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);

    function updatePosition(clientX: number, clientY: number) {
      const rect = frameElement.getBoundingClientRect();
      const position = snapToCenter(
        (clientX - rect.left) / rect.width,
        (clientY - rect.top) / rect.height,
      );
      const rawX = position.x;
      const rawY = position.y;
      const nextGuides = { horizontal: position.snapY, vertical: position.snapX };
      if (
        activeGuidesRef.current.horizontal !== nextGuides.horizontal ||
        activeGuidesRef.current.vertical !== nextGuides.vertical
      ) {
        activeGuidesRef.current = nextGuides;
        setActiveGuides(nextGuides);
      }
      pendingPositionRef.current = {
        x: rawX,
        y: rawY,
      };

      if (overlayRef.current) {
        overlayRef.current.style.left = `${rawX * 100}%`;
        overlayRef.current.style.top = `${rawY * 100}%`;
        overlayRef.current.style.transform = overlayTransform(
          editor.watermark.anchor,
          editor.watermark.rotationDegrees,
        );
      }

      if (dragRafRef.current !== null) return;
      dragRafRef.current = window.requestAnimationFrame(() => {
        dragRafRef.current = null;
      });
    }

    updatePosition(event.clientX, event.clientY);

    function handlePointerMove(moveEvent: PointerEvent) {
      updatePosition(moveEvent.clientX, moveEvent.clientY);
    }

    function handlePointerUp() {
      const position = pendingPositionRef.current;
      if (position) {
        editor.setPosition(position.x, position.y);
      }
      pendingPositionRef.current = null;
      activeGuidesRef.current = { horizontal: false, vertical: false };
      setActiveGuides(activeGuidesRef.current);
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerUp);
    }

    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp, { once: true });
  }

  function handleFitZoom() {
    zoomFocusRef.current = false;
    setManualZoomPercent(null);
  }

  function handleZoomChange(value: number) {
    zoomFocusRef.current = true;
    setManualZoomPercent(value);
  }

  return (
    <main className="preview-pane" aria-label="Image preview">
      <div className="preview-toolbar">
        <div>
          <strong>Preview</strong>
          <span>{asset ? asset.filename : "No image selected"}</span>
        </div>
        <div className="preview-actions">
          <button
            type="button"
            className={manualZoomPercent === null ? "active" : ""}
            onClick={handleFitZoom}
            disabled={!asset}
          >
            Fit
          </button>
          <label className="zoom-control">
            <span>Zoom</span>
            <input
              aria-label="Preview zoom"
              disabled={!asset}
              max="200"
              min="10"
              step="1"
              type="range"
              value={Math.round(zoomPercent)}
              onChange={(event) => handleZoomChange(Number(event.currentTarget.value))}
            />
            <strong>{Math.round(zoomPercent)}%</strong>
          </label>
          <button
            type="button"
            className={!showWatermark ? "active" : ""}
            onClick={() => setShowWatermark((current) => !current)}
            disabled={!asset || !watermarkText}
          >
            Before
          </button>
        </div>
      </div>

      <div className={`preview-stage ${canPan ? "pan-enabled" : ""}`} ref={stageRef}>
        {asset && previewSrc ? (
          <div
            className="image-frame"
            ref={frameRef}
            style={{
              height: `${frameSize.height}px`,
              width: `${frameSize.width}px`,
            }}
          >
            <img
              className="preview-image"
              src={previewSrc}
              alt={`Preview of ${asset.filename}`}
            />
            {watermarkText && showWatermark && (
              <button
                className="watermark-drag-surface"
                type="button"
                onPointerDown={handlePointerDown}
                title="Click or drag to place watermark"
              />
            )}
            {activeGuides.vertical && showWatermark && <span className="alignment-guide vertical" />}
            {activeGuides.horizontal && showWatermark && <span className="alignment-guide horizontal" />}
            {watermarkText && showWatermark && (
              <span
                className="watermark-live-overlay"
                ref={overlayRef}
                style={{
                  color: editor.watermark.color,
                  fontFamily: fontStack(editor.watermark.fontFamily),
                  fontSize: `${Math.max(
                    12,
                    editor.watermark.fontSizePx * previewScale,
                  )}px`,
                  left: `${editor.watermark.x * 100}%`,
                  opacity: editor.watermark.opacity,
                  top: `${editor.watermark.y * 100}%`,
                  transform: overlayTransform(
                    editor.watermark.anchor,
                    editor.watermark.rotationDegrees,
                  ),
                  transformOrigin: "center",
                }}
              >
                {watermarkText}
              </span>
            )}
          </div>
        ) : asset ? (
          <div className="empty-preview">
            <strong>{previewError ? "Preview unavailable" : "Loading preview..."}</strong>
            <span>
              {previewError ??
                "The original file is still ready for export while the preview loads."}
            </span>
          </div>
        ) : (
          <div className="empty-preview">
            <strong>{isImporting ? "Reading images..." : "Import images to begin"}</strong>
            <span>Use the left pane to add a file, several files, or one folder.</span>
          </div>
        )}
      </div>

      <div className="preview-status">
        {asset ? (
          <>
            <span>{asset.width} x {asset.height}</span>
            <span>{asset.extension.toUpperCase()}</span>
            <span>{formatBytes(asset.byteSize)}</span>
            <span>Source read-only</span>
          </>
        ) : (
          <span>Waiting for import</span>
        )}
      </div>
    </main>
  );
}

function snapToCenter(
  x: number,
  y: number,
): { x: number; y: number; snapX: boolean; snapY: boolean } {
  const threshold = 0.015;
  const snapX = Math.abs(x - 0.5) <= threshold;
  const snapY = Math.abs(y - 0.5) <= threshold;

  return {
    x: snapX ? 0.5 : x,
    y: snapY ? 0.5 : y,
    snapX,
    snapY,
  };
}

function anchorTransform(anchor: string): string {
  switch (anchor) {
    case "bottom-left":
      return "translate(0, -100%)";
    case "bottom-right":
      return "translate(-100%, -100%)";
    case "bottom-center":
      return "translate(-50%, -100%)";
    case "top-left":
      return "translate(0, 0)";
    case "top-right":
      return "translate(-100%, 0)";
    case "top-center":
      return "translate(-50%, 0)";
    case "center-left":
      return "translate(0, -50%)";
    case "center-right":
      return "translate(-100%, -50%)";
    default:
      return "translate(-50%, -50%)";
  }
}

function overlayTransform(anchor: string, rotationDegrees: number): string {
  return `translate3d(0, 0, 0) ${anchorTransform(anchor)} rotate(${rotationDegrees}deg)`;
}

function fontStack(fontFamily: string): string {
  return `${JSON.stringify(fontFamily)}, Arial, "Helvetica Neue", Helvetica, sans-serif`;
}

function centerStageOnWatermark(
  stage: HTMLDivElement | null,
  frame: HTMLDivElement | null,
  x: number,
  y: number,
) {
  if (!stage || !frame) return;

  const targetX = frame.offsetLeft + x * frame.offsetWidth;
  const targetY = frame.offsetTop + y * frame.offsetHeight;
  stage.scrollTo({
    left: Math.max(0, targetX - stage.clientWidth / 2),
    top: Math.max(0, targetY - stage.clientHeight / 2),
  });
}

function fitZoomPercent(
  imageWidth: number,
  imageHeight: number,
  availableWidth: number,
  availableHeight: number,
): number {
  if (imageWidth <= 0 || imageHeight <= 0 || availableWidth <= 0 || availableHeight <= 0) {
    return 100;
  }

  const paddedWidth = Math.max(1, availableWidth - 48);
  const paddedHeight = Math.max(1, availableHeight - 48);
  const scale = Math.min(paddedWidth / imageWidth, paddedHeight / imageHeight, 1);

  return Math.max(10, Math.min(200, Math.floor(scale * 100)));
}

function zoomSize(
  imageWidth: number,
  imageHeight: number,
  zoomPercent: number,
): { width: number; height: number } {
  const scale = Math.max(0.1, zoomPercent / 100);

  return {
    width: Math.max(1, Math.floor(imageWidth * scale)),
    height: Math.max(1, Math.floor(imageHeight * scale)),
  };
}
