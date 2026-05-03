export type NamingRule =
  | "name-watermark"
  | "watermark-name"
  | "watermark-index"
  | "custom-prefix-index";

export type MetadataPolicy = "clear-description" | "rewrite-description" | "preserve";

export type OutputRules = {
  outputFolder?: string | null;
  namingRule: NamingRule;
  customPrefix: string;
  metadataPolicy: MetadataPolicy;
  description: string;
};
