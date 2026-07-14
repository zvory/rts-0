export const DEFAULT_PORT: number;
export const DEFAULT_TTL_MS: number;

export interface PublishedTailnetPreview {
  url: string;
  expiresAt: number | null;
}

export function publishTailnetPreview(options: {
  source: string;
  root?: string;
  host?: string;
  port?: number;
  ttlMs?: number;
  keep?: boolean;
}): Promise<PublishedTailnetPreview>;
