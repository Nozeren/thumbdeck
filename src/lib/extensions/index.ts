// The extensions this thumbdeck has: each one's tab and setup form, by id (the same ids as
// AVAILABLE in src-tauri/src/extensions/mod.rs). Adding an extension: a folder here with its two
// components, a module there with its setup and commands, and one line in each list.
import type { Component } from "svelte";
import type { ReviewRequest } from "../review/types.ts";
import type { Keymap } from "../keys/keys.ts";
import LogViewer from "./logs/LogViewer.svelte";
import LogsSetup from "./logs/SetupForm.svelte";

/** What every tab component is given */
export interface TabProps {
  /** The project's folder */
  path: string;
  /** The project's name (its tmux session is named after it) */
  project: string;
  /** The tab's setup (the extension's own shape) */
  setup: any;
  /** Has the keyboard (Esc gives it back) */
  active: boolean;
  /** A short message at the bottom of the window */
  say: (text: string, error?: boolean) => void;
  /** Asks for the keyboard (e.g. a click in the tab) */
  onActivate: () => void;
  /** Gives the keyboard back to thumbdeck */
  onRelease: () => void;
  /** Opens the setup form */
  onEditSetup: () => void;
  /** Opens the review page (a diff over the whole window) */
  openReview: (request: ReviewRequest) => void;
}

/** What every tab component offers the page */
export interface TabExports {
  /** A key while the tab has the keyboard; false when it isn't one of the tab's (Esc then gives the keyboard back) */
  handleKey(e: KeyboardEvent): boolean;
  /** Its keys right now (for ? help, and its name in the status bar) */
  keymap(): Keymap;
}

/** What every setup form is given */
export interface SetupProps {
  setup: any;
  /** A new tab (not saved yet): no Remove */
  isNew: boolean;
  /** The project's name */
  project: string;
  onSave: (setup: any) => void;
  onRemove: () => void;
  onCancel: () => void;
}

export interface ExtensionUi {
  tab: Component<TabProps, TabExports>;
  setup: Component<SetupProps>;
}

export const extensions: Record<string, ExtensionUi> = {
  logs: { tab: LogViewer, setup: LogsSetup },
};
