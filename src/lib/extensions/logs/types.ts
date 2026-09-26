// The Logs tab: the shapes the Rust side sends (src-tauri/src/extensions/logs)

export type Kind = "bookmark" | "index";

export type Alias =
  | { type: "regex"; value: string }
  | { type: "rewrite"; value: string }
  | { type: "replace"; value: { from: string; to: string } }
  | { type: "prefix"; value: string };

export interface BlockConfig {
  name: string;
  /** bookmark: a top-level section (◆); index: a section inside one (▸) */
  kind: Kind;
  start: string[];
  end: string[];
  alias: Alias | null;
}

export interface Setup {
  title: string;
  folders: string[];
  pattern: string;
  blocks: BlockConfig[];
  fields: { message: string[]; level: string[]; time: string[] };
  /** Plain-text lines: a regex with the groups time, level and message; "" reads them automatically */
  line_pattern: string;
}

export interface LogFile {
  name: string;
  path: string;
  mtime: number;
  size: number;
  error: boolean;
}

export interface Line {
  level: string;
  time: string;
  message: string;
  data: Record<string, unknown>;
}

export type Node =
  | { type: "line"; line: number }
  | { type: "block"; kind: Kind; name: string; first_line: number; last_line: number; has_error: boolean; children: Node[] };

export type BlockNode = Extract<Node, { type: "block" }>;

export interface Log {
  lines: Line[];
  outline: Node[];
  size: number;
}

export interface Summary {
  total: number;
  levels: [string, number][];
  blocks: number;
  first_error: string | null;
  duration: string | null;
}
