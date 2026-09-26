<script lang="ts">
  // Setting up the Logs tab for a project: where the logs are, how lines are read, and
  // (optionally) what marks a section
  import { invoke } from "@tauri-apps/api/core";
  import type { Alias, BlockConfig, Setup } from "./types.ts";
  import type { SetupProps } from "../index.ts";

  let { setup, isNew, project, onSave, onRemove, onCancel }: Omit<SetupProps, "setup" | "onSave"> & { setup: Setup; onSave: (setup: Setup) => void } = $props();

  // Edited as text: one folder / start / end per line, field names separated by commas
  const lines = (list: string[]) => list.join("\n");
  const unlines = (text: string) => text.split("\n").map((s) => s.trim()).filter(Boolean);
  const commas = (text: string) => text.split(",").map((s) => s.trim()).filter(Boolean);

  // svelte-ignore state_referenced_locally (a copy to edit; saved on Save)
  let title = $state(setup.title);
  // svelte-ignore state_referenced_locally
  let folders = $state(lines(setup.folders));
  // svelte-ignore state_referenced_locally
  let pattern = $state(setup.pattern);
  // svelte-ignore state_referenced_locally
  let blocks = $state(setup.blocks.map(toRow));
  // svelte-ignore state_referenced_locally
  let fields = $state({ message: setup.fields.message.join(", "), level: setup.fields.level.join(", "), time: setup.fields.time.join(", ") });
  // svelte-ignore state_referenced_locally
  let linePattern = $state(setup.line_pattern);
  // svelte-ignore state_referenced_locally
  let advanced = $state(setup.line_pattern !== "");
  let problems = $state<string[]>([]);

  interface BlockRow {
    name: string;
    kind: BlockConfig["kind"];
    start: string;
    end: string;
    alias: "" | Alias["type"];
    value: string;
    to: string;
  }

  function toRow(b: BlockConfig): BlockRow {
    const a = b.alias;
    return {
      name: b.name, kind: b.kind, start: lines(b.start), end: lines(b.end),
      alias: a?.type ?? "",
      value: a ? (a.type === "replace" ? a.value.from : a.value) : "",
      to: a?.type === "replace" ? a.value.to : "",
    };
  }

  function fromRow(r: BlockRow): BlockConfig {
    let alias: Alias | null = null;
    if (r.alias === "replace") alias = { type: "replace", value: { from: r.value, to: r.to } };
    else if (r.alias) alias = { type: r.alias, value: r.value };
    return { name: r.name.trim() || "section", kind: r.kind, start: unlines(r.start), end: unlines(r.end), alias };
  }

  const current = $derived<Setup>({
    title: title.trim(),
    folders: unlines(folders),
    pattern: pattern.trim() || "*.log",
    blocks: blocks.map(fromRow),
    fields: { message: commas(fields.message), level: commas(fields.level), time: commas(fields.time) },
    line_pattern: linePattern,
  });

  // Check the setup as it's edited
  $effect(() => {
    const s = current;
    const timer = setTimeout(async () => (problems = await invoke<string[]>("logs_check", { setup: s })), 200);
    return () => clearTimeout(timer);
  });

  const newBlock = (): BlockRow => ({ name: "", kind: "index", start: "", end: "", alias: "", value: "", to: "" });
  const aliasHelp: Record<string, string> = {
    "": "the start line itself",
    regex: "the first (group) of this regex",
    rewrite: "always this text",
    replace: "the line, with one text replaced",
    prefix: "the line, with this before it",
  };
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" onclick={(e) => e.target === e.currentTarget && onCancel()}>
  <form class="dialog wide" onsubmit={(e) => { e.preventDefault(); if (!problems.length) onSave(current); }}>
    <h2><span class="dot orange"></span>{isNew ? "Add a Logs tab" : "Set up the Logs tab"} <span class="for">{project}</span></h2>

    <div class="cols">
      <label>Tab title <input bind:value={title} placeholder="Logs" {@attach (el) => el.select()} /></label>
      <label>Log files <input bind:value={pattern} placeholder="*.log" /></label>
    </div>
    <label>Folders <span class="sub">one per line: in the project ("." is the project itself), or a full path</span>
      <textarea bind:value={folders} rows="3" placeholder="."></textarea>
    </label>

    <div class="blocks-head"><span>Sections <span class="sub">optional</span></span></div>
    <p class="hint">Fold a log into parts (a request, a test, a deploy step…): a section starts at a line whose
      message contains a start text and ends at the next line with an end text (or at the end of the file).
      <b>◆ top</b> sections hold <b>▸ inner</b> ones.</p>
    <div class="blocks">
      {#each blocks as b, i}
        <fieldset>
          <div class="cols">
            <label>Name <input bind:value={b.name} placeholder="request" /></label>
            <label>Kind
              <select bind:value={b.kind}>
                <option value="bookmark">◆ top</option>
                <option value="index">▸ inner</option>
              </select>
            </label>
            <button type="button" class="icon" title="Remove this section" onclick={() => blocks.splice(i, 1)}>×</button>
          </div>
          <div class="cols">
            <label>Starts at <span class="sub">one text per line</span><textarea bind:value={b.start} rows="2"></textarea></label>
            <label>Ends at <span class="sub">one text per line</span><textarea bind:value={b.end} rows="2"></textarea></label>
          </div>
          <div class="cols">
            <label>Title
              <select bind:value={b.alias}>
                <option value="">start line</option>
                <option value="regex">regex</option>
                <option value="rewrite">fixed text</option>
                <option value="replace">replace</option>
                <option value="prefix">prefix</option>
              </select>
            </label>
            {#if b.alias}
              <label>{b.alias === "replace" ? "Replace" : b.alias === "regex" ? "Regex" : "Text"}
                <input bind:value={b.value} /></label>
            {/if}
            {#if b.alias === "replace"}<label>With <input bind:value={b.to} /></label>{/if}
          </div>
          <p class="hint">Title: {aliasHelp[b.alias]}</p>
        </fieldset>
      {:else}
        <p class="hint">None: the log is one list of lines.</p>
      {/each}
      <button type="button" class="ghost add" onclick={() => blocks.push(newBlock())}>+ section</button>
    </div>

    <button type="button" class="fold" onclick={() => (advanced = !advanced)}>
      <span class="caret">{advanced ? "▾" : "▸"}</span>Advanced: how lines are read
    </button>
    {#if advanced}
      <p class="hint">JSON lines: the field names to read, first found wins.</p>
      <div class="cols">
        <label>Message <input bind:value={fields.message} /></label>
        <label>Level <input bind:value={fields.level} /></label>
        <label>Time <input bind:value={fields.time} /></label>
      </div>
      <label>Plain-text lines <span class="sub">empty: read automatically (a time at the start, a level word
        like ERROR or level=warn). Or a regex with the groups time, level and message; lines that don't match
        belong to the one before.</span>
        <input bind:value={linePattern} placeholder="automatic" />
      </label>
    {/if}

    {#each problems as p}<p class="problem">{p}</p>{/each}
    <div class="buttons">
      {#if !isNew}<button type="button" class="ghost danger" onclick={onRemove}>Remove tab</button>{/if}
      <span class="spacer"></span>
      <button type="button" class="ghost" onclick={onCancel}>Cancel</button>
      <button type="submit" class="primary" disabled={problems.length > 0}>{isNew ? "Add" : "Save"}</button>
    </div>
  </form>
</div>

<style>
  h2 { display: flex; align-items: center; gap: 8px; margin: 0; font: 600 13px var(--mono); }
  .wide { width: min(760px, 94vw); max-height: 90vh; overflow: auto; }
  .cols { display: flex; gap: 10px; align-items: flex-end; }
  .cols > label { flex: 1; }
  .sub { color: var(--grey-dim); font-size: 11px; }
  select { background: var(--bg1); border: 1px solid var(--bg2); border-radius: 8px; padding: 7px 8px; color: var(--fg); font: 12.5px var(--mono); }
  .blocks-head { display: flex; align-items: center; justify-content: space-between; font: 12px var(--mono); color: var(--grey); }
  .blocks { display: flex; flex-direction: column; gap: 8px; }
  fieldset { border: 1px solid var(--bg2); border-radius: 10px; padding: 10px; display: flex; flex-direction: column; gap: 8px; margin: 0; }
  fieldset .hint { margin: 0; }
  .add { align-self: flex-start; }
  .problem { margin: 0; color: var(--red); font: 12px var(--mono); }
  .spacer { flex: 1; }
  .danger { color: var(--red); }
  .fold { align-self: flex-start; }
</style>
