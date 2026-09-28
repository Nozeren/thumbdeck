export interface Project {
  name: string;
  path: string;
  branch: string | null;
  dirty: boolean;
  /** Scan folder it was found in; null when added by hand */
  root: string | null;
  /** django, android, tauri, node, rust, go, nvim, python or folder; "plugin": see icon */
  kind: string;
  /** A plugin's .svg icon for it (a data URL) */
  icon: string | null;
  hidden: boolean;
  pinned: boolean;
}

export interface ProjectList {
  projects: Project[];
  /** Folders scanned for git repositories */
  roots: string[];
  /** Project selected last, reopened on start */
  last: string | null;
  /** The character in the top bar (empty: the octopus; "none": no character) */
  avatar: string;
}

export interface Action {
  id: string;
  label: string;
  command: string;
  /** "custom" for your own actions, otherwise the toolkit pack's id (npm, django, ...) */
  source: string;
  /** Group title in the Toolkit: the pack's name, or "yours" */
  group: string;
  description: string | null;
  /** Ask before running */
  confirm: boolean;
  /** Runs in this window of the project's tmux session instead of in thumbdeck */
  tmux: string | null;
}

/** A custom action as stored in the settings */
export interface CustomAction {
  id: string;
  name: string;
  command: string;
  confirm: boolean;
  /** Window of the project's tmux session to run in; null runs it inside thumbdeck */
  tmux: string | null;
}

export interface Details {
  actions: Action[];
  /** Detected actions you hid */
  hidden: Action[];
  /** Toolkit packs that couldn't be used, and why */
  problems: string[];
  readme: string | null;
  /** The plugin tabs turned on for the project */
  tabs: TabInfo[];
}

/** A plugin's tab that can be added to a project */
export interface Addable {
  /** "<plugin>:<tab>" */
  id: string;
  name: string;
  description: string;
}

/** A plugin's tab turned on for a project, with its setup */
export interface Tab {
  plugin: string;
  /** The tab's id in the plugin's manifest */
  tab: string;
  setup: any; // the fields the plugin declares, and the title
}

export interface TabInfo extends Tab {
  title: string;
  /** Its frame, when its plugin works */
  frame?: import("./plugins/types.ts").FrameInfo;
  /** Why it can't show (its plugin is off, broken or gone) */
  missing?: string;
}

export interface Run {
  id: number;
  projectPath: string;
  label: string;
  source: string;
  command: string;
  startedAt: number;
  endedAt: number | null;
  /** null while running */
  code: number | null;
  lines: { text: string; stderr: boolean }[];
}

/** A newer release of thumbdeck */
export interface Update {
  version: string;
  current: string;
  /** What's new (markdown) */
  notes: string;
  date: string;
}
