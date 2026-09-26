// Syntax colours for the review page: highlight.js with the languages projects here use
// (a few, to keep the app small). Lines are coloured one at a time, so a construct spanning
// lines (a long string, a block comment) may be coloured only on its first line.
import hljs from "highlight.js/lib/core";
import bash from "highlight.js/lib/languages/bash";
import css from "highlight.js/lib/languages/css";
import dockerfile from "highlight.js/lib/languages/dockerfile";
import gherkin from "highlight.js/lib/languages/gherkin";
import go from "highlight.js/lib/languages/go";
import ini from "highlight.js/lib/languages/ini";
import java from "highlight.js/lib/languages/java";
import javascript from "highlight.js/lib/languages/javascript";
import json from "highlight.js/lib/languages/json";
import markdown from "highlight.js/lib/languages/markdown";
import python from "highlight.js/lib/languages/python";
import rust from "highlight.js/lib/languages/rust";
import sql from "highlight.js/lib/languages/sql";
import typescript from "highlight.js/lib/languages/typescript";
import xml from "highlight.js/lib/languages/xml";
import yaml from "highlight.js/lib/languages/yaml";
import type { Segment } from "./diff.ts";

for (const [name, lang] of Object.entries({ bash, css, dockerfile, gherkin, go, ini, java, javascript, json, markdown, python, rust, sql, typescript, xml, yaml })) {
  hljs.registerLanguage(name, lang);
}

const BY_EXTENSION: Record<string, string> = {
  sh: "bash", bash: "bash", zsh: "bash", css: "css", scss: "css", go: "go", toml: "ini", ini: "ini", cfg: "ini",
  java: "java", kt: "java", js: "javascript", mjs: "javascript", cjs: "javascript", jsx: "javascript", json: "json",
  md: "markdown", py: "python", rs: "rust", sql: "sql", ts: "typescript", tsx: "typescript", html: "xml", xml: "xml",
  svelte: "xml", vue: "xml", svg: "xml", yml: "yaml", yaml: "yaml", feature: "gherkin",
};

/** The language of a file, from its name (null: plain text) */
export function languageFor(path: string): string | null {
  const name = path.split("/").at(-1)?.toLowerCase() ?? "";
  if (name === "dockerfile" || name.startsWith("dockerfile.")) return "dockerfile";
  return BY_EXTENSION[name.split(".").at(-1) ?? ""] ?? null;
}

const escape = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

function colour(text: string, lang: string | null): string {
  if (!lang || !text) return escape(text);
  try {
    return hljs.highlight(text, { language: lang, ignoreIllegals: true }).value;
  } catch {
    return escape(text);
  }
}

/** A line as HTML: coloured, with the words that changed marked (<mark>) */
export function lineHtml(text: string, lang: string | null, segments: Segment[] | null): string {
  if (!segments) return colour(text, lang);
  return segments.map((s) => (s.changed ? `<mark>${colour(s.text, lang)}</mark>` : colour(s.text, lang))).join("");
}
