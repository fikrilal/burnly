import { openExternalUrl } from "./client";

export function openExternalLink(url: string): Promise<void> {
  return openExternalUrl(url).then(() => undefined);
}
