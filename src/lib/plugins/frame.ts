// A plugin frame's address: plugin://<id>/<page>?td=<its context as base64 JSON>. The frame's
// window.thumbdeck.context is read from it (src-tauri/src/plugins/frame/api.js).
import type { FrameInfo } from "./types.ts";

export interface FrameContext {
  surface: "tab" | "panel" | "card" | "page" | "view";
  /** The tab/panel/page id from the manifest */
  id: string;
  project: { path: string; name: string; branch: string | null } | null;
  /** A page's data from openPage */
  data?: unknown;
  api: number;
  thumbdeck: string;
}

/** Text as base64url (UTF-8, no padding) */
export function base64url(text: string): string {
  let bin = "";
  for (const b of new TextEncoder().encode(text)) bin += String.fromCharCode(b);
  return btoa(bin).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

export function frameUrl(info: FrameInfo, c: FrameContext): string {
  const context = {
    plugin: { id: info.plugin, version: info.version, folder: info.folder, dataFolder: info.data_folder },
    surface: c.surface,
    id: c.id,
    project: c.project && { path: c.project.path, name: c.project.name, branch: c.project.branch },
    ...(c.data === undefined ? {} : { data: c.data }),
    api: c.api,
    thumbdeck: c.thumbdeck,
  };
  const page = info.page.split("/").map(encodeURIComponent).join("/");
  return `plugin://${info.plugin}/${page}?td=${base64url(JSON.stringify(context))}`;
}
