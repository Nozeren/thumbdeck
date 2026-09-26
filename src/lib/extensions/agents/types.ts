// The Agents tab: the shapes the Rust side sends (src-tauri/src/extensions/agents)

export interface Setup {
  title: string;
  /** Also list your own subagents and skills (~/.claude/agents, ~/.claude/skills) */
  user_agents: boolean;
}

export interface Tokens {
  input: number;
  output: number;
  cache_read: number;
  cache_write: number;
}

/** working: mid-turn and writing; idle: turn over; stale: mid-turn but quiet for long */
export type Status = "working" | "idle" | "stale";

export interface Subagent {
  id: string;
  agent_type: string;
  description: string;
  model: string | null;
  tokens: Tokens;
  status: Status;
  started: string;
  last_activity: string;
  depth: number;
  file: string;
}

export interface Session {
  id: string;
  title: string;
  first_prompt: string;
  started: string;
  last_activity: string;
  model: string | null;
  branch: string | null;
  tokens: Tokens;
  cost: number | null;
  /** Answers came after the cost was written: it's at least that */
  cost_behind: boolean;
  prompts: number;
  status: Status;
  subagents: Subagent[];
  file: string;
}

export interface Defined {
  name: string;
  description: string;
  tools: string[];
  model: string | null;
  color: string | null;
  scope: "project" | "user";
  file: string;
  instructions: string;
}

export interface Skill {
  name: string;
  description: string;
  scope: "project" | "user";
  file: string;
  instructions: string;
}

export interface AgentList {
  sessions: Session[];
  defined: Defined[];
  skills: Skill[];
  dir: string;
}

export interface ToolResult {
  text: string;
  error: boolean;
  length: number;
}

export type Entry =
  | { kind: "prompt"; time: string; text: string }
  | { kind: "text"; time: string; text: string }
  | { kind: "tool"; time: string; id: string; name: string; input: any; result: ToolResult | null; agent: string | null }
  | { kind: "note"; time: string; text: string };
