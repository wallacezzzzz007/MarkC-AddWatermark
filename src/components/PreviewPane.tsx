import { useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
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
  const pendingPositionRef = useRef<{ x: number; y: number } | null>(null);
  const [stageSize, setStageSize] = useState({ width: 0, height: 0 });
  const [previewSrc, setPreviewSrc] = useState<string | null>(asset?.previewSrc ?? null);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const watermarkText = editor.watermark.text.trim();
  const frameSize = asset
    ? fitSize(asset.width, asset.height, stageSize.width, stageSize.height)
    : { width: 0, height: 0 };

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
      const rawX = (clientX - rect.left) / rect.width;
      const rawY = (clientY - rect.top) / rect.height;
      pendingPositionRef.current = {
        x: rawX,
        y: rawY,
      };

      if (overlayRef.current) {
        overlayRef.current.style.left = `${rawX * 100}%`;
        overlayRef.current.style.top = `${rawY * 100}%`;
      }

      if (dragRafRef.current !== null) return;
      dragRafRef.current = window.requestAnimationFrame(() => {
        dragRafRef.current = null;
        const position = pendingPositionRef.current;
        if (position) {
          editor.setPosition(position.x, position.y);
        }
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
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerUp);
    }

    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp, { once: true });
  }

  return (
    <main className="preview-pane" aria-label="Image preview">
      <div className="preview-toolbar">
        <div>
          <strong>Preview</strong>
          <span>{asset ? asset.filename : "No image selected"}</span>
        </div>
      </div>

      <div className="preview-stage" ref={stageRef}>
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
            {watermarkText && (
              <button
                className="watermark-drag-surface"
                type="button"
                onPointerDown={handlePointerDown}
                title="Click or drag to place watermark"
              />
            )}
            {watermarkText && (
              <span
                className="watermark-live-overlay"
                ref={overlayRef}
                style={{
                  color: editor.watermark.color,
                  fontFamily: fontStack(editor.watermark.fontFamily),
                  fontSize: `${Math.max(
                    12,
                    frameSize.height * (editor.watermark.fontSizePercent / 100),
                  )}px`,
                  left: `${editor.watermark.x * 100}%`,
                  opacity: editor.watermark.opacity,
                  top: `${editor.watermark.y * 100}%`,
                  transform: anchorTransform(editor.watermark.anchor),
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

function fontStack(fontFamily: string): string {
  return `${JSON.stringify(fontFamily)}, Arial, "Helvetica Neue", Helvetica, sans-serif`;
}

function fitSize(
  imageWidth: number,
  imageHeight: number,
  availableWidth: number,
  availableHeight: number,
): { width: number; height: number } {
  if (imageWidth <= 0 || imageHeight <= 0 || availableWidth <= 0 || availableHeight <= 0) {
    return { width: 0, height: 0 };
  }

  const scale = Math.min(availableWidth / imageWidth, availableHeight / imageHeight, 1);

  return {
    width: Math.max(1, Math.floor(imageWidth * scale)),
    height: Math.max(1, Math.floor(imageHeight * scale)),
  };
}
