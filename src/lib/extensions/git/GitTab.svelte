<script lang="ts">
  // The Git tab: the project's repo at a glance, read-only. Three lists (a switches): the
  // uncommitted changes, recent commits, and branches with stashes; the highlighted one's diff
  // below. Keys come through handleKey() while the tab has the keyboard.
  import { invoke } from "@tauri-apps/api/core";
  import { tick } from "svelte";
  import type { TabProps } from "../index.ts";
  import type { Branches, Commit, Setup, Status } from "./types.ts";
  import { groupChanges, lineKind, refNames, stateWord, track, type FileRow } from "./format.ts";
  import { ago } from "../agents/format.ts";

  let { path, setup, active, say, onActivate, onRelease, onEditSetup, openReview }: Omit<TabProps, "setup"> & { setup: Setup } = $props();

  const views = ["changes", "commits", "branches"] as const;
  let view = $state<(typeof views)[number]>("changes");
  let cursor = $state(0);
  let error = $state("");

  let status = $state<Status | null>(null);
  let commits = $state<Commit[] | null>(null);
  let refs = $state<Branches | null>(null);

  type Row =
    | { kind: "file"; file: FileRow }
    | { kind: "commit"; commit: Commit }
    | { kind: "branch"; branch: Branches["branches"][number] }
    | { kind: "stash"; stash: Branches["stashes"][number] };

  const rows = $derived.by<Row[]>(() => {
    if (view === "changes") return groupChanges(status?.changes ?? []).map((file) => ({ kind: "file" as const, file }));
    if (view === "commits") return (commits ?? []).map((commit) => ({ kind: "commit" as const, commit }));
    return [
      ...(refs?.branches ?? []).map((branch) => ({ kind: "branch" as const, branch })),
      ...(refs?.stashes ?? []).map((stash) => ({ kind: "stash" as const, stash })),
    ];
  });
  const row = $derived<Row | undefined>(rows[cursor]);
  /** Which row that is: the same after a refresh rebuilds the list */
  const rowKey = $derived.by(() => {
    const r = row;
    if (!r) return "";
    if (r.kind === "file") return `${view}:${r.file.group}:${r.file.change.path}`;
    return `${view}:${r.kind === "commit" ? r.commit.hash : r.kind === "branch" ? r.branch.name : r.stash.name}`;
  });

  async function load(which = view) {
    try {
      if (which === "changes") status = await invoke<Status>("git_status", { path });
      else if (which === "commits") commits = await invoke<Commit[]>("git_log", { path, count: setup.commits });
      else refs = await invoke<Branches>("git_branches", { path });
      error = "";
      cursor = Math.min(cursor, Math.max(0, rows.length - 1));
    } catch (err) {
      error = String(err);
    }
  }

  // The branch line is always shown; the changes follow edits: look again every 3s
  $effect(() => {
    load("changes");
    const timer = setInterval(() => {
      if (!document.hidden) load("changes").then(() => {
        if (view === "changes") loadDetail(false);
      });
    }, 3000);
    return () => clearInterval(timer);
  });

  // ------------------------------------------------------------ the highlighted one's diff
  let detail = $state("");
  let detailEl = $state<HTMLElement | null>(null);
  let asked = 0; // only the latest answer is shown

  async function loadDetail(fromTop = true) {
    const r = row;
    const mine = ++asked;
    let text = "";
    try {
      if (!r) text = "";
      else if (r.kind === "file") {
        const f = r.file;
        text = await invoke<string>("git_diff", { path, file: f.change.path, staged: f.group === "staged", untracked: f.group === "untracked" });
        if (!text) text = f.change.from ? `Renamed from ${f.change.from}` : "(no changes to show)";
      } else if (r.kind === "commit") text = await invoke<string>("git_show", { path, hash: r.commit.hash });
      else if (r.kind === "stash") text = await invoke<string>("git_stash", { path, name: r.stash.name });
      else text = `${r.branch.name}${r.branch.upstream ? ` → ${r.branch.upstream} ${r.branch.track}` : " (no upstream)"}\n\nLast commit: ${r.branch.subject}\n${ago(r.branch.date)}`;
    } catch (err) {
      text = String(err);
    }
    if (mine !== asked) return;
    detail = text;
    if (fromTop) detailEl?.scrollTo({ top: 0 });
  }

  // A new highlighted row: its diff (a short wait, so holding j doesn't ask for every one)
  $effect(() => {
    void rowKey;
    const timer = setTimeout(() => loadDetail(), 80);
    return () => clearTimeout(timer);
  });

  function showView(to: (typeof views)[number]) {
    view = to;
    cursor = 0;
    load(to);
  }

  function move(to: number) {
    cursor = Math.max(0, Math.min(rows.length - 1, to));
    tick().then(() => document.getElementById(`git-row-${cursor}`)?.scrollIntoView({ block: "nearest" }));
  }

  /** The review page for the highlighted row: all changes (at its file), a commit, a stash */
  function reviewRow() {
    const r = row;
    const name = path.split("/").filter(Boolean).at(-1) ?? path;
    const load = (what: string) => () => invoke<string>("git_review", { path, what });
    if (view === "changes") {
      if (!status?.changes.length) return say("Nothing to review: the working tree is clean.");
      openReview({ title: "Uncommitted changes", subtitle: `${name} · ${status.branch.name ?? "detached HEAD"}`, load: load("changes"),
        viewedKey: `changes:${path}`, startFile: r?.kind === "file" ? r.file.change.path : undefined });
    } else if (r?.kind === "commit") {
      openReview({ title: `${r.commit.short} ${r.commit.subject}`, subtitle: `${r.commit.author} · ${ago(r.commit.date)} · ${name}`,
        load: load(r.commit.hash), viewedKey: `commit:${r.commit.hash}` });
    } else if (r?.kind === "stash") {
      openReview({ title: `${r.stash.name} ${r.stash.subject}`, subtitle: name, load: load(r.stash.name), viewedKey: `stash:${path}:${r.stash.subject}` });
    } else if (r?.kind === "branch") {
      say("A branch has no diff of its own: review its commits in Commits (a)");
    }
  }

  function scrollDetail(pages: number) {
    detailEl?.scrollBy({ top: pages * (detailEl.clientHeight * 0.8) });
  }

  let help = $state(false);

  export function handleKey(e: KeyboardEvent): boolean {
    if (help) {
      help = false; // any key closes the help
      return true;
    }
    if (e.ctrlKey || e.metaKey || e.altKey) return false;
    switch (e.key) {
      case "j": case "ArrowDown": move(cursor + 1); break;
      case "k": case "ArrowUp": move(cursor - 1); break;
      case "g": move(0); break;
      case "G": move(rows.length - 1); break;
      case "J": case "d": scrollDetail(1); break;
      case "K": case "u": scrollDetail(-1); break;
      case "a": showView(views[(views.indexOf(view) + 1) % views.length]); break;
      case "Enter": case "l": reviewRow(); break;
      case "r": load().then(() => loadDetail()); say("refreshed"); break;
      case "S": onEditSetup(); break;
      case "?": help = true; break;
      case "q": onRelease(); break;
      default: return false;
    }
    return true;
  }

  const b = $derived(status?.branch);
  const counts = $derived.by(() => {
    const g = groupChanges(status?.changes ?? []);
    return { staged: g.filter((r) => r.group === "staged").length, changes: g.filter((r) => r.group !== "staged").length };
  });
  const groupTitle: Record<string, string> = { staged: "Staged", changes: "Changes", untracked: "Untracked" };
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="git" class:active onmousedown={onActivate}>
  <header class="bar">
    {#if b}
      <strong class="branch">{b.name ?? "detached HEAD"}</strong>
      {#if b.upstream}
        <span class="dim">→ {b.upstream}</span>
        {#if b.gone}<span class="red">upstream gone</span>
        {:else}<span class="track">{track(b.ahead, b.behind) || "up to date"}</span>{/if}
      {:else if b.name}<span class="dim">no upstream</span>{/if}
      {#if status?.fetched}<span class="dim">· fetched {ago(new Date(status.fetched * 1000).toISOString())}</span>{/if}
    {/if}
    <span class="spacer"></span>
    {#each views as v}
      <button class="seg" class:on={view === v} onclick={() => showView(v)}>
        {v === "changes" ? "Changes" : v === "commits" ? "Commits" : "Branches"}
        {#if v === "changes" && status}<span class="dim">{counts.staged ? `${counts.staged}+` : ""}{counts.changes}</span>{/if}
      </button>
    {/each}
    <button class="tool" title="Set up this tab (S)" onclick={onEditSetup}>⚙</button>
  </header>
  {#if error}<p class="note-line err">{error}</p>{/if}

  <ul class="rows">
    {#each rows as r, i}
      {@const prev = rows[i - 1]}
      {#if r.kind === "file" && (prev?.kind !== "file" || prev.file.group !== r.file.group)}
        <li class="group">{groupTitle[r.file.group]}</li>
      {:else if r.kind === "stash" && prev?.kind !== "stash"}
        <li class="group">Stashes</li>
      {/if}
      <li id="git-row-{i}">
        <button class="row" class:cursor={i === cursor} onclick={() => (cursor = i)} ondblclick={reviewRow}>
          {#if r.kind === "file"}
            <span class="state s{r.file.state === "?" ? "N" : r.file.state}" title={stateWord[r.file.state] ?? r.file.state}>{r.file.state === "?" ? "+" : r.file.state}</span>
            <span class="title">{r.file.change.path}{#if r.file.change.from}<span class="dim"> ← {r.file.change.from}</span>{/if}</span>
          {:else if r.kind === "commit"}
            <span class="hash">{r.commit.short}</span>
            <span class="title">{r.commit.subject}
              {#each refNames(r.commit.refs) as name}<span class="ref">{name}</span>{/each}</span>
            <span class="meta">{r.commit.author}</span>
            <span class="meta">{ago(r.commit.date)}</span>
          {:else if r.kind === "branch"}
            <span class="state">{r.branch.current ? "●" : ""}</span>
            <span class="title" class:current={r.branch.current}>{r.branch.name} <span class="dim">{r.branch.subject}</span></span>
            <span class="meta">{r.branch.track.replace(/[[\]]/g, "")}</span>
            <span class="meta">{ago(r.branch.date)}</span>
          {:else}
            <span class="state dim">≡</span>
            <span class="title">{r.stash.name} <span class="dim">{r.stash.subject}</span></span>
            <span class="meta">{ago(r.stash.date)}</span>
          {/if}
        </button>
      </li>
    {:else}
      <li class="dim empty">
        {view === "changes" ? (status ? "Nothing to commit: the working tree is clean." : "Looking…")
          : view === "commits" ? (commits ? "No commits yet." : "Looking…") : refs ? "No branches yet." : "Looking…"}
      </li>
    {/each}
  </ul>

  {#if row}
    <pre class="detail" bind:this={detailEl}>{#each detail.split("\n") as line}<span class={lineKind(line)}>{line}</span>
{/each}</pre>
  {/if}

  {#if help}
    <div class="overlay">
      <strong>Git: keys</strong> <span class="dim">(any key closes this) · it only looks, it never changes the repo</span>
      <dl>
        <dt>a</dt><dd>next list: changes → commits → branches and stashes</dd>
        <dt>j / k, g / G</dt><dd>down / up, first / last</dd>
        <dt>Enter, l</dt><dd>review: all changes (at this file), the commit or the stash, side by side over the whole window</dd>
        <dt>J / K, d / u</dt><dd>scroll the diff down / up</dd>
        <dt>r</dt><dd>refresh (changes refresh every 3s on their own)</dd>
        <dt>S</dt><dd>set up this tab</dd>
        <dt>q, Esc</dt><dd>give the keyboard back to thumbdeck</dd>
      </dl>
    </div>
  {/if}
</div>

<style>
  .git { position: relative; flex: 1; min-height: 0; display: flex; flex-direction: column; font: 12.5px var(--mono);
         border-top: 2px solid transparent; }
  .git.active { border-top-color: var(--orange); }
  .bar { display: flex; align-items: center; gap: 10px; padding: 6px 12px; border-bottom: 1px solid var(--bg2); white-space: nowrap; overflow: hidden; }
  .spacer { flex: 1; }
  .dim { color: var(--grey); }
  .err, .red { color: var(--red); }
  .branch { color: var(--green); }
  .track { color: var(--yellow); }
  .tool, .seg { padding: 2px 8px; border-radius: 6px; color: var(--grey); font: 12px var(--mono); }
  .tool:hover, .seg:hover, .seg.on { background: var(--bg2); color: var(--fg); }
  .note-line { margin: 6px 12px; }

  .rows { list-style: none; margin: 0; padding: 4px 6px; overflow: auto; flex: 0 1 auto; max-height: 40%; min-height: 60px; }
  .group { padding: 6px 8px 2px; color: var(--grey); font-size: 11px; }
  .row { width: 100%; display: flex; gap: 10px; align-items: center; padding: 2px 8px; border-radius: 6px; text-align: left;
         font: 12.5px var(--mono); white-space: nowrap; }
  .row.cursor { background: var(--bg2); }
  .git.active .row.cursor { box-shadow: inset 2px 0 var(--orange); }
  .title { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; }
  .title.current { color: var(--green); }
  .meta { flex: none; color: var(--grey); font-size: 11.5px; }
  .hash { flex: none; color: var(--yellow); }
  .ref { margin-left: 8px; padding: 0 6px; border-radius: 8px; background: var(--bg2); color: var(--aqua); font-size: 11px; }
  .state { flex: none; width: 12px; text-align: center; font-weight: 600; }
  .sM { color: var(--blue); } .sA, .sN { color: var(--green); } .sD { color: var(--red); }
  .sR, .sC { color: var(--purple); } .sU { color: var(--orange); }
  .empty { padding: 10px; }

  .detail { flex: 1; min-height: 0; overflow: auto; margin: 0; padding: 8px 14px; border-top: 1px solid var(--bg2);
            font: 12px var(--mono); white-space: pre; tab-size: 4; }
  .detail .add { color: var(--green); }
  .detail .del { color: var(--red); }
  .detail .hunk { color: var(--aqua); }
  .detail .meta { color: var(--grey); font-size: 12px; }

  .overlay { position: absolute; inset: 0; background: var(--bg0); padding: 16px 20px; overflow: auto; z-index: 5; }
  .overlay dl { display: grid; grid-template-columns: max-content 1fr; gap: 4px 16px; }
  .overlay dt { color: var(--orange); } .overlay dd { margin: 0; }
</style>
