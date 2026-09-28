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
}
