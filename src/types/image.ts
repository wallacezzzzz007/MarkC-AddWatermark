export type ImageStatus = "ready";

export type ImageAsset = {
  id: string;
  path: string;
  previewSrc?: string;
  filename: string;
  extension: "jpg" | "jpeg" | "png";
  width: number;
  height: number;
  byteSize: number;
  status: ImageStatus;
};

export type ImportSkipReason =
  | "unsupported_format"
  | "corrupt_image"
  | "not_file"
  | "not_directory"
  | "read_error";

export type SkippedImport = {
  path: string;
  reason: ImportSkipReason;
  message: string;
};

export type ImportResult = {
  accepted: ImageAsset[];
  skipped: SkippedImport[];
};
