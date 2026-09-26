<script lang="ts">
  // The Agents tab: the subagents and skills the project (and you) have, each one startable on
  // a task in the project's "claude" tmux window; and the project's Claude Code sessions
  // (status, model, tokens, cost) with the subagents they started and their conversations.
  // Keys come through handleKey() while the tab has the keyboard.
  import { invoke } from "@tauri-apps/api/core";
  import { tick } from "svelte";
  import type { TabProps } from "../index.ts";
  import type { AgentList, Defined, Entry, Session, Setup, Skill, Subagent } from "./types.ts";
  import { actionFor } from "../../keys/keys.ts";
  import { AGENTS_LIST, AGENTS_START, AGENTS_TRANSCRIPT } from "../../keys/maps.ts";
  import KeyHelp from "../../keys/KeyHelp.svelte";
  import { ago, money, shortModel, tokensLine, toolSummary, uses } from "./format.ts";

  let { path, project, setup, active, say, onActivate, onRelease, onEditSetup }: Omit<TabProps, "setup"> & { setup: Setup } = $props();

  // ------------------------------------------------------------ lists
  let data = $state<AgentList | null>(null);
  let error = $state("");
  const sections = ["agents", "skills", "sessions"] as const;
  let section = $state<(typeof sections)[number]>("agents");
  let cursor = $state(0);

  async function refresh() {
    try {
      data = await invoke<AgentList>("agents_list", { path, setup });
      error = "";
    } catch (err) {
      error = String(err);
    }
  }

  // Refresh while the list is shown (sessions change as they run)
  $effect(() => {
    if (open) return;
    refresh();
    const timer = setInterval(refresh, 3000);
    return () => clearInterval(timer);
  });

  type Row = { kind: "session"; session: Session } | { kind: "subagent"; session: Session; agent: Subagent };
  const rows = $derived.by<Row[]>(() =>
    (data?.sessions ?? []).flatMap((s) => [
      { kind: "session" as const, session: s },
      ...s.subagents.map((agent) => ({ kind: "subagent" as const, session: s, agent })),
    ]),
  );
  const used = $derived(uses(data?.sessions ?? []));
  const count = $derived(section === "sessions" ? rows.length : section === "skills" ? (data?.skills.length ?? 0) : (data?.defined.length ?? 0));

  function showSection(to: (typeof sections)[number]) {
    section = to;
    cursor = 0;
  }

  function move(to: number) {
    cursor = Math.max(0, Math.min(count - 1, to));
    tick().then(() => document.getElementById(`agents-row-${cursor}`)?.scrollIntoView({ block: "nearest" }));
  }

  // ------------------------------------------------------------ a conversation, or a defined agent
  type Open =
    | { kind: "transcript"; title: string; file: string; session: Session; live: boolean }
    | { kind: "defined"; agent: Defined }
    | { kind: "skill"; skill: Skill };
  let open = $state<Open | null>(null);
  let trail = $state<Open[]>([]); // what "back" returns to (a session under a subagent)
  let entries = $state<Entry[]>([]);
  let expanded = $state(new Set<number>());
  let entryCursor = $state(0);
  let scroller = $state<HTMLElement | null>(null);

  async function openTranscript(title: string, file: string, session: Session, live: boolean, push = false) {
    if (push && open) trail = [...trail, open];
    else if (!push) trail = [];
    open = { kind: "transcript", title, file, session, live };
    expanded = new Set();
    await loadEntries(true);
  }

  async function loadEntries(first = false) {
    if (open?.kind !== "transcript") return;
    const atEnd = !scroller || scroller.scrollTop + scroller.clientHeight >= scroller.scrollHeight - 30;
    try {
      entries = await invoke<Entry[]>("agents_transcript", { file: open.file });
    } catch (err) {
      say(String(err), true);
      return;
    }
    await tick();
    // Start at the end (the latest), and keep following it while it's being written
    if (first || atEnd) {
      scroller?.scrollTo({ top: scroller.scrollHeight });
      if (first || entryCursor >= entries.length - 2) entryCursor = Math.max(0, entries.length - 1);
    }
  }

  // A working session's conversation grows: read it again every 2s
  $effect(() => {
    if (open?.kind !== "transcript" || !open.live) return;
    const timer = setInterval(() => loadEntries(), 2000);
    return () => clearInterval(timer);
  });

  function back() {
    const previous = trail.at(-1);
    trail = trail.slice(0, -1);
    open = previous ?? null;
    if (previous?.kind === "transcript") loadEntries(true);
  }

  function openRow(i: number) {
    if (section === "agents") {
      const agent = data?.defined[i];
      if (agent) showToStart({ kind: "defined", agent });
      return;
    }
    if (section === "skills") {
      const skill = data?.skills[i];
      if (skill) showToStart({ kind: "skill", skill });
      return;
    }
    const r = rows[i];
    if (!r) return;
    if (r.kind === "session") openTranscript(r.session.title, r.session.file, r.session, r.session.status === "working");
    else openTranscript(`${r.agent.agent_type}: ${r.agent.description}`, r.agent.file, r.session, r.agent.status === "working");
  }

  /** Enter on an entry: open a subagent's conversation, or show / hide a tool call's details */
  function openEntry(i: number) {
    const e = entries[i];
    if (e?.kind !== "tool") return;
    if (e.agent && open?.kind === "transcript") {
      const agent = open.session.subagents.find((a) => a.id === e.agent);
      if (agent) return openTranscript(`${agent.agent_type}: ${agent.description}`, agent.file, open.session, agent.status === "working", true);
    }
    const next = new Set(expanded);
    if (next.has(i)) next.delete(i);
    else next.add(i);
    expanded = next;
  }

  // ------------------------------------------------------------ starting an agent or a skill
  let task = $state("");
  let taskBox = $state<HTMLInputElement | null>(null);
  let starting = $state(false);

  /** An agent's or skill's page, with the task box ready to type in */
  async function showToStart(what: Open) {
    open = what;
    trail = [];
    task = "";
    await tick();
    taskBox?.focus();
  }

  /** Start Claude Code on the task, as the open agent or with the open skill, in tmux */
  async function start() {
    if (starting || (open?.kind !== "defined" && open?.kind !== "skill")) return;
    const [kind, agent] = open.kind === "defined" ? ["agent", open.agent.name] : ["skill", open.skill.name];
    starting = true;
    try {
      say(await invoke<string>("agents_start", { path, name: project, kind, agent, task }));
      task = "";
    } catch (err) {
      say(String(err), true);
    } finally {
      starting = false;
    }
  }

  function taskKey(e: KeyboardEvent) {
    e.stopPropagation(); // the page's keys don't apply while typing the task
    if (e.key === "Enter") {
      e.preventDefault();
      start();
    } else if (e.key === "Escape") {
      e.preventDefault();
      taskBox?.blur();
      back();
    }
  }

  function moveEntry(to: number) {
    entryCursor = Math.max(0, Math.min(entries.length - 1, to));
    tick().then(() => document.getElementById(`agents-entry-${entryCursor}`)?.scrollIntoView({ block: "nearest" }));
  }

  // ------------------------------------------------------------ keys
  let help = $state(false);

  /** Its keys right now: the lists, a conversation, or an agent or skill to start */
  export function keymap() {
    return open?.kind === "transcript" ? AGENTS_TRANSCRIPT : open ? AGENTS_START : AGENTS_LIST;
  }

  function cycle(by: number) {
    showSection(sections[(sections.indexOf(section) + by + sections.length) % sections.length]);
  }

  export function handleKey(e: KeyboardEvent): boolean {
    if (help) {
      help = false; // any key closes the help
      return true;
    }
    const action = actionFor(keymap(), e);
    if (action === "help") return (help = true);
    if (action === "setup") return (onEditSetup(), true);
    if (action === "leave") return (onRelease(), true);
    if (open?.kind === "transcript") {
      const page = Math.max(1, Math.floor((scroller?.clientHeight ?? 400) / 60));
      switch (action) {
        case "down": moveEntry(entryCursor + 1); break;
        case "up": moveEntry(entryCursor - 1); break;
        case "first": moveEntry(0); break;
        case "last": moveEntry(entries.length - 1); break;
        case "page-down": moveEntry(entryCursor + page); break;
        case "page-up": moveEntry(entryCursor - page); break;
        case "open": openEntry(entryCursor); break;
        case "back": back(); break;
        case "refresh": loadEntries(); break;
        default: return false;
      }
      return true;
    }
    if (open) {
      switch (action) {
        case "type": e.preventDefault(); taskBox?.focus(); break;
        case "back": back(); break;
        default: return false;
      }
      return true;
    }
    switch (action) {
      case "down": move(cursor + 1); break;
      case "up": move(cursor - 1); break;
      case "first": move(0); break;
      case "last": move(count - 1); break;
      case "open": openRow(cursor); break;
      case "next-list": cycle(1); break;
      case "previous-list": cycle(-1); break;
      case "refresh": refresh(); break;
      default: return false;
    }
    return true;
  }

  const dot = (status: string) => ({ working: "●", idle: "○", stale: "◌" })[status] ?? "○";
  const statusWord = (status: string, isAgent: boolean) =>
    status === "working" ? "working" : status === "stale" ? "quiet (interrupted?)" : isAgent ? "done" : "idle";
</script>

{#snippet taskForm(label: string)}
  <form class="task" onsubmit={(e) => { e.preventDefault(); start(); }}>
    <input bind:this={taskBox} bind:value={task} onkeydown={taskKey} placeholder="What should it do? (can be left empty)" />
    <button type="submit" disabled={starting}>{label}</button>
  </form>
  <p class="dim hint">Opens in the project's "claude" tmux window. If Claude is already running there, it's shown instead.</p>
{/snippet}

<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
<div class="agents" class:active onmousedown={onActivate}>
  {#if open?.kind === "transcript"}
    <header class="bar">
      <button class="tool" title="Back (-)" onclick={back}>‹ {trail.length ? "session" : setup.title || "Agents"}</button>
      <strong class="title">{open.title}</strong>
      {#if open.live}<span class="st working">● live</span>{/if}
      <span class="spacer"></span>
      <span class="dim">{entries.length} entries</span>
    </header>
    <div class="entries" bind:this={scroller}>
      {#each entries as e, i}
        <div id="agents-entry-{i}" class="entry {e.kind}" class:cursor={i === entryCursor} onclick={() => { entryCursor = i; openEntry(i); }}>
          {#if e.kind === "prompt"}
            <span class="who">you</span><div class="text">{e.text}</div>
          {:else if e.kind === "text"}
            <span class="who">claude</span><div class="text">{e.text}</div>
          {:else if e.kind === "note"}
            <span class="who"></span><div class="text note">{e.text}</div>
          {:else}
            <span class="who">{e.result === null ? "…" : e.result.error ? "✗" : "✓"}</span>
            <div class="call">
              <span class="name" class:agent={!!e.agent}>{e.agent ? "◇ " : ""}{e.name}</span>
              <span class="summary" class:clamped={!expanded.has(i)}>{toolSummary(e)}</span>
              {#if e.agent}<span class="dim">Enter: its conversation</span>{/if}
              {#if expanded.has(i)}
                <pre class="detail">{JSON.stringify(e.input, null, 2)}</pre>
                {#if e.result}
                  <pre class="detail" class:err={e.result.error}>{e.result.text}{e.result.length > e.result.text.length ? `\n… (${e.result.length} characters)` : ""}</pre>
                {/if}
              {/if}
            </div>
          {/if}
        </div>
      {:else}
        <p class="dim empty">Nothing in this conversation yet.</p>
      {/each}
    </div>
  {:else if open?.kind === "defined"}
    {@const a = open.agent}
    <header class="bar">
      <button class="tool" title="Back (-)" onclick={back}>‹ {setup.title || "Agents"}</button>
      <strong class="title">{a.name}</strong>
      <span class="dim">{a.scope === "project" ? "this project" : "yours (all projects)"}</span>
    </header>
    <div class="defined">
      <p>{a.description}</p>
      <p class="dim">model: {a.model ?? "inherits the session's"} · tools: {a.tools.length ? a.tools.join(", ") : "all"}
        · used {used.get(a.name) ?? 0}× in these sessions</p>
      <p class="dim">{a.file}</p>
      {@render taskForm(`Start Claude Code as ${a.name}`)}
      <pre class="instructions">{a.instructions}</pre>
    </div>
  {:else if open?.kind === "skill"}
    {@const s = open.skill}
    <header class="bar">
      <button class="tool" title="Back (-)" onclick={back}>‹ {setup.title || "Agents"}</button>
      <strong class="title">/{s.name}</strong>
      <span class="dim">{s.scope === "project" ? "this project" : "yours (all projects)"}</span>
    </header>
    <div class="defined">
      <p>{s.description}</p>
      <p class="dim">{s.file}</p>
      {@render taskForm(`Start Claude Code with /${s.name}`)}
      <pre class="instructions">{s.instructions}</pre>
    </div>
  {:else}
    <header class="bar">
      <button class="seg" class:on={section === "agents"} onclick={() => showSection("agents")}>
        Agents <span class="dim">{data?.defined.length ?? ""}</span></button>
      <button class="seg" class:on={section === "skills"} onclick={() => showSection("skills")}>
        Skills <span class="dim">{data?.skills.length ?? ""}</span></button>
      <button class="seg" class:on={section === "sessions"} onclick={() => showSection("sessions")}>
        Sessions <span class="dim">{data?.sessions.length ?? ""}</span></button>
      <span class="dim">Tab switches</span>
      <span class="spacer"></span>
      <button class="tool" title="Set up this tab (S)" onclick={onEditSetup}>⚙ setup</button>
    </header>
    {#if error}<p class="note-line err">{error}</p>{/if}
    <ul class="rows">
      {#if section === "sessions"}
        {#each rows as r, i}
          <li id="agents-row-{i}">
            <button class="row" class:cursor={i === cursor} class:sub={r.kind === "subagent"}
                    onclick={() => (cursor = i)} ondblclick={() => openRow(i)}>
              {#if r.kind === "session"}
                {@const s = r.session}
                <span class="st {s.status}" title={statusWord(s.status, false)}>{dot(s.status)}</span>
                <span class="title">{s.title || "(no prompt yet)"}</span>
                <span class="meta">{shortModel(s.model)}</span>
                <span class="meta" title={s.cost_behind ? "at least: Claude Code writes the total now and then" : tokensLine(s.tokens)}>
                  {s.cost !== null ? money(s.cost) + (s.cost_behind ? "+" : "") : tokensLine(s.tokens)}</span>
                <span class="meta">{ago(s.last_activity)}</span>
              {:else}
                {@const a = r.agent}
                <span class="st {a.status}" title={statusWord(a.status, true)}>{dot(a.status)}</span>
                <span class="title"><span class="kind">◇ {a.agent_type}</span> {a.description}</span>
                <span class="meta">{shortModel(a.model)}</span>
                <span class="meta">{tokensLine(a.tokens)}</span>
                <span class="meta">{statusWord(a.status, true)}</span>
              {/if}
            </button>
          </li>
        {:else}
          <li class="dim empty">
            {data ? "No Claude Code sessions in this project yet." : "Looking…"}
            {#if data}<span class="where">(read from {data.dir})</span>{/if}
          </li>
        {/each}
      {:else if section === "skills"}
        {#each data?.skills ?? [] as s, i}
          <li id="agents-row-{i}">
            <button class="row" class:cursor={i === cursor} onclick={() => (cursor = i)} ondblclick={() => openRow(i)}>
              <span class="st idle">/</span>
              <span class="title"><span class="kind">{s.name}</span> {s.description}</span>
              <span class="meta">{s.scope === "project" ? "project" : "yours"}</span>
            </button>
          </li>
        {:else}
          <li class="dim empty">No skills: add them in .claude/skills/&lt;name&gt;/SKILL.md (or ~/.claude/skills/ for all projects).</li>
        {/each}
      {:else}
        {#each data?.defined ?? [] as a, i}
          <li id="agents-row-{i}">
            <button class="row" class:cursor={i === cursor} onclick={() => (cursor = i)} ondblclick={() => openRow(i)}>
              <span class="st idle" style={a.color ? `color: var(--${a.color}, ${a.color})` : ""}>◆</span>
              <span class="title"><span class="kind">{a.name}</span> {a.description}</span>
              <span class="meta">{a.scope === "project" ? "project" : "yours"}</span>
              <span class="meta">{a.model ?? "inherit"}</span>
              <span class="meta">used {used.get(a.name) ?? 0}×</span>
            </button>
          </li>
        {:else}
          <li class="dim empty">No subagents defined: add them in .claude/agents/ (or ~/.claude/agents/ for all projects).</li>
        {/each}
      {/if}
    </ul>
  {/if}

  {#if help}<KeyHelp map={keymap()} />{/if}
</div>

<style>
  .agents { position: relative; flex: 1; min-height: 0; display: flex; flex-direction: column; font: 12.5px var(--mono);
            border-top: 2px solid transparent; }
  .agents.active { border-top-color: var(--orange); }
  .bar { display: flex; align-items: center; gap: 10px; padding: 6px 12px; border-bottom: 1px solid var(--bg2); white-space: nowrap; }
  .spacer { flex: 1; }
  .dim { color: var(--grey); }
  .err { color: var(--red); }
  .tool, .seg { padding: 2px 8px; border-radius: 6px; color: var(--grey); font: 12px var(--mono); }
  .tool:hover, .seg:hover { background: var(--bg2); color: var(--fg); }
  .seg.on { background: var(--bg2); color: var(--fg); }
  .title { overflow: hidden; text-overflow: ellipsis; min-width: 0; }
  .note-line { margin: 6px 12px; }

  .rows { list-style: none; margin: 0; padding: 6px; overflow: auto; flex: 1; min-height: 0; }
  .row { width: 100%; display: flex; gap: 10px; align-items: center; padding: 4px 8px; border-radius: 6px; text-align: left;
         font: 12.5px var(--mono); white-space: nowrap; }
  .row .title { flex: 1; }
  .row.sub { padding-left: 28px; color: var(--grey); }
  .row.cursor { background: var(--bg2); }
  .agents.active .row.cursor { box-shadow: inset 2px 0 var(--orange); }
  .meta { flex: none; color: var(--grey); font-size: 11.5px; }
  .kind { color: var(--purple); }
  .st { flex: none; width: 12px; text-align: center; color: var(--grey); }
  .st.working { color: var(--green); }
  .st.stale { color: var(--orange); }
  .empty { padding: 10px; white-space: normal; }
  .where { display: block; font-size: 11px; margin-top: 4px; word-break: break-all; }

  .entries { flex: 1; min-height: 0; overflow: auto; padding: 6px 0; }
  .entry { display: grid; grid-template-columns: 56px 1fr; gap: 8px; padding: 4px 12px; cursor: default; }
  .entry.cursor { background: var(--bg1); }
  .agents.active .entry.cursor { box-shadow: inset 2px 0 var(--orange); }
  .who { color: var(--grey); text-align: right; font-size: 11px; padding-top: 1px; }
  .entry.prompt .who { color: var(--aqua); }
  .entry.prompt .text { color: var(--aqua); }
  .text { white-space: pre-wrap; word-break: break-word; }
  .note { color: var(--grey); font-style: italic; }
  .call { min-width: 0; }
  .call .name { color: var(--yellow); margin-right: 8px; }
  .call .name.agent { color: var(--purple); }
  .call .summary { color: var(--fg); overflow-wrap: anywhere; }
  /* Long commands take two lines until opened */
  .call .summary.clamped { display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .call .dim { margin-left: 8px; font-size: 11px; }
  .detail { margin: 6px 0 2px; padding: 8px 10px; background: var(--bg-dim); border-radius: 6px; white-space: pre-wrap;
            word-break: break-word; max-height: 360px; overflow: auto; font: 12px var(--mono); }

  .defined { padding: 10px 16px; overflow: auto; flex: 1; min-height: 0; }
  .defined p { margin: 6px 0; }
  .task { display: flex; gap: 8px; margin: 14px 0 4px; }
  .task input { flex: 1; min-width: 0; padding: 6px 10px; border-radius: 6px; font: 12.5px var(--mono); }
  .task button { flex: none; padding: 6px 12px; border-radius: 6px; background: var(--bg2); font: 12.5px var(--mono); }
  .task button:hover { color: var(--orange); }
  .hint { font-size: 11.5px; margin: 0 0 10px; }
  .instructions { margin-top: 12px; padding: 10px 12px; background: var(--bg-dim); border-radius: 8px; white-space: pre-wrap; font: 12.5px var(--mono); }

</style>
