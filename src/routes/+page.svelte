<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { marked } from "marked";
  import { onMount, tick } from "svelte";
  import { ask, open } from "@tauri-apps/plugin-dialog";
  import { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";
  import type { Action, CustomAction, Details, Project, ProjectList, Run } from "$lib/types";

  let projects = $state<Project[]>([]);
  let roots = $state<string[]>([]);
  // Folded sections of the tree (not saved: each start folds all but the one in use)
  let collapsed = $state<string[]>([]);
  let addMenu = $state(false);
  // Form for adding / editing a custom action (null when closed)
  let form = $state<CustomAction | null>(null);
  let showHiddenActions = $state(false);
  // Short message at the bottom (e.g. after opening a project in tmux)
  let toast = $state<{ text: string; error: boolean } | null>(null);
  let toastTimer: ReturnType<typeof setTimeout> | undefined;
  function say(text: string, error = false) {
    toast = { text, error };
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toast = null), 3500);
  }

  async function openInTmux() {
    if (!selected) return;
    try {
      say(await invoke<string>("open_in_tmux", { path: selected.path, name: selected.name }));
    } catch (err) {
      say(String(err), true);
    }
  }
  let addMenuEl = $state<HTMLElement | null>(null);

  // Close the + menu on a click anywhere else, or on Esc
  function closeMenuOutside(e: MouseEvent) {
    if (addMenu && addMenuEl && !addMenuEl.contains(e.target as Node)) addMenu = false;
  }
  // ------------------------------------------------------------ keyboard
  let hints = $state(false); // Space pressed: Toolkit buttons show their letters
  let helpOpen = $state(false);
  let pendingG = false; // first g of gg
  let filterEl = $state<HTMLInputElement | null>(null);
  const HINT_KEYS = "asdfghjkl;qwertyuiopzxcvbnm";

  // Projects in the order the tree shows them (a pinned project counts once, where it's pinned)
  const navOrder = $derived.by(() => {
    const seen = new Set<string>();
    const out: Project[] = [];
    for (const sec of sections) {
      if (sec.id === "hidden") continue;
      for (const p of sec.items) if (!seen.has(p.path)) (seen.add(p.path), out.push(p));
    }
    return out;
  });

  function move(delta: number | "first" | "last") {
    if (!navOrder.length) return;
    const i = navOrder.findIndex((p) => p.path === selected?.path);
    const next =
      delta === "first" ? 0 : delta === "last" ? navOrder.length - 1
      : Math.min(navOrder.length - 1, Math.max(0, (i < 0 ? 0 : i) + delta));
    const p = navOrder[next];
    collapsed = collapsed.filter((id) => id !== sectionOf(p)); // unfold its section
    select(p);
  }

  function cycleRuns(delta: number) {
    const list = projectRuns.toReversed(); // oldest first
    if (!list.length) return;
    const i = list.findIndex((r) => r.id === shownRun);
    shownRun = list[(i < 0 ? list.length - 1 : i + delta + list.length) % list.length].id;
  }

  function onKey(e: KeyboardEvent) {
    const typing = e.target instanceof HTMLElement && e.target.closest("input, textarea, select");
    if (e.key === "Escape") {
      addMenu = false;
      form = null;
      hints = false;
      helpOpen = false;
      if (typing) (e.target as HTMLElement).blur();
      return;
    }
    if (typing || form || e.ctrlKey || e.metaKey || e.altKey) return;

    if (hints) {
      e.preventDefault();
      hints = false;
      const i = HINT_KEYS.indexOf(e.key);
      const a = details?.actions[i];
      if (i >= 0 && a) run(a);
      return;
    }
    const wasG = pendingG;
    pendingG = false;
    switch (e.key) {
      case " ": hints = (details?.actions.length ?? 0) > 0; break;
      case "j": move(1); break;
      case "k": move(-1); break;
      case "g": if (wasG) move("first"); else pendingG = true; break;
      case "G": move("last"); break;
      case "/": filterEl?.focus(); break;
      case "p": if (selected) edit("pin", selected.path); break;
      case "x": if (selected) edit("remove", selected.path); break;
      case "a": if (selected) openForm(); break;
      case "s": if (current && current.endedAt === null) stop(current); break;
      case "o": shownRun = shownRun === null ? (projectRuns[0]?.id ?? null) : null; break;
      case "[": cycleRuns(-1); break;
      case "]": cycleRuns(1); break;
      case "?": helpOpen = !helpOpen; break;
      case "Enter": openInTmux(); break;
      default: return;
    }
    e.preventDefault();
  }

  // Enter in the filter opens the first match
  function filterKey(e: KeyboardEvent) {
    if (e.key === "Enter" && navOrder.length) {
      select(navOrder[0]);
      filter = "";
      (e.target as HTMLElement).blur();
    }
  }

  let filter = $state("");
  let selected = $state<Project | null>(null);
  let details = $state<Details | null>(null);
  let runs = $state<Run[]>([]);
  let shownRun = $state<number | null>(null); // run whose output is in the center; null = README
  let now = $state(Date.now());
  let outputEl = $state<HTMLElement | null>(null);
  // macOS shows its window buttons over the top-left of the app (overlay title bar)
  const mac = navigator.userAgent.includes("Mac");

  const visible = $derived(
    projects.filter((p) => !p.hidden && p.name.toLowerCase().includes(filter.toLowerCase())),
  );
  // The project tree: Pinned, one section per scan folder, Added, Hidden
  const sections = $derived.by(() => {
    const match = (p: Project) => p.name.toLowerCase().includes(filter.toLowerCase());
    const shown = projects.filter((p) => !p.hidden && match(p));
    const list = [
      { id: "pinned", label: "★ Pinned", root: null as string | null, items: shown.filter((p) => p.pinned) },
      ...roots.map((r) => ({ id: r, label: home(r), root: r as string | null, items: shown.filter((p) => p.root === r) })),
      { id: "added", label: "Added", root: null, items: shown.filter((p) => p.root === null) },
      { id: "hidden", label: "Hidden", root: null, items: projects.filter((p) => p.hidden && match(p)) },
    ];
    // Scan folders always show (so they can be removed); the others only when they have projects
    return list.filter((sec) => sec.items.length > 0 || sec.root !== null);
  });
  const folded = (id: string) => collapsed.includes(id) && !filter;

  const icons: Record<string, [string, string]> = {
    django: ["", "green"], python: ["", "yellow"], android: ["", "aqua"],
    node: ["", "green"], tauri: ["", "orange"], rust: ["", "orange"],
    go: ["", "blue"], nvim: ["", "green"], folder: ["", "grey"],
  };

  const home = (path: string) => path.replace(/^\/(home|Users)\/[^/]+/, "~");

  // Section of the tree a project sits in
  const sectionOf = (p: Project) => p.root ?? "added";

  function applyList(list: ProjectList, starting = false) {
    projects = list.projects;
    roots = list.roots;
    const shown = projects.filter((p) => !p.hidden);
    // Keep the selection if it's still visible; on start, reopen the last project used
    let next = selected && shown.find((p) => p.path === selected!.path);
    if (starting) {
      next = shown.find((p) => p.path === list.last) ?? shown.find((p) => p.pinned) ?? shown[0];
      // Fold every section except Pinned and the one the project is in
      const inUse = next ? sectionOf(next) : null;
      collapsed = [...roots, "added", "hidden"].filter((id) => id !== inUse);
    }
    if (next) {
      if (next.path !== selected?.path) select(next);
    } else {
      selected = null;
    }
  }

  function toggleFold(id: string) {
    collapsed = collapsed.includes(id) ? collapsed.filter((c) => c !== id) : [...collapsed, id];
  }

  async function edit(change: string, path: string) {
    applyList(await invoke<ProjectList>("edit_projects", { change, path }));
  }

  async function pickFolder(change: "add" | "add-root") {
    addMenu = false;
    const title = change === "add" ? "Add a project folder" : "Add a folder to scan for projects";
    const path = await open({ directory: true, title });
    if (typeof path === "string") await edit(change, path);
  }
  const projectRuns = $derived(runs.filter((r) => r.projectPath === selected?.path).toReversed());
  const current = $derived(runs.find((r) => r.id === shownRun) ?? null);
  // Toolkit grouped by where each action came from (npm, django, compose, ...)
  const groups = $derived.by(() => {
    const byGroup = new Map<string, Action[]>();
    for (const a of details?.actions ?? []) byGroup.set(a.source, [...(byGroup.get(a.source) ?? []), a]);
    return [...byGroup.entries()];
  });

  async function select(p: Project) {
    selected = p;
    invoke("edit_projects", { change: "last", path: p.path }); // remembered for the next start
    shownRun = null;
    details = null;
    details = await invoke<Details>("project_details", { path: p.path });
  }

  async function editActions(change: string, id = "", action: CustomAction | null = null) {
    if (!selected) return;
    details = await invoke<Details>("edit_actions", { path: selected.path, change, id, action });
  }

  function openForm(a?: Action) {
    form = a
      ? { id: a.id.replace(/^custom:/, ""), name: a.label, command: a.command, confirm: a.confirm }
      : { id: "", name: "", command: "", confirm: false };
  }

  async function saveForm() {
    if (!form || !form.name.trim() || !form.command.trim()) return;
    await editActions("save", "", { ...form, name: form.name.trim(), command: form.command.trim() });
    form = null;
  }

  async function run(a: Action) {
    if (!selected) return;
    if (a.confirm && !(await ask(`${a.command}`, { title: `Run "${a.label}"?`, kind: "warning", okLabel: "Run" }))) return;
    const id = await invoke<number>("run_action", { path: selected.path, command: a.command });
    runs.push({
      id, projectPath: selected.path, label: a.label, source: a.source, command: a.command,
      startedAt: Date.now(), endedAt: null, code: null, lines: [],
    });
    shownRun = id;
  }

  function stop(r: Run) {
    invoke("stop_run", { id: r.id });
  }

  function elapsed(r: Run) {
    const s = Math.max(0, Math.floor(((r.endedAt ?? now) - r.startedAt) / 1000));
    return s < 60 ? `${s}s` : `${Math.floor(s / 60)}m ${s % 60}s`;
  }

  // Tell the system when a command finishes while you're looking at another window
  async function notifyFinished(r: Run) {
    // Ask the window itself: the web view isn't always told when the window loses focus
    if (await getCurrentWindow().isFocused()) return;
    let allowed = await isPermissionGranted();
    if (!allowed) allowed = (await requestPermission()) === "granted";
    if (!allowed) return;
    const project = projects.find((p) => p.path === r.projectPath)?.name ?? "";
    const title = r.code === 0 ? `✓ ${r.label} finished` : `✗ ${r.label} failed (exit ${r.code})`;
    sendNotification({ title, body: `${r.code === 0 ? "in" : "after"} ${elapsed(r)} · ${project}` });
  }

  function status(r: Run) {
    if (r.endedAt === null) return "running";
    return r.code === 0 ? "done" : "failed";
  }

  onMount(() => {
    invoke<ProjectList>("list_projects").then((list) => applyList(list, true));
    const unlistenOut = listen<{ id: number; line: string; stderr: boolean }>("run-output", async (e) => {
      const r = runs.find((x) => x.id === e.payload.id);
      if (!r) return;
      r.lines.push({ text: e.payload.line, stderr: e.payload.stderr });
      if (r.id === shownRun) {
        await tick();
        outputEl?.scrollTo({ top: outputEl.scrollHeight });
      }
    });
    const unlistenExit = listen<{ id: number; code: number | null }>("run-exit", (e) => {
      const r = runs.find((x) => x.id === e.payload.id);
      if (r) {
        r.endedAt = Date.now();
        r.code = e.payload.code ?? -1;
        notifyFinished(r);
      }
    });
    const timer = setInterval(() => (now = Date.now()), 1000);
    return () => {
      unlistenOut.then((f) => f());
      unlistenExit.then((f) => f());
      clearInterval(timer);
    };
  });
</script>

<svelte:window onclick={closeMenuOutside} onkeydown={onKey} />
<div class="drag" data-tauri-drag-region></div>
<main class:mac>
  <!-- ------------------------------------------------------------ projects -->
  <aside class="panel left">
    <header class="app" data-tauri-drag-region>thumbdeck</header>
    <input class="filter" placeholder="Filter projects…   /" bind:value={filter} bind:this={filterEl} onkeydown={filterKey} />
    <section class="card helm">
      <h2>
        <span class="dot purple"></span>Projects <span class="count">{visible.length}</span>
        <span class="menu-anchor" bind:this={addMenuEl}>
          <button class="icon" title="Add…" onclick={() => (addMenu = !addMenu)}>+</button>
          {#if addMenu}
            <div class="menu">
              <button onclick={() => pickFolder("add")}>Add project…</button>
              <button onclick={() => pickFolder("add-root")}>Add folder to scan…</button>
            </div>
          {/if}
        </span>
      </h2>
      {#each sections as sec (sec.id)}
        <div class="section">
          <div class="section-head">
            <button class="fold" onclick={() => toggleFold(sec.id)}>
              <span class="caret">{folded(sec.id) ? "▸" : "▾"}</span>{sec.label}
              <span class="n">{sec.items.length}</span>
            </button>
            {#if sec.root}
              <button class="icon hide" title="Stop scanning this folder" onclick={() => edit("remove-root", sec.root!)}>×</button>
            {/if}
          </div>
          {#if !folded(sec.id)}
            <ul>
              {#each sec.items as p (p.path)}
                {@const [glyph, color] = icons[p.kind] ?? icons.folder}
                <li class="project" class:muted={p.hidden}>
                  <button class="pick" class:active={selected?.path === p.path && !p.hidden}
                          disabled={p.hidden} onclick={() => select(p)}>
                    <span class="kind" style="color: var(--{color})">{glyph}</span>
                    <span class="name">{p.name}</span>
                    {#if p.dirty}<span class="changes" title="uncommitted changes"></span>{/if}
                  </button>
                  {#if p.hidden}
                    <button class="icon" title="Show again" onclick={() => edit("unhide", p.path)}>↺</button>
                  {:else}
                    <button class="icon hide star" class:on={p.pinned} title={p.pinned ? "Unpin" : "Pin"}
                            onclick={() => edit("pin", p.path)}>{p.pinned ? "★" : "☆"}</button>
                    <button class="icon hide" title={p.root === null ? "Remove from the list" : "Hide"}
                            onclick={() => edit("remove", p.path)}>×</button>
                  {/if}
                </li>
              {/each}
            </ul>
          {/if}
        </div>
      {/each}
    </section>
  </aside>

  <!-- ------------------------------------------------------------ center -->
  <section class="panel center">
    {#if selected}
      <nav class="crumbs">
        <span>{home(selected.path).split("/").slice(0, -1).join(" / ")}</span>
        <span> / </span><strong>{selected.name}</strong>
        {#if selected.branch}<span class="branch">  {selected.branch}</span>{/if}
        <span class="spacer"></span>
        <button class="tab open" title="Open in tmux (Enter)" onclick={openInTmux}> Open in tmux</button>
        <button class="tab" class:on={shownRun === null} onclick={() => (shownRun = null)}>README</button>
        {#if current}
          <button class="tab on">{current.label}</button>
        {/if}
      </nav>
      {#if current}
        <pre class="output" bind:this={outputEl}><span class="cmd">$ {current.command}</span>
{#each current.lines as l}<span class:err={l.stderr}>{l.text}</span>
{/each}{#if current.endedAt !== null}<span class={current.code === 0 ? "ok" : "err"}>{current.code === 0 ? "✓ finished" : `✗ exited with ${current.code}`} after {elapsed(current)}</span>{/if}</pre>
      {:else if details?.readme}
        <article class="readme">{@html marked.parse(details.readme)}</article>
      {:else}
        <p class="empty">{details ? "No README in this project." : "Loading…"}</p>
      {/if}
    {:else}
      <p class="empty">No projects found in ~/dev, ~/projects or your home folder.</p>
    {/if}
  </section>

  <!-- ------------------------------------------------------------ running + toolkit -->
  <aside class="panel right">
    <section class="card running">
      <h2><span class="dot green"></span>Running</h2>
      {#if projectRuns.length === 0}
        <p class="hint">Nothing yet: pick something from the toolkit.</p>
      {/if}
      {#each projectRuns as r (r.id)}
        <div class="run" class:shown={r.id === shownRun} role="button" tabindex="0"
             onclick={() => (shownRun = r.id)} onkeydown={(e) => e.key === "Enter" && (shownRun = r.id)}>
          <span class="avatar {r.source}">{r.source.slice(0, 2)}</span>
          <span class="what">
            <span class="label">{r.label}</span>
            <span class="pill {status(r)}">{status(r)}</span>
          </span>
          <span class="time">{elapsed(r)}</span>
          {#if r.endedAt === null}
            <button class="stop" title="Stop" onclick={(e) => { e.stopPropagation(); stop(r); }}>■</button>
          {/if}
        </div>
      {/each}
    </section>

    <section class="card toolkit">
      <h2>
        <span class="dot orange"></span>Toolkit {#if selected}<span class="for">{selected.name}</span>{/if}
        {#if selected}<button class="icon" title="Add your own action" onclick={() => openForm()}>+</button>{/if}
      </h2>
      {#if details && groups.length === 0}
        <p class="hint">Nothing detected here. Add your own action with +.</p>
      {/if}
      {#each groups as [source, actions] (source)}
        <h3>{source === "custom" ? "yours" : source}</h3>
        <div class="grid">
          {#each actions as a (a.id)}
            {@const hint = HINT_KEYS[details?.actions.indexOf(a) ?? -1]}
            <div class="action-wrap">
              <button class="action" class:custom={a.source === "custom"} title={a.command} onclick={() => run(a)}>
                {#if hints && hint}
                  <span class="hint-key">{hint}</span>
                {:else}
                  <span class="play">{a.confirm ? "!" : "▷"}</span>
                {/if}
                <span class="alabel">{a.label}</span>
              </button>
              <span class="action-tools">
                {#if a.source === "custom"}
                  <button class="icon" title="Edit" onclick={() => openForm(a)}>✎</button>
                  <button class="icon" title="Delete" onclick={() => editActions("delete", a.id)}>×</button>
                {:else}
                  <button class="icon" title="Hide from this project" onclick={() => editActions("hide", a.id)}>×</button>
                {/if}
              </span>
            </div>
          {/each}
        </div>
      {/each}
      {#if details?.hidden.length}
        <button class="fold hidden-actions" onclick={() => (showHiddenActions = !showHiddenActions)}>
          <span class="caret">{showHiddenActions ? "▾" : "▸"}</span>hidden ({details.hidden.length})
        </button>
        {#if showHiddenActions}
          <ul class="hidden-list">
            {#each details.hidden as a (a.id)}
              <li class="project muted">
                <span class="name">{a.source} · {a.label}</span>
                <button class="icon" title="Show again" onclick={() => editActions("unhide", a.id)}>↺</button>
              </li>
            {/each}
          </ul>
        {/if}
      {/if}
    </section>
  </aside>
</main>

{#if toast}
  <div class="toast" class:error={toast.error}>{toast.text}</div>
{/if}

{#if helpOpen}
  <div class="backdrop" role="presentation" onclick={() => (helpOpen = false)}>
    <div class="dialog help">
      <h2><span class="dot purple"></span>Keys <span class="for">? or Esc to close</span></h2>
      <dl>
        <dt>j / k</dt><dd>next / previous project</dd>
        <dt>Enter</dt><dd>open the project in tmux (its own session: Neovim + a shell)</dd>
        <dt>gg / G</dt><dd>first / last project</dd>
        <dt>/</dt><dd>filter projects (Enter opens the first match)</dd>
        <dt>p</dt><dd>pin / unpin project</dd>
        <dt>x</dt><dd>hide project</dd>
        <dt>Space, letter</dt><dd>run a Toolkit button</dd>
        <dt>a</dt><dd>add your own action</dd>
        <dt>s</dt><dd>stop the command shown</dd>
        <dt>o</dt><dd>README ↔ output</dd>
        <dt>[ / ]</dt><dd>previous / next run's output</dd>
        <dt>Esc</dt><dd>close menus, forms, this help; leave the filter</dd>
      </dl>
    </div>
  </div>
{/if}

{#if form && selected}
  <div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && (form = null)}>
    <form class="dialog" onsubmit={(e) => { e.preventDefault(); saveForm(); }}>
      <h2><span class="dot orange"></span>{form.id ? "Edit action" : "New action"} <span class="for">{selected.name}</span></h2>
      <label>Name <input bind:value={form.name} placeholder="Backup DB" {@attach (el) => el.focus()} /></label>
      <label>Command
        <textarea bind:value={form.command} rows="3" placeholder="./scripts/backup.sh && ls backups/"
                  onkeydown={(e) => e.key === "Enter" && (e.ctrlKey || e.metaKey) && saveForm()}></textarea>
      </label>
      <p class="hint">Runs in {home(selected.path)} with your shell's environment.</p>
      <label class="check"><input type="checkbox" bind:checked={form.confirm} /> Ask before running</label>
      <div class="buttons">
        <button type="button" class="ghost" onclick={() => (form = null)}>Cancel</button>
        <button type="submit" class="primary" disabled={!form.name.trim() || !form.command.trim()}>Save</button>
      </div>
    </form>
  </div>
{/if}

<style>
  :global(:root) {
    /* Everforest dark hard */
    --bg-dim: #1e2326; --bg0: #272e33; --bg1: #2e383c; --bg2: #374145; --bg3: #414b50;
    --fg: #d3c6aa; --grey: #859289; --grey-dim: #7a8478;
    --red: #e67e80; --orange: #e69875; --yellow: #dbbc7f; --green: #a7c080;
    --aqua: #83c092; --blue: #7fbbb3; --purple: #d699b6;
    --mono: "Iosevka Nerd Font", "JetBrains Mono", ui-monospace, monospace;
    --sans: "Inter", ui-sans-serif, system-ui, sans-serif;
    color-scheme: dark;
  }
  :global(html, body) { margin: 0; height: 100%; background: var(--bg-dim); color: var(--fg); font: 14px/1.5 var(--sans); }
  :global(*) { box-sizing: border-box; }
  button { font: inherit; color: inherit; background: none; border: 0; cursor: pointer; }

  /* invisible strip along the top to move the window by (macOS has no title bar here) */
  .drag { position: fixed; inset: 0 0 auto 0; height: 8px; z-index: 10; }
  main.mac .app { padding-top: 26px; }
  main { display: grid; grid-template-columns: 250px 1fr 340px; gap: 8px; height: 100vh; padding: 8px; }
  .panel { min-height: 0; display: flex; flex-direction: column; gap: 8px; }
  .card { background: var(--bg0); border: 1px solid var(--bg2); border-radius: 12px; padding: 12px; }
  h2 { display: flex; align-items: center; gap: 8px; margin: 0 0 10px; font: 600 13px var(--mono); }
  h3 { margin: 10px 0 6px; font: 500 11px var(--mono); color: var(--grey); text-transform: uppercase; letter-spacing: 0.08em; }
  .dot { width: 8px; height: 8px; border-radius: 50%; }
  .dot.purple { background: var(--purple); } .dot.green { background: var(--green); } .dot.orange { background: var(--orange); }
  .count, .for { margin-left: auto; color: var(--grey); font-weight: 400; }
  .hint, .empty { color: var(--grey); font-size: 13px; }
  .branch { color: var(--purple); font: 12px var(--mono); }

  /* left */
  .app { font: 700 16px var(--mono); padding: 4px 6px; }
  .filter { width: 100%; background: var(--bg0); border: 1px solid var(--bg2); border-radius: 8px; padding: 7px 10px; color: var(--fg); font: 13px var(--mono); outline: none; }
  .filter:focus { border-color: var(--purple); }
  .helm { flex: 1; overflow: auto; }
  .section { margin-top: 6px; }
  .section-head { display: flex; align-items: center; }
  .section-head:hover .hide { opacity: 1; }
  .fold { flex: 1; display: flex; align-items: center; gap: 6px; text-align: left; padding: 4px 6px; font: 600 11px var(--mono); color: var(--grey); letter-spacing: 0.03em; }
  .fold:hover { color: var(--fg); }
  .caret { width: 10px; }
  .n { margin-left: auto; font-weight: 400; opacity: 0.7; }
  .helm ul { list-style: none; margin: 2px 0 4px; padding: 0 0 0 10px; }
  .project { display: flex; align-items: center; gap: 2px; }
  .pick { flex: 1; min-width: 0; display: flex; align-items: center; gap: 8px; text-align: left; padding: 5px 8px; border-radius: 8px; }
  .pick:hover:not(:disabled) { background: var(--bg1); }
  .pick.active { background: var(--bg2); }
  .pick:disabled { cursor: default; }
  .kind { width: 16px; text-align: center; font-family: var(--mono); }
  .name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .muted { opacity: 0.55; }
  .icon { width: 22px; height: 22px; flex: none; border-radius: 6px; color: var(--grey); font: 13px var(--mono); }
  .icon:hover { background: var(--bg2); color: var(--fg); }
  .hide { opacity: 0; }
  .project:hover .hide, .star.on { opacity: 1; }
  .star.on { color: var(--yellow); }
  h2 .icon { margin-left: 6px; }
  .menu-anchor { position: relative; }
  .menu { position: absolute; right: 0; top: 26px; z-index: 20; min-width: 190px; padding: 4px; border-radius: 10px; background: var(--bg1); border: 1px solid var(--bg3); box-shadow: 0 8px 24px #0008; }
  .menu button { display: block; width: 100%; text-align: left; padding: 6px 10px; border-radius: 6px; font: 12.5px var(--sans); }
  .menu button:hover { background: var(--bg2); }

  .name { font: 13px var(--mono); }
  .changes { width: 6px; height: 6px; border-radius: 50%; background: var(--orange); }


  /* center */
  .center { background: var(--bg0); border: 1px solid var(--bg2); border-radius: 12px; overflow: hidden; }
  .crumbs { display: flex; align-items: center; gap: 4px; padding: 10px 14px; border-bottom: 1px solid var(--bg2); font: 12px var(--mono); color: var(--grey); }
  .crumbs strong { color: var(--fg); }
  .spacer { flex: 1; }
  .tab { padding: 3px 10px; border-radius: 6px; font: 12px var(--mono); color: var(--grey); }
  .tab.on { background: var(--bg2); color: var(--fg); }
  .readme { padding: 8px 28px 28px; overflow: auto; max-width: 860px; }
  .readme :global(h1), .readme :global(h2) { font-family: var(--mono); border-bottom: 1px solid var(--bg2); padding-bottom: 6px; }
  .readme :global(code) { font-family: var(--mono); background: var(--bg1); padding: 1px 5px; border-radius: 4px; color: var(--aqua); }
  .readme :global(pre) { background: var(--bg1); padding: 12px; border-radius: 8px; overflow: auto; }
  .readme :global(pre code) { background: none; padding: 0; color: var(--fg); }
  .readme :global(a) { color: var(--blue); }
  .output { flex: 1; margin: 0; padding: 14px; overflow: auto; font: 12.5px/1.45 var(--mono); white-space: pre-wrap; word-break: break-word; }
  .output .cmd { color: var(--green); }
  .output .err { color: var(--red); } .output .ok { color: var(--green); }
  .empty { padding: 20px; }

  /* right */
  .running { max-height: 45%; overflow: auto; }
  .run { display: flex; align-items: center; gap: 10px; padding: 8px; border-radius: 10px; background: var(--bg1); border: 1px solid transparent; margin-bottom: 6px; cursor: pointer; }
  .run.shown { border-color: var(--green); }
  .avatar { width: 30px; height: 30px; border-radius: 8px; display: grid; place-items: center; font: 700 11px var(--mono); text-transform: uppercase; color: var(--bg0); background: var(--grey); }
  .avatar.npm { background: var(--red); } .avatar.django { background: var(--green); } .avatar.pytest { background: var(--yellow); }
  .avatar.compose { background: var(--blue); } .avatar.gradle { background: var(--aqua); } .avatar.make { background: var(--purple); }
  .avatar.cargo { background: var(--orange); } .avatar.go { background: var(--blue); }
  .what { flex: 1; display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .label { font: 13px var(--mono); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .pill { align-self: flex-start; font: 10px var(--mono); padding: 1px 7px; border-radius: 99px; }
  .pill.running { color: var(--green); background: color-mix(in srgb, var(--green) 15%, transparent); }
  .pill.done { color: var(--grey); background: var(--bg2); }
  .pill.failed { color: var(--red); background: color-mix(in srgb, var(--red) 15%, transparent); }
  .time { font: 11px var(--mono); color: var(--grey); }
  .stop { color: var(--red); padding: 2px 6px; border-radius: 6px; }
  .stop:hover { background: var(--bg2); }

  .toolkit { flex: 1; overflow: auto; border-color: color-mix(in srgb, var(--orange) 35%, var(--bg2)); }
  .grid { display: grid; grid-template-columns: 1fr 1fr; gap: 6px; }
  .action-wrap { position: relative; }
  .action { width: 100%; display: flex; align-items: center; gap: 8px; text-align: left; padding: 8px 10px; border-radius: 8px; background: var(--bg1); border: 1px solid var(--bg2); font: 12.5px var(--mono); }
  .action.custom { border-color: color-mix(in srgb, var(--yellow) 30%, var(--bg2)); }
  .alabel { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .action-tools { position: absolute; right: 4px; top: 50%; transform: translateY(-50%); display: flex; opacity: 0; background: var(--bg1); border-radius: 6px; }
  .action-wrap:hover .action-tools { opacity: 1; }
  .hidden-actions { margin-top: 12px; }
  .hidden-list { list-style: none; margin: 0; padding: 0 0 0 10px; font: 12px var(--mono); }
  .avatar.custom { background: var(--yellow); }
  .tab.open { color: var(--green); }
  .tab.open:hover { background: var(--bg2); }
  .toast { position: fixed; left: 50%; bottom: 18px; transform: translateX(-50%); z-index: 60; padding: 8px 16px; border-radius: 10px; background: var(--bg1); border: 1px solid var(--green); font: 12.5px var(--mono); box-shadow: 0 8px 24px #0008; }
  .toast.error { border-color: var(--red); color: var(--red); }
  .hint-key { min-width: 16px; padding: 0 4px; border-radius: 4px; text-align: center; background: var(--orange); color: var(--bg0); font-weight: 700; }
  .hint-note { font: 400 11px var(--mono); color: var(--orange); }
  .help dl { display: grid; grid-template-columns: max-content 1fr; gap: 6px 16px; margin: 0; font-size: 13px; }
  .help dt { font-family: var(--mono); color: var(--orange); }
  .help dd { margin: 0; color: var(--fg); }

  .backdrop { position: fixed; inset: 0; z-index: 50; background: #0009; display: grid; place-items: center; }
  .dialog { width: min(520px, 90vw); background: var(--bg0); border: 1px solid var(--bg3); border-radius: 14px; padding: 18px; display: flex; flex-direction: column; gap: 12px; box-shadow: 0 20px 60px #000a; }
  .dialog label { display: flex; flex-direction: column; gap: 6px; font: 12px var(--mono); color: var(--grey); }
  .dialog input:not([type]), .dialog textarea { background: var(--bg1); border: 1px solid var(--bg2); border-radius: 8px; padding: 8px 10px; color: var(--fg); font: 13px var(--mono); outline: none; resize: vertical; }
  .dialog input:not([type]):focus, .dialog textarea:focus { border-color: var(--orange); }
  .dialog .check { flex-direction: row; align-items: center; gap: 8px; color: var(--fg); }
  .dialog .hint { margin: -6px 0 0; font-size: 12px; }
  .buttons { display: flex; justify-content: flex-end; gap: 8px; }
  .ghost, .primary { padding: 7px 16px; border-radius: 8px; font: 12.5px var(--mono); }
  .ghost { color: var(--grey); } .ghost:hover { background: var(--bg2); color: var(--fg); }
  .primary { background: var(--orange); color: var(--bg0); font-weight: 600; }
  .primary:disabled { opacity: 0.4; cursor: default; }
  .action:hover { border-color: var(--orange); }
  .play { color: var(--orange); }
</style>
