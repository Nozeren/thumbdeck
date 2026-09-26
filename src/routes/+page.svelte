<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { marked } from "marked";
  import { onMount, tick } from "svelte";
  import type { Action, Details, Project, Run } from "$lib/types";

  let projects = $state<Project[]>([]);
  let filter = $state("");
  let selected = $state<Project | null>(null);
  let details = $state<Details | null>(null);
  let runs = $state<Run[]>([]);
  let shownRun = $state<number | null>(null); // run whose output is in the center; null = README
  let now = $state(Date.now());
  let outputEl = $state<HTMLElement | null>(null);

  const visible = $derived(
    projects.filter((p) => p.name.toLowerCase().includes(filter.toLowerCase())),
  );
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
    shownRun = null;
    details = null;
    details = await invoke<Details>("project_details", { path: p.path });
  }

  async function run(a: Action) {
    if (!selected) return;
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

  function status(r: Run) {
    if (r.endedAt === null) return "running";
    return r.code === 0 ? "done" : "failed";
  }

  onMount(() => {
    invoke<Project[]>("list_projects").then((ps) => {
      projects = ps;
      if (ps.length) select(ps[0]);
    });
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

<main>
  <!-- ------------------------------------------------------------ projects -->
  <aside class="panel left">
    <header class="app">thumbdeck</header>
    <input class="filter" placeholder="Filter projects…" bind:value={filter} />
    <section class="card helm">
      <h2><span class="dot purple"></span>Projects <span class="count">{projects.length}</span></h2>
      <ul>
        {#each visible as p (p.path)}
          <li>
            <button class:active={selected?.path === p.path} onclick={() => select(p)}>
              <span class="name">{p.name}</span>
              {#if p.dirty}<span class="changes" title="uncommitted changes"></span>{/if}
              {#if p.branch}<span class="branch"> {p.branch}</span>{/if}
            </button>
          </li>
        {/each}
      </ul>
    </section>
  </aside>

  <!-- ------------------------------------------------------------ center -->
  <section class="panel center">
    {#if selected}
      <nav class="crumbs">
        <span>{selected.path.replace(/^\/(home|Users)\/[^/]+/, "~").split("/").slice(0, -1).join(" / ")}</span>
        <span> / </span><strong>{selected.name}</strong>
        {#if selected.branch}<span class="branch">  {selected.branch}</span>{/if}
        <span class="spacer"></span>
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
      <h2><span class="dot orange"></span>Toolkit {#if selected}<span class="for">{selected.name}</span>{/if}</h2>
      {#if details && groups.length === 0}
        <p class="hint">Nothing detected in this project.</p>
      {/if}
      {#each groups as [source, actions] (source)}
        <h3>{source}</h3>
        <div class="grid">
          {#each actions as a (a.id)}
            <button class="action" title={a.command} onclick={() => run(a)}>
              <span class="play">▷</span>{a.label}
            </button>
          {/each}
        </div>
      {/each}
    </section>
  </aside>
</main>

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
  .helm ul { list-style: none; margin: 0; padding: 0; }
  .helm li button { width: 100%; text-align: left; display: flex; flex-wrap: wrap; align-items: center; gap: 4px 8px; padding: 6px 8px; border-radius: 8px; }
  .helm li button:hover { background: var(--bg1); }
  .helm li button.active { background: var(--bg2); }
  .name { font: 13px var(--mono); }
  .changes { width: 6px; height: 6px; border-radius: 50%; background: var(--orange); }
  .helm .branch { flex-basis: 100%; font-size: 11px; }

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
  .action { display: flex; align-items: center; gap: 8px; text-align: left; padding: 8px 10px; border-radius: 8px; background: var(--bg1); border: 1px solid var(--bg2); font: 12.5px var(--mono); }
  .action:hover { border-color: var(--orange); }
  .play { color: var(--orange); }
</style>
