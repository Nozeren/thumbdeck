// Plugins as thumbdeck's backend describes them (src-tauri/src/plugins)

/** A settings or setup field, drawn by thumbdeck (FieldsForm) */
export interface Field {
  key: string;
  label: string;
  /** text, number, bool, choice, list, folder, file, folders or files */
  type: string;
  default: unknown;
  help: string | null;
  /** A text field: a text area */
  multiline: boolean;
  min: number | null;
  max: number | null;
  choices: { value: string; label: string }[];
}

/** An installed plugin, as Settings › Plugins shows it */
export interface PluginInfo {
  id: string;
  name: string;
  /** null when its plugin.toml couldn't be read */
  version: string | null;
  description: string;
  homepage: string | null;
  /** Its icon file, in its folder */
  icon: string | null;
  /** A git URL (maybe with #folder), or the folder it's linked from */
  source: string;
  /** The release tag it's at; null: the default branch, or linked */
  tag: string | null;
  linked: boolean;
  enabled: boolean;
  folder: string;
  readme: string | null;
  /** What it adds, in a few words each */
  adds: string[];
  /** Why it isn't loaded (empty when it is) */
  problems: string[];
  /** Its settings form */
  fields: Field[];
  /** Its settings, every field filled in */
  settings: Record<string, unknown>;
}

/** A key binding in a plugin's manifest */
export interface PluginBinding {
  keys: string[];
  action: string;
  does: string;
}

/** A `[keys.<name>]` keymap in a plugin's manifest */
export interface PluginKeymap {
  name: string;
  surface: string;
  bindings: PluginBinding[];
}

/** What a frame needs from its plugin (src-tauri/src/plugins/mod.rs FrameInfo) */
export interface FrameInfo {
  plugin: string;
  version: string;
  folder: string;
  data_folder: string;
  plugin_name: string;
  /** The surface's name (a tab's default title) */
  name: string;
  /** Its HTML file, relative to the plugin's folder */
  page: string;
  /** [name, keymap], the starting one first */
  keymaps: [string, PluginKeymap][];
  /** A tab's setup form */
  fields: Field[];
  setup_page: string | null;
}

/** What a plugin frame offers the page, for its keys */
export interface TabExports {
  /** A key while the frame has the keyboard (true: it was handled) */
  handleKey(e: KeyboardEvent): boolean;
  /** Its keys right now (for ? help, and its name in the status bar) */
  keymap(): import("../keys/keys.ts").Keymap;
}

/** What a frame can ask of the page (the page's side of the API) */
export interface Host {
  /** The projects in the list, and the selected one */
  projects(): { path: string; name: string; branch: string | null }[];
  selected(): { path: string; name: string; branch: string | null } | null;
  /** Start a command like a Toolkit button (a tab with its output, in Running); its run id */
  startRun(project: { path: string; name: string }, command: string, label: string, source: string): Promise<number>;
  stopRun(id: number): void;
  /** A run's label and project, for the `run` event */
  run(id: number): { label: string; projectPath: string } | null;
  /** The plugin's Toolkit buttons may have changed (its backend is asked again) */
  refreshToolkit(plugin: string): void;
  /** A tab's badge next to its title (null: none) */
  badge(key: string, value: string | null): void;
  /** A signal for the avatar from this frame (null takes it back) */
  mood(key: string, signal: string, value: unknown): void;
  /** The frame is gone: its signals go too */
  forget(key: string): void;
  /** One of the plugin's pages, over the whole window */
  openPage(plugin: string, id: string, data: unknown, project: { path: string; name: string; branch: string | null } | null): void;
  /** Close the page that's open */
  closePage(): void;
  /** The plugin's view: a short status next to its name in the Plugins pane */
  status(plugin: string, text: string | null): void;
}

/** A plugin's view, in the Plugins pane */
export interface View {
  plugin: string;
  name: string;
  status: boolean;
  frame: FrameInfo;
}

/** A panel on the right */
export interface PanelInfo {
  plugin: string;
  id: string;
  name: string;
  scope: "project" | "app";
  /** null: as tall as its page (up to half the column); else a number of lines */
  lines: number | null;
  frame: FrameInfo;
}

/** An official plugin in the catalog */
export interface CatalogEntry {
  id: string;
  name: string;
  description: string;
  source: string;
  recommended: boolean;
}
