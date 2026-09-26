<script lang="ts">
  // The Logs tab: the project's log files, then one log
  // with levels, search, error jumps, live tail, optional sections and a detail view of one
  // entry. Keys come through handleKey() while the tab has the keyboard.
  import { invoke } from "@tauri-apps/api/core";
  import { tick } from "svelte";
  import type { Log, LogFile, Setup, Summary } from "./types.ts";
  import type { TabProps } from "../index.ts";
  import {
    age, ancestors, copyText, findLine, formatLine, jsonRows, lineOf, parentIndex, rows, type Row,
  } from "./tree.ts";

  let { path, setup, active, say, onActivate, onRelease, onEditSetup }: Omit<TabProps, "setup"> & { setup: Setup } = $props();

  const LEVELS = ["DEBUG", "INFO", "WARNING", "ERROR"];
  const ROW = 21; // px per row (keep in sync with .row height)

  // ------------------------------------------------------------ file list
  let files = $state<LogFile[]>([]);
  let missing = $state<string[]>([]);
  let listed = $state(false);
  let listCursor = $state(0);
  let summary = $state<Summary | { error: string } | null>(null);
  let summaryFor = "";

  async function refreshList() {
    const res = await invoke<{ files: LogFile[]; missing: string[] }>("logs_list", { path, setup });
    const selected = files[listCursor]?.path;
    files = res.files;
    missing = res.missing;
    listed = true;
    const i = files.findIndex((f) => f.path === selected);
    listCursor = i >= 0 ? i : Math.min(listCursor, Math.max(0, files.length - 1));
    loadSummary();
  }

  async function loadSummary() {
    const f = files[listCursor];
    const key = f ? `${f.path}:${f.mtime}` : "";
    if (key === summaryFor) return;
    summaryFor = key;
    summary = null;
    if (!f) return;
    try {
      const s = await invoke<Summary>("logs_summary", { file: f.path, setup });
      if (summaryFor === key) summary = s;
    } catch (err) {
      if (summaryFor === key) summary = { error: String(err) };
    }
  }

  function moveList(to: number) {
    if (!files.length) return;
    listCursor = Math.max(0, Math.min(files.length - 1, to));
    loadSummary();
    document.getElementById(`log-file-${listCursor}`)?.scrollIntoView({ block: "nearest" });
  }

  // The list refreshes every 3s while shown, so a run's new log shows up by itself
  $effect(() => {
    if (log) return;
    refreshList();
    const timer = setInterval(refreshList, 3000);
    return () => clearInterval(timer);
  });

  // ------------------------------------------------------------ one log
  let file = $state<LogFile | null>(null);
  let log = $state<Log | null>(null);
  let opened = $state(new Set<string>());
  let hidden = $state(new Set<string>());
  let cursorKey = $state<string | null>(null);
  let cursorLine = 0; // where the cursor was, for when its row goes away (a level hidden)
  let tailing = $state(false);
  let status = $state(""); // search status
  let searching = $state(false);
  let query = $state("");
  let matches: number[] = [];
  let matchIndex = -1;
  let scroller = $state<HTMLElement | null>(null);
  let scrollTop = $state(0);
  let height = $state(400);
  let searchEl = $state<HTMLInputElement | null>(null);

  const list = $derived<Row[]>(log ? rows(log.outline, log.lines, opened, hidden) : []);
  const parentsOf = $derived(log ? ancestors(log.outline) : new Map<number, string[]>());
  const cursor = $derived.by(() => {
    const i = list.findIndex((r) => r.key === cursorKey);
    if (i >= 0) return i;
    const next = list.findIndex((r) => lineOf(r) >= cursorLine);
    return next >= 0 ? next : Math.max(0, list.length - 1);
  });
  const first = $derived(Math.max(0, Math.floor(scrollTop / ROW) - 10));
  const shown = $derived(list.slice(first, first + Math.ceil(height / ROW) + 20));

  async function openLog(f: LogFile) {
    try {
      const loaded = await invoke<Log>("logs_open", { file: f.path, setup });
      file = f;
      log = loaded;
      opened = new Set();
      hidden = new Set();
      cursorKey = null;
      cursorLine = 0;
      tailing = false;
      status = "";
      matches = [];
      await tick();
      if (scroller) scroller.scrollTop = 0;
    } catch (err) {
      say(String(err), true);
    }
  }

  function backToList() {
    log = null;
    file = null;
    detail = null;
    tailing = false;
  }

  /** Read the file again, keeping what's open and where the cursor is */
  async function reload() {
    if (!file) return;
    const atEnd = list.length > 0 && cursor === list.length - 1;
    try {
      log = await invoke<Log>("logs_open", { file: file.path, setup });
    } catch (err) {
      say(String(err), true);
      return;
    }
    if (atEnd && tailing) setCursor(list.length - 1); // follow the end while tailing
  }

  function setCursor(i: number) {
    if (!list.length) return;
    i = Math.max(0, Math.min(list.length - 1, i));
    cursorKey = list[i].key;
    cursorLine = lineOf(list[i]);
    if (!scroller) return;
    if (i * ROW < scroller.scrollTop) scroller.scrollTop = i * ROW;
    else if ((i + 1) * ROW > scroller.scrollTop + height) scroller.scrollTop = (i + 1) * ROW - height;
  }

  /** Open the blocks around a line, then put the cursor on it */
  async function reveal(line: number) {
    const keys = parentsOf.get(line) ?? [];
    if (keys.some((k) => !opened.has(k))) opened = new Set([...opened, ...keys]);
    await tick();
    const i = list.findIndex((r) => r.type === "line" && r.line === line);
    if (i >= 0) setCursor(i);
  }

  function toggle(r: Row) {
    if (r.type !== "block") return;
    const next = new Set(opened);
    if (next.has(r.key)) next.delete(r.key);
    else next.add(r.key);
    opened = next;
  }

  function jumpTo(direction: 1 | -1, match: (l: Log["lines"][number]) => boolean, none: string) {
    if (!log?.lines.length) return;
    const from = list[cursor] ? lineOf(list[cursor]) : direction > 0 ? -1 : 0;
    const found = findLine(log.lines, from, direction, hidden, match);
    if (found === null) say(none);
    else reveal(found);
  }

  function toggleLevel(level: string) {
    const next = new Set(hidden);
    if (next.has(level)) next.delete(level);
    else next.add(level);
    hidden = next;
    say(`${level} lines ${next.has(level) ? "hidden" : "visible"}`);
  }

  // Search: Enter jumps to the first match, then n / N go through them
  function submitSearch() {
    const q = query.trim().toLowerCase();
    searching = false;
    matches = [];
    matchIndex = -1;
    if (q && log) {
      matches = log.lines.flatMap((l, i) => (!hidden.has(l.level) && l.message.toLowerCase().includes(q) ? [i] : []));
    }
    if (!q) status = "";
    else if (!matches.length) status = `no matches for '${q}'`;
    else goToMatch(1);
  }

  function goToMatch(direction: 1 | -1) {
    if (!matches.length) return;
    matchIndex = (matchIndex + direction + matches.length) % matches.length;
    reveal(matches[matchIndex]);
    status = `match ${matchIndex + 1}/${matches.length}`;
  }

  function searchKey(e: KeyboardEvent) {
    if (e.key === "Enter") submitSearch();
    else if (e.key === "Escape") (searching = false), (status = "");
    else return;
    e.preventDefault();
    e.stopPropagation(); // Esc here only closes the search
  }

  async function copy(text: string, what: string) {
    try {
      await navigator.clipboard.writeText(text);
      say(`copied ${what}`);
    } catch {
      say("couldn't copy to the clipboard", true);
    }
  }

  // Live tail: read the file again when it grows (every 2s)
  let lastSize = -1;
  $effect(() => {
    if (!tailing || !file) return;
    const f = file.path;
    lastSize = log?.size ?? -1;
    const timer = setInterval(async () => {
      const size = await invoke<number | null>("logs_size", { file: f });
      if (size !== null && size !== lastSize) {
        lastSize = size;
        reload();
      }
    }, 2000);
    return () => clearInterval(timer);
  });

  // ------------------------------------------------------------ detail view (one entry)
  let detail = $state<number | null>(null); // line number
  let detailCursor = $state(0);
  let detailClosed = $state(new Set<string>());
  let detailOpened = $state(new Set<string>());
  const detailRows = $derived(detail !== null && log ? jsonRows(log.lines[detail].data, detailClosed, detailOpened) : []);
  // The selected value in full, when the row can't show it (long, or several lines: a traceback)
  const fullValue = $derived.by(() => {
    const v = detailRows[detailCursor]?.value;
    return typeof v === "string" && (v.length > 80 || v.includes("\n")) ? v : null;
  });

  function showDetail(line: number) {
    detail = line;
    detailCursor = 0;
    detailClosed = new Set();
    detailOpened = new Set();
  }

  function toggleDetail(i: number) {
    const r = detailRows[i];
    if (!r?.container) return;
    if (r.depth === 0) {
      const next = new Set(detailClosed);
      if (r.open) next.add(r.path);
      else next.delete(r.path);
      detailClosed = next;
    } else {
      const next = new Set(detailOpened);
      if (r.open) next.delete(r.path);
      else next.add(r.path);
      detailOpened = next;
    }
  }

  function detailKey(e: KeyboardEvent): boolean {
    const r = detailRows[detailCursor];
    const move = (i: number) => {
      detailCursor = Math.max(0, Math.min(detailRows.length - 1, i));
      document.getElementById(`log-detail-${detailCursor}`)?.scrollIntoView({ block: "nearest" });
    };
    switch (e.key) {
      case "j": case "ArrowDown": move(detailCursor + 1); break;
      case "k": case "ArrowUp": move(detailCursor - 1); break;
      case "g": move(0); break;
      case "G": move(detailRows.length - 1); break;
      case "l": case "Enter": case " ": toggleDetail(detailCursor); break;
      case "h": {
        if (r?.container && r.open) toggleDetail(detailCursor);
        else {
          const depth = r?.depth ?? 0;
          for (let i = detailCursor - 1; i >= 0; i--) if (detailRows[i].depth < depth) { move(i); break; }
        }
        break;
      }
      case "y": if (r) copy(copyText(r.value), `the value of '${r.key}'`); break;
      case "Y": if (detail !== null && log) copy(JSON.stringify(log.lines[detail].data, null, 2), "the full entry"); break;
      case "q": case "Escape": detail = null; break;
      default: return false;
    }
    return true;
  }

  // ------------------------------------------------------------ keys
  let help = $state(false);

  /** A key while the tab has the keyboard; false when it isn't one of the tab's */
  export function handleKey(e: KeyboardEvent): boolean {
    if (help) {
      help = false; // any key closes the help
      return true;
    }
    if (e.key === "?") return (help = true);
    if (e.key === "S" && detail === null) return (onEditSetup(), true);
    if (detail !== null) return detailKey(e);
    if (!log) return listKey(e);

    const r = list[cursor];
    const page = Math.max(1, Math.floor(height / ROW));
    if (e.ctrlKey) {
      const by = { d: page / 2, u: -page / 2, f: page, b: -page }[e.key];
      if (by === undefined) return false;
      setCursor(cursor + Math.trunc(by));
      return true;
    }
    if (e.metaKey || e.altKey) return false;
    switch (e.key) {
      case "j": case "ArrowDown": setCursor(cursor + 1); break;
      case "k": case "ArrowUp": setCursor(cursor - 1); break;
      case "l": case "ArrowRight": case "Enter": case " ":
        if (r?.type === "block") toggle(r);
        else if (r) showDetail(r.line);
        break;
      case "h": case "ArrowLeft":
        if (r?.type === "block" && r.open) toggle(r);
        else {
          const p = parentIndex(list, cursor);
          if (p !== null) setCursor(p);
        }
        break;
      case "g": if (log.lines.length) reveal(0); break;
      case "G": if (log.lines.length) reveal(log.lines.length - 1); break;
      case "e": jumpTo(1, (l) => l.level === "ERROR", "no ERROR lines"); break;
      case "E": jumpTo(-1, (l) => l.level === "ERROR", "no ERROR lines"); break;
      case "/": searching = true; query = ""; tick().then(() => searchEl?.focus()); break;
      case "n": goToMatch(1); break;
      case "N": goToMatch(-1); break;
      case "t": tailing = !tailing; say(`live tail ${tailing ? "on" : "off"}`); break;
      case "d": toggleLevel("DEBUG"); break;
      case "i": toggleLevel("INFO"); break;
      case "w": toggleLevel("WARNING"); break;
      case "r": toggleLevel("ERROR"); break;
      case "a": hidden = new Set(); say("all levels visible"); break;
      case "y":
        if (r?.type === "line") copy(log.lines[r.line].message, "the message");
        else if (r) copy(r.node.name, "the title");
        break;
      case "Y": if (r?.type === "line") copy(JSON.stringify(log.lines[r.line].data), "the JSON"); break;
      case "-": case "q": backToList(); break;
      case "Escape":
        if (!status) return false; // nothing to close: give the keyboard back
        status = "";
        break;
      default: return false;
    }
    return true;
  }

  function listKey(e: KeyboardEvent): boolean {
    if (e.ctrlKey || e.metaKey || e.altKey) return false;
    switch (e.key) {
      case "j": case "ArrowDown": moveList(listCursor + 1); break;
      case "k": case "ArrowUp": moveList(listCursor - 1); break;
      case "g": moveList(0); break;
      case "G": moveList(files.length - 1); break;
      case "l": case "Enter": if (files[listCursor]) openLog(files[listCursor]); break;
      case "q": onRelease(); break;
      default: return false;
    }
    return true;
  }

  const levelColor = (level: string) => ({ DEBUG: "blue", INFO: "aqua", WARNING: "orange", ERROR: "red" })[level] ?? "fg";
  const kindLabel = (k: string) => (k === "string" ? "str" : k);
</script>

<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
<div class="viewer" class:active onmousedown={onActivate}>
  {#if !log}
    <header class="bar">
      <strong>{setup.title || "Logs"}</strong>
      <span class="dim">{setup.folders.filter((f) => f.trim()).join(", ")} · {setup.pattern}</span>
      <span class="spacer"></span>
      <button class="tool" title="Set up this tab (S)" onclick={onEditSetup}>⚙ setup</button>
    </header>
    {#if listed && !files.length && missing.length}
      <p class="note">not found: {missing.join(", ")}. Check the folders in ⚙ setup.</p>
    {/if}
    <div class="picker">
      <ul class="files">
        {#each files as f, i (f.path)}
          <li id="log-file-{i}">
            <button class="file" class:cursor={i === listCursor} onclick={() => moveList(i)} ondblclick={() => openLog(f)}>
              <span class="dot" class:err={f.error}>●</span>
              <span class="fname">{f.name}</span>
              <span class="dim">{age(f.mtime)}</span>
            </button>
          </li>
        {:else}
          <li class="dim empty">{listed ? `No ${setup.pattern} files here yet.` : "Looking…"}</li>
        {/each}
      </ul>
      <aside class="preview">
        {#if files[listCursor]}
          <strong>{files[listCursor].name}</strong>
          {#if summary && "error" in summary}
            <p class="err">{summary.error}</p>
          {:else if summary}
            <p>{summary.total} lines{summary.duration ? ` · ${summary.duration}` : ""}{summary.blocks ? ` · ${summary.blocks} sections` : ""}</p>
            <p>
              {#each summary.levels as [level, n]}
                {#if n}<span class="lvl" style="color: var(--{levelColor(level)})">{level} {n}</span>{/if}
              {/each}
            </p>
            {#if summary.first_error}
              <p class="dim">first error</p>
              <p class="err first-error">{summary.first_error}</p>
            {/if}
            <p class="dim">Enter or double-click opens it</p>
          {:else}
            <p class="dim">…</p>
          {/if}
        {/if}
      </aside>
    </div>
  {:else if file}
    <header class="bar">
      <button class="tool" title="Back to the files (-)" onclick={backToList}>‹ {setup.title || "Logs"}</button>
      <span class="mode" class:search={searching}>{searching ? "SEARCH" : "NORMAL"}</span>
      <strong class="fname">{file.name}</strong>
      <span class="levels">
        {#each LEVELS as level}
          <button class="lvl" class:off={hidden.has(level)} style="color: var(--{levelColor(level)})"
                  title="Show / hide {level} ({level[0].toLowerCase()})" onclick={() => toggleLevel(level)}>{level[0]}</button>
        {/each}
      </span>
      {#if tailing}<span class="tail" title="Live tail (t)">● tail</span>{/if}
      <span class="spacer"></span>
      <span class="dim">{status}</span>
      <span class="dim">{list[cursor] ? `line ${lineOf(list[cursor]) + 1} of ${log.lines.length}` : `${log.lines.length} lines`}</span>
    </header>
    {#if searching}
      <input class="search" bind:this={searchEl} bind:value={query} onkeydown={searchKey}
             placeholder="Search messages… (Enter jumps, then n / N)" />
    {/if}
    <div class="rows" bind:this={scroller} bind:clientHeight={height} onscroll={() => (scrollTop = scroller?.scrollTop ?? 0)}>
      <div style="height: {list.length * ROW}px; position: relative">
        {#each shown as r, j (r.key)}
          {@const i = first + j}
          <div class="row" class:cursor={i === cursor} style="top: {i * ROW}px; padding-left: {8 + r.depth * 16}px"
               onclick={() => setCursor(i)}
               ondblclick={() => (r.type === "block" ? toggle(r) : showDetail(r.line))}>
            {#if r.type === "block"}
              <button class="caret" onclick={(e) => { e.stopPropagation(); setCursor(i); toggle(r); }}>{r.open ? "▾" : "▸"}</button>
              <span class:err={r.node.has_error} class:ok={!r.node.has_error}>{r.node.kind === "bookmark" ? "◆" : "▸"}</span>
              <strong>{r.node.name}</strong>
              <span class="dim">({r.node.last_line - r.node.first_line + 1} lines)</span>
            {:else}
              {@const l = log.lines[r.line]}
              <span class="text" class:bold={l.level === "ERROR"} style="color: var(--{levelColor(l.level)})">{formatLine(l)}</span>
            {/if}
          </div>
        {/each}
      </div>
    </div>
  {/if}

  {#if detail !== null && log}
    <div class="overlay">
      <header class="bar">
        <strong>line {detail + 1}: full entry</strong>
        <span class="spacer"></span>
        <span class="dim">y value · Y all · q close</span>
      </header>
      <div class="json">
        {#each detailRows as d, i (d.path)}
          <div id="log-detail-{i}" class="row static" class:cursor={i === detailCursor} style="padding-left: {8 + d.depth * 16}px"
               onclick={() => { detailCursor = i; toggleDetail(i); }}>
            {#if d.container}<span class="caret">{d.open ? "▾" : "▸"}</span>{/if}
            <span class="key" class:container={d.container}>{d.key}</span>{#if !d.container}:{/if}
            <span class="v {kindLabel(d.kind)}">{d.display}</span>
          </div>
        {/each}
      </div>
      {#if fullValue !== null}<pre class="full">{fullValue}</pre>{/if}
    </div>
  {/if}

  {#if help}
    <div class="overlay help">
      <strong>Logs: keys</strong> <span class="dim">(any key closes this)</span>
      <dl>
        <dt>j / k</dt><dd>down / up</dd>
        <dt>h / l</dt><dd>close / open a section (h on a line: its section)</dd>
        <dt>g / G</dt><dd>first / last line</dd>
        <dt>ctrl d u f b</dt><dd>half / full page down / up</dd>
        <dt>e / E</dt><dd>next / previous ERROR</dd>
        <dt>Enter, Space</dt><dd>on a line: its full entry; on a section: open / close</dd>
        <dt>y / Y</dt><dd>copy the message (section: its title) / the entry as JSON</dd>
        <dt>/ , n / N</dt><dd>search, next / previous match</dd>
        <dt>t</dt><dd>live tail</dd>
        <dt>d i w r / a</dt><dd>show / hide DEBUG INFO WARNING ERROR / show all</dd>
        <dt>- or q</dt><dd>back to the files (in the files: give keys back)</dd>
        <dt>S</dt><dd>set up this tab (folders, how lines are read, sections)</dd>
        <dt>Esc</dt><dd>close; give the keyboard back to thumbdeck</dd>
      </dl>
    </div>
  {/if}
</div>

<style>
  .viewer { position: relative; flex: 1; min-height: 0; display: flex; flex-direction: column; font: 12.5px var(--mono);
        border-top: 2px solid transparent; }
  .viewer.active { border-top-color: var(--orange); }
  .bar { display: flex; align-items: center; gap: 10px; padding: 6px 12px; border-bottom: 1px solid var(--bg2); white-space: nowrap; }
  .spacer { flex: 1; }
  .dim { color: var(--grey); }
  .err { color: var(--red); } .ok { color: var(--green); }
  .bold { font-weight: 700; }
  .note { margin: 6px 12px; color: var(--orange); }
  .tool { padding: 2px 8px; border-radius: 6px; color: var(--grey); font: 12px var(--mono); }
  .tool:hover { background: var(--bg2); color: var(--fg); }
  .mode { padding: 0 6px; border-radius: 4px; background: var(--bg2); color: var(--aqua); font-weight: 700; }
  .mode.search { color: var(--orange); }
  .fname { overflow: hidden; text-overflow: ellipsis; }
  .levels { display: flex; gap: 2px; }
  .lvl { font: 700 12px var(--mono); padding: 0 3px; }
  .lvl.off { opacity: 0.35; text-decoration: line-through; }
  .tail { color: var(--green); }
  .search { margin: 6px 12px; background: var(--bg1); border: 1px solid var(--orange); border-radius: 8px; padding: 6px 10px;
            color: var(--fg); font: 12.5px var(--mono); outline: none; }

  .picker { flex: 1; min-height: 0; display: grid; grid-template-columns: minmax(0, 1.2fr) minmax(0, 1fr); }
  .files { list-style: none; margin: 0; padding: 6px; overflow: auto; border-right: 1px solid var(--bg2); }
  .file { width: 100%; display: flex; gap: 8px; align-items: center; padding: 3px 8px; border-radius: 6px; text-align: left; font: 12.5px var(--mono); }
  .file .fname { flex: 1; }
  .file.cursor { background: var(--bg2); }
  .viewer.active .file.cursor { outline: 1px solid var(--orange); }
  .dot { color: var(--green); } .dot.err { color: var(--red); }
  .empty { padding: 8px; }
  .preview { padding: 10px 14px; overflow: auto; }
  .preview p { margin: 6px 0; }
  .preview .lvl { margin-right: 8px; }
  .first-error { white-space: pre-wrap; word-break: break-word; }

  .rows { flex: 1; min-height: 0; overflow: auto; }
  .row { position: absolute; left: 0; right: 0; height: 21px; line-height: 21px; display: flex; gap: 6px; align-items: center;
         white-space: pre; overflow: hidden; cursor: default; padding-right: 8px; }
  .row.static { position: static; }
  .row:hover { background: var(--bg1); }
  .row.cursor { background: var(--bg2); }
  .viewer.active .row.cursor { box-shadow: inset 2px 0 var(--orange); }
  .text { overflow: hidden; text-overflow: ellipsis; }
  .caret { width: 12px; color: var(--grey); font: 12px var(--mono); }

  .overlay { position: absolute; inset: 0; background: var(--bg0); display: flex; flex-direction: column; z-index: 5; }
  .json { flex: 0 1 auto; min-height: 0; overflow: auto; padding: 4px 0; }
  .key { color: var(--blue); font-weight: 700; }
  .key.container { color: var(--purple); }
  .v { overflow: hidden; text-overflow: ellipsis; }
  .v.str { color: var(--yellow); } .v.number { color: var(--aqua); } .v.bool, .v.null { color: var(--purple); }
  .v.container { color: var(--grey); }
  .full { flex: 1 1 40%; min-height: 0; overflow: auto; margin: 0; padding: 10px 14px; border-top: 1px solid var(--bg2);
          white-space: pre-wrap; word-break: break-word; font: 12.5px/1.45 var(--mono); }
  .help { padding: 16px 20px; overflow: auto; }
  .help dl { display: grid; grid-template-columns: max-content 1fr; gap: 4px 16px; }
  .help dt { color: var(--orange); } .help dd { margin: 0; }
</style>
