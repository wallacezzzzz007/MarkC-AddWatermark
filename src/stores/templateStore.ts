import { useEffect, useState } from "react";
import type { OutputRules } from "../types/output";
import type { WatermarkDraft, WatermarkLayer } from "../types/watermark";

export type WatermarkTemplate = {
  id: string;
  name: string;
  watermarks: WatermarkLayer[];
  outputRules: OutputRules;
};

export type TemplateStore = {
  templates: WatermarkTemplate[];
  selectedTemplateId: string | null;
  defaultTemplateId: string | null;
  saveTemplate: (
    name: string,
    watermarks: WatermarkLayer[],
    outputRules: OutputRules,
  ) => WatermarkTemplate;
  deleteTemplate: (id: string) => void;
  renameTemplate: (id: string, name: string) => void;
  setDefaultTemplate: (id: string | null) => void;
  selectTemplate: (id: string | null) => void;
};

const STORAGE_KEY = "watermark.templates.v1";
const DEFAULT_TEMPLATE_KEY = "watermark.defaultTemplateId.v1";

export function useTemplateStore(): TemplateStore {
  const [templates, setTemplates] = useState<WatermarkTemplate[]>(() => {
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      return raw ? migrateTemplates(JSON.parse(raw)) : [];
    } catch {
      return [];
    }
  });
  const [selectedTemplateId, setSelectedTemplateId] = useState<string | null>(null);
  const [defaultTemplateId, setDefaultTemplateId] = useState<string | null>(() =>
    localStorage.getItem(DEFAULT_TEMPLATE_KEY),
  );

  useEffect(() => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(templates));
  }, [templates]);

  useEffect(() => {
    if (defaultTemplateId && !templates.some((template) => template.id === defaultTemplateId)) {
      setDefaultTemplateId(null);
      return;
    }
    if (defaultTemplateId) {
      localStorage.setItem(DEFAULT_TEMPLATE_KEY, defaultTemplateId);
    } else {
      localStorage.removeItem(DEFAULT_TEMPLATE_KEY);
    }
  }, [defaultTemplateId, templates]);

  function saveTemplate(
    name: string,
    watermarks: WatermarkLayer[],
    outputRules: OutputRules,
  ): WatermarkTemplate {
    const templateName = uniqueTemplateName(
      name.trim() || "Untitled template",
      templates.map((template) => template.name),
    );
    const template = {
      id: crypto.randomUUID(),
      name: templateName,
      outputRules,
      watermarks: normalizeLayers(watermarks),
    };
    setTemplates((current) => [template, ...current]);
    setSelectedTemplateId(template.id);
    return template;
  }

  function deleteTemplate(id: string) {
    setTemplates((current) => current.filter((template) => template.id !== id));
    setSelectedTemplateId((current) => (current === id ? null : current));
    setDefaultTemplateId((current) => (current === id ? null : current));
  }

  function renameTemplate(id: string, name: string) {
    const trimmed = name.trim();
    if (!trimmed) return;
    setTemplates((current) => {
      const existingNames = current
        .filter((template) => template.id !== id)
        .map((template) => template.name);
      const nextName = uniqueTemplateName(trimmed, existingNames);
      return current.map((template) =>
        template.id === id ? { ...template, name: nextName } : template,
      );
    });
  }

  return {
    templates,
    selectedTemplateId,
    defaultTemplateId,
    saveTemplate,
    deleteTemplate,
    renameTemplate,
    setDefaultTemplate: setDefaultTemplateId,
    selectTemplate: setSelectedTemplateId,
  };
}

function uniqueTemplateName(name: string, existingNames: string[]): string {
  const existing = new Set(existingNames);
  if (!existing.has(name)) return name;

  let suffix = 1;
  while (existing.has(`${name}-${suffix}`)) {
    suffix += 1;
  }

  return `${name}-${suffix}`;
}

function migrateTemplates(value: unknown): WatermarkTemplate[] {
  if (!Array.isArray(value)) return [];

  return value
    .map((item): WatermarkTemplate | null => {
      if (!item || typeof item !== "object") return null;
      const template = item as Record<string, unknown>;
      if ((template.watermark || template.watermarks) && template.outputRules) {
        const next = template as WatermarkTemplate & { watermark?: WatermarkDraft };
        const legacyWatermarks =
          Array.isArray(template.watermarks) && template.watermarks.length > 0
            ? normalizeLayers(template.watermarks as WatermarkLayer[])
            : normalizeLayers([
                {
                  ...next.watermark,
                  id: crypto.randomUUID(),
                  name: "Watermark 1",
                  visible: true,
                } as WatermarkLayer,
              ]);
        return {
          id: next.id,
          name: next.name,
          outputRules: next.outputRules,
          watermarks: legacyWatermarks,
        };
      }

      const {
        id,
        name,
        text,
        fontFamily,
        color,
        opacity,
        fontSizePercent,
        x,
        y,
        anchor,
      } = template;
      if (typeof id !== "string" || typeof name !== "string") return null;
      if (typeof text !== "string" || typeof color !== "string") return null;
      if (typeof opacity !== "number" || typeof fontSizePercent !== "number") return null;
      if (typeof x !== "number" || typeof y !== "number" || typeof anchor !== "string") {
        return null;
      }

      const migratedFontFamily = typeof fontFamily === "string" ? fontFamily : "Arial";

      return {
        id,
        name,
        watermarks: normalizeLayers([
          {
            text,
            fontFamily: migratedFontFamily,
            color,
            opacity,
            fontSizePx: Math.round(fontSizePercent * 14.56),
            fontSizePercent,
            rotationDegrees: 0,
            x,
            y,
            anchor: anchor as WatermarkDraft["anchor"],
            id: crypto.randomUUID(),
            name: "Watermark 1",
            visible: true,
          },
        ]),
        outputRules: {
          outputFolder: null,
          namingRule: "name-watermark",
          customPrefix: text,
          metadataPolicy: "clear-description",
          description: "",
        } satisfies OutputRules,
      };
    })
    .filter((item): item is WatermarkTemplate => item !== null);
}

function normalizeLayers(layers: WatermarkLayer[]): WatermarkLayer[] {
  const normalized = layers.length > 0 ? layers : [];

  return normalized.map((layer, index) => ({
    text: layer.text ?? "@test",
    fontFamily: layer.fontFamily ?? "Arial",
    color: layer.color ?? "#ffffff",
    opacity: typeof layer.opacity === "number" ? layer.opacity : 0.82,
    fontSizePx:
      typeof layer.fontSizePx === "number"
        ? layer.fontSizePx
        : Math.round((layer.fontSizePercent ?? 3.2) * 14.56),
    fontSizePercent:
      typeof layer.fontSizePercent === "number" ? layer.fontSizePercent : 3.2,
    rotationDegrees:
      typeof layer.rotationDegrees === "number" ? layer.rotationDegrees : 0,
    x: typeof layer.x === "number" ? layer.x : 0.5,
    y: typeof layer.y === "number" ? layer.y : 0.5,
    anchor: layer.anchor ?? "center",
    id: layer.id || crypto.randomUUID(),
    name: layer.name?.trim() || `Watermark ${index + 1}`,
    visible: layer.visible ?? true,
  }));
}
