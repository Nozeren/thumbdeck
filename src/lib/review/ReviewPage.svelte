<script lang="ts">
  // The review page: a diff over the whole window, one file at a time. The changed files on
  // the left (tick them off as viewed), the file's changes on the right, side by side or in one
  // column, with syntax colours and the words that changed marked. Read-only. Keys come
  // through handleKey() (the page sends them all here while it's open).
  import { tick } from "svelte";
  import type { ReviewRequest } from "./types.ts";
  import { parseDiff, splitRows, unifiedRows, wordDiff, type DiffFile, type Line, type Row, type Segment } from "./diff.ts";
  import { languageFor, lineHtml } from "./highlight.ts";

  let { request, onClose }: { request: ReviewRequest; onClose: () => void } = $props();

  let files = $state<DiffFile[]>([]);
  let error = $state("");
  let loading = $state(true);
  let current = $state(0);

  async function load() {
    loading = true;
    try {
      files = parseDiff(await request.load());
      error = "";
      const start = files.findIndex((f) => f.path === request.startFile);
      if (start >= 0) current = start;
      current = Math.min(current, Math.max(0, files.length - 1));
    } catch (err) {
      error = String(err);
    } finally {
      loading = false;
    }
  }
  $effect(() => {
    void request;
    load();
  });

  // ------------------------------------------------------------ remembered per viewer
  const store = {
    get(key: string) {
      try {
        return localStorage.getItem(key);
      } catch {
        return null;
      }
    },
    set(key: string, value: string) {
      try {
        localStorage.setItem(key, value);
      } catch {
        // Not remembered: fine
      }
    },
  };

  let split = $state(store.get("thumbdeck:review:mode") !== "unified");
  $effect(() => store.set("thumbdeck:review:mode", split ? "split" : "unified"));

  /** path -> the fingerprint of its changes when you marked it viewed */
  let viewed = $state<Record<string, string>>({});
  $effect(() => {
    try {
      viewed = JSON.parse(store.get(`thumbdeck:viewed:${request.viewedKey}`) ?? "{}");
    } catch {
      viewed = {};
    }
  });
  const isViewed = (f: DiffFile) => viewed[f.path] === f.hash;
  const viewedCount = $derived(files.filter(isViewed).length);

  function toggleViewed(f: DiffFile | undefined, next = true) {
    if (!f) return;
    const v = { ...viewed };
    if (isViewed(f)) delete v[f.path];
    else v[f.path] = f.hash;
    viewed = v;
    store.set(`thumbdeck:viewed:${request.viewedKey}`, JSON.stringify(v));
    // Marked viewed: on to the next one still to look at
    if (next && isViewed(f)) {
      const after = files.findIndex((x, i) => i > current && !isViewed(x));
      const any = after >= 0 ? after : files.findIndex((x) => !isViewed(x));
      if (any >= 0) showFile(any);
    }
  }

  // ------------------------------------------------------------ the shown file
  const file = $derived<DiffFile | undefined>(files[current]);
  const lang = $derived(file ? languageFor(file.path) : null);

  /** Word changes of each removed line and the added line that replaced it */
  const words = $derived.by(() => {
    const map = new Map<Line, Segment[]>();
    if (!file) return map;
    for (const r of splitRows(file)) {
      if (r.left?.kind === "del" && r.right?.kind === "add") {
        const d = wordDiff(r.left.text, r.right.text);
        if (d) map.set(r.left, d.before), map.set(r.right, d.after);
      }
    }
    return map;
  });

  const rows = $derived<Row[]>(file ? (split ? splitRows(file) : unifiedRows(file)) : []);
  const html = (l: Line | null) => (l ? lineHtml(l.text, lang, words.get(l) ?? null) : "");

  let diffEl = $state<HTMLElement | null>(null);

  async function showFile(i: number) {
    current = Math.max(0, Math.min(files.length - 1, i));
    await tick();
    diffEl?.scrollTo({ top: 0 });
    document.getElementById(`rv-file-${current}`)?.scrollIntoView({ block: "nearest" });
  }

  /** Scroll to the next (or previous) block of changes; past the last one, the next file */
  function jumpChange(dir: 1 | -1) {
    if (!diffEl) return;
    const top = diffEl.scrollTop + 8;
    const starts = [...diffEl.querySelectorAll<HTMLElement>("[data-change]")].map((el) => el.offsetTop - 40);
    const target = dir > 0 ? starts.find((t) => t > top) : starts.toReversed().find((t) => t < top - 16);
    if (target !== undefined) diffEl.scrollTo({ top: Math.max(0, target) });
    else if (dir > 0 && current < files.length - 1) showFile(current + 1);
    else if (dir < 0 && current > 0) showFile(current - 1);
  }

  let help = $state(false);

  export function handleKey(e: KeyboardEvent): boolean {
    if (help) {
      help = false; // any key closes the help
      return true;
    }
    if (e.ctrlKey || e.metaKey || e.altKey) return false;
    const page = (diffEl?.clientHeight ?? 400) * 0.8;
    switch (e.key) {
      case "j": case "ArrowDown": diffEl?.scrollBy({ top: 54 }); break;
      case "k": case "ArrowUp": diffEl?.scrollBy({ top: -54 }); break;
      case "d": case "PageDown": case " ": diffEl?.scrollBy({ top: page }); break;
      case "u": case "PageUp": diffEl?.scrollBy({ top: -page }); break;
      case "g": diffEl?.scrollTo({ top: 0 }); break;
      case "G": diffEl?.scrollTo({ top: diffEl.scrollHeight }); break;
      case "]": case "c": jumpChange(1); break;
      case "[": case "C": jumpChange(-1); break;
      case "n": case "J": case "ArrowRight": showFile(current + 1); break;
      case "p": case "K": case "ArrowLeft": showFile(current - 1); break;
      case "v": toggleViewed(file); break;
      case "s": split = !split; break;
      case "r": load(); break;
      case "?": help = true; break;
      case "q": case "Escape": onClose(); break;
      default: return false;
    }
    return true;
  }

  const totals = $derived(files.reduce((t, f) => ({ add: t.add + f.additions, del: t.del + f.deletions }), { add: 0, del: 0 }));
  const statusLetter = { modified: "M", added: "A", deleted: "D", renamed: "R" } as const;
  const dirOf = (p: string) => (p.includes("/") ? p.slice(0, p.lastIndexOf("/") + 1) : "");
  const nameOf = (p: string) => p.slice(p.lastIndexOf("/") + 1);
</script>

<div class="review" role="dialog" aria-label="Review: {request.title}">
  <header class="top">
    <button class="back" title="Back (Esc)" onclick={onClose}>‹ back</button>
    <div class="what">
      <strong>{request.title}</strong>
      {#if request.subtitle}<span class="dim">{request.subtitle}</span>{/if}
    </div>
    <span class="spacer"></span>
    <span class="stat"><span class="plus">+{totals.add}</span> <span class="minus">−{totals.del}</span></span>
    <span class="dim">{files.length} file{files.length === 1 ? "" : "s"} · {viewedCount} viewed</span>
    <div class="modes">
      <button class:on={split} onclick={() => (split = true)}>Side by side</button>
      <button class:on={!split} onclick={() => (split = false)}>One column</button>
    </div>
    <button class="back" title="Keys (?)" onclick={() => (help = true)}>?</button>
  </header>

  <div class="body">
    <nav class="files">
      <div class="progress"><span style="width: {files.length ? (viewedCount / files.length) * 100 : 0}%"></span></div>
      {#each files as f, i}
        <button id="rv-file-{i}" class="file" class:on={i === current} class:seen={isViewed(f)} onclick={() => showFile(i)}>
          <span class="st {f.status}">{statusLetter[f.status]}</span>
          <span class="name"><span class="dir">{dirOf(f.path)}</span>{nameOf(f.path)}</span>
          <span class="counts"><span class="plus">{f.additions ? `+${f.additions}` : ""}</span> <span class="minus">{f.deletions ? `−${f.deletions}` : ""}</span></span>
          <span class="tick" title={isViewed(f) ? "viewed" : ""}>{isViewed(f) ? "✓" : ""}</span>
        </button>
      {/each}
    </nav>

    <section class="diff">
      {#if loading && !files.length}
        <p class="dim msg">Reading the changes…</p>
      {:else if error}
        <p class="msg err">{error}</p>
      {:else if !file}
        <p class="dim msg">No changes.</p>
      {:else}
        <header class="file-head">
          <span class="st {file.status}">{statusLetter[file.status]}</span>
          <strong>{file.path}</strong>
          {#if file.oldPath}<span class="dim">← {file.oldPath}</span>{/if}
          <span class="spacer"></span>
          <span class="dim">{current + 1} / {files.length}</span>
          <label class="viewed"><input type="checkbox" checked={isViewed(file)} onchange={() => toggleViewed(file, false)} /> Viewed <span class="dim">v</span></label>
        </header>
        <div class="scroll" bind:this={diffEl}>
          {#if file.binary}
            <p class="dim msg">A binary file: no text to compare.</p>
          {:else if !rows.length}
            <p class="dim msg">{file.status === "renamed" ? "Renamed, with no changes inside." : "No changes inside (e.g. only its mode changed)."}</p>
          {:else}
            <div class="grid" class:split>
              {#each rows as r}
                {#if r.header}
                  <div class="hunk">{r.header}</div>
                {:else if split}
                  <span class="n {r.left?.kind ?? 'empty'}" data-change={r.changeStart ? "" : undefined}>{r.left?.old ?? ""}</span>
                  <code class="{r.left?.kind ?? 'empty'}">{@html html(r.left)}</code>
                  <span class="n {r.right?.kind ?? 'empty'}">{r.right?.new ?? ""}</span>
                  <code class="{r.right?.kind ?? 'empty'}">{@html html(r.right)}</code>
                {:else}
                  {@const l = (r.left ?? r.right)!}
                  <span class="n {l.kind}" data-change={r.changeStart ? "" : undefined}>{l.old ?? ""}</span>
                  <span class="n {l.kind}">{l.new ?? ""}</span>
                  <code class={l.kind}><span class="sign">{l.kind === "add" ? "+" : l.kind === "del" ? "−" : " "}</span>{@html html(l)}</code>
                {/if}
              {/each}
            </div>
          {/if}
        </div>
      {/if}
    </section>
  </div>

  {#if help}
    <div class="overlay">
      <strong>Review: keys</strong> <span class="dim">(any key closes this) · read-only</span>
      <dl>
        <dt>n / p (→ / ←)</dt><dd>next / previous file</dd>
        <dt>] / [ (or c / C)</dt><dd>next / previous change (past the last: the next file)</dd>
        <dt>j / k, d / u, g / G</dt><dd>scroll a little / a page, top / bottom</dd>
        <dt>v</dt><dd>mark the file viewed and go to the next one (remembered until its changes change)</dd>
        <dt>s</dt><dd>side by side ↔ one column</dd>
        <dt>r</dt><dd>read the changes again</dd>
        <dt>q, Esc</dt><dd>back</dd>
      </dl>
    </div>
  {/if}
</div>

<style>
  .review { position: fixed; inset: 0; z-index: 50; display: flex; flex-direction: column; background: var(--bg0); font: 12.5px var(--mono);
            --add-bg: color-mix(in srgb, var(--green) 14%, transparent); --del-bg: color-mix(in srgb, var(--red) 14%, transparent);
            --add-mark: color-mix(in srgb, var(--green) 38%, transparent); --del-mark: color-mix(in srgb, var(--red) 38%, transparent); }
  .top { display: flex; align-items: center; gap: 14px; padding: 8px 14px; border-bottom: 1px solid var(--bg2); white-space: nowrap; }
  .what { display: flex; flex-direction: column; min-width: 0; }
  .what strong { overflow: hidden; text-overflow: ellipsis; font-size: 13.5px; }
  .what .dim { font-size: 11.5px; }
  .spacer { flex: 1; }
  .dim { color: var(--grey); }
  .err { color: var(--red); }
  .plus { color: var(--green); } .minus { color: var(--red); }
  .back { padding: 3px 10px; border-radius: 6px; color: var(--grey); font: 12.5px var(--mono); }
  .back:hover { background: var(--bg2); color: var(--fg); }
  .modes { display: flex; background: var(--bg1); border-radius: 8px; padding: 2px; }
  .modes button { padding: 3px 10px; border-radius: 6px; color: var(--grey); font: 12px var(--mono); }
  .modes button.on { background: var(--bg2); color: var(--fg); }

  .body { flex: 1; min-height: 0; display: grid; grid-template-columns: minmax(200px, 22%) 1fr; }
  .files { overflow: auto; border-right: 1px solid var(--bg2); padding: 6px; }
  .progress { height: 3px; margin: 2px 6px 8px; background: var(--bg2); border-radius: 2px; overflow: hidden; }
  .progress span { display: block; height: 100%; background: var(--green); transition: width 0.2s; }
  .file { width: 100%; display: flex; align-items: center; gap: 8px; padding: 4px 8px; border-radius: 6px; text-align: left; font: 12.5px var(--mono); }
  .file:hover { background: var(--bg1); }
  .file.on { background: var(--bg2); box-shadow: inset 2px 0 var(--orange); }
  .file.seen .name { color: var(--grey); }
  .name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .dir { color: var(--grey); }
  .counts { flex: none; font-size: 11px; }
  .tick { flex: none; width: 12px; color: var(--green); }
  .st { flex: none; width: 12px; text-align: center; font-weight: 700; }
  .st.modified { color: var(--blue); } .st.added { color: var(--green); } .st.deleted { color: var(--red); } .st.renamed { color: var(--purple); }

  .diff { min-width: 0; min-height: 0; display: flex; flex-direction: column; }
  .file-head { display: flex; align-items: center; gap: 10px; padding: 7px 14px; border-bottom: 1px solid var(--bg2); background: var(--bg1); white-space: nowrap; }
  .file-head strong { overflow: hidden; text-overflow: ellipsis; }
  .viewed { display: flex; align-items: center; gap: 6px; cursor: pointer; }
  .scroll { position: relative; flex: 1; min-height: 0; overflow: auto; } /* relative: rows measure from here */
  .msg { padding: 16px; }

  .grid { display: grid; grid-template-columns: 48px 48px 1fr; min-width: min-content; }
  .grid.split { grid-template-columns: 48px minmax(0, 1fr) 48px minmax(0, 1fr); }
  .hunk { grid-column: 1 / -1; padding: 4px 12px; margin-top: 4px; color: var(--aqua); background: color-mix(in srgb, var(--aqua) 8%, transparent); font-size: 11.5px; }
  .n { padding: 0 8px; text-align: right; color: var(--grey); font-size: 11.5px; user-select: none; line-height: 19px; }
  code { padding: 0 10px; white-space: pre-wrap; word-break: break-all; font: 12.5px var(--mono); line-height: 19px; tab-size: 4; }
  .split code { border-right: 1px solid var(--bg1); }
  .add { background: var(--add-bg); } .del { background: var(--del-bg); }
  .n.add { color: var(--green); } .n.del { color: var(--red); }
  .empty { background: repeating-linear-gradient(135deg, transparent 0 6px, var(--bg1) 6px 7px); }
  .sign { display: inline-block; width: 14px; color: var(--grey); user-select: none; }
  code.add .sign { color: var(--green); } code.del .sign { color: var(--red); }
  code.add :global(mark) { background: var(--add-mark); color: inherit; border-radius: 2px; }
  code.del :global(mark) { background: var(--del-mark); color: inherit; border-radius: 2px; }

  /* Syntax colours from the theme */
  code :global(.hljs-keyword), code :global(.hljs-selector-tag), code :global(.hljs-literal) { color: var(--red); }
  code :global(.hljs-string), code :global(.hljs-regexp), code :global(.hljs-addition) { color: var(--green); }
  code :global(.hljs-number), code :global(.hljs-symbol) { color: var(--purple); }
  code :global(.hljs-comment), code :global(.hljs-quote) { color: var(--grey); font-style: italic; }
  code :global(.hljs-title), code :global(.hljs-section), code :global(.hljs-name) { color: var(--green); font-weight: 600; }
  code :global(.hljs-built_in), code :global(.hljs-type), code :global(.hljs-class) { color: var(--yellow); }
  code :global(.hljs-attr), code :global(.hljs-attribute), code :global(.hljs-property), code :global(.hljs-variable) { color: var(--blue); }
  code :global(.hljs-meta), code :global(.hljs-tag), code :global(.hljs-params) { color: var(--aqua); }

  .overlay { position: absolute; inset: 0; background: var(--bg0); padding: 20px 26px; overflow: auto; z-index: 5; }
  .overlay dl { display: grid; grid-template-columns: max-content 1fr; gap: 5px 18px; }
  .overlay dt { color: var(--orange); } .overlay dd { margin: 0; }
</style>
