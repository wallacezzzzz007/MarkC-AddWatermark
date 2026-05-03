import { useEffect, useState } from "react";
import type { OutputRules } from "../types/output";
import type { WatermarkDraft } from "../types/watermark";

export type WatermarkTemplate = {
  id: string;
  name: string;
  watermark: WatermarkDraft;
  outputRules: OutputRules;
};

export type TemplateStore = {
  templates: WatermarkTemplate[];
  selectedTemplateId: string | null;
  saveTemplate: (
    name: string,
    watermark: WatermarkDraft,
    outputRules: OutputRules,
  ) => WatermarkTemplate;
  deleteTemplate: (id: string) => void;
  selectTemplate: (id: string | null) => void;
};

const STORAGE_KEY = "watermark.templates.v1";

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

  useEffect(() => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(templates));
  }, [templates]);

  function saveTemplate(
    name: string,
    watermark: WatermarkDraft,
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
      watermark,
    };
    setTemplates((current) => [template, ...current]);
    setSelectedTemplateId(template.id);
    return template;
  }

  function deleteTemplate(id: string) {
    setTemplates((current) => current.filter((template) => template.id !== id));
    setSelectedTemplateId((current) => (current === id ? null : current));
  }

  return { templates, selectedTemplateId, saveTemplate, deleteTemplate, selectTemplate: setSelectedTemplateId };
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
    .map((item) => {
      if (!item || typeof item !== "object") return null;
      const template = item as Record<string, unknown>;
      if (template.watermark && template.outputRules) {
        const next = template as WatermarkTemplate;
        return {
          ...next,
          watermark: {
            ...next.watermark,
            fontFamily: next.watermark.fontFamily ?? "Arial",
          },
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
        watermark: {
          text,
          fontFamily: migratedFontFamily,
          color,
          opacity,
          fontSizePercent,
          x,
          y,
          anchor: anchor as WatermarkDraft["anchor"],
        },
        outputRules: {
          outputFolder: null,
          namingRule: "name-watermark",
          customPrefix: text,
          metadataPolicy: "clear-description",
          description: "",
        },
      } satisfies WatermarkTemplate;
    })
    .filter((item): item is WatermarkTemplate => item !== null);
}
