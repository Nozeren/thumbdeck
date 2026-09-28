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
