<script lang="ts">
  // The Overview, the center's first tab: where the project stands, at a glance. Cards: how
  // each Toolkit action ran last (thumbdeck's own), then the cards of the project's plugins
  // (the Git plugin's changes and commits, …) in the plugins' order. Read-only. Keys come
  // through handleKey() while the center has the keyboard; the card under the cursor gets
  // its own keys too.
  import { tick } from "svelte";
  import type { Details, Run } from "../types.ts";
  import type { CardInfo, Host } from "../plugins/types.ts";
  import type { ReviewRequest } from "../review/types.ts";
  import PluginFrame from "../plugins/PluginFrame.svelte";
  import { actionFor, type Keymap } from "../keys/keys.ts";
  import { OVERVIEW } from "../keys/maps.ts";
  import { ago, duration } from "./time.ts";

  let { project, details, runs, cards: pluginCards, visible, active, now, thumbdeck, host, badges, reloads, say, openReview, openTab, toToolkit, onFocus, hideCard }: {
    project: { path: string; name: string; branch: string | null };
    details: Details;
    /** The project's runs, newest first */
    runs: Run[];
    cards: CardInfo[];
    /** Shown (it's kept while hidden, so its cards don't load again) */
    visible: boolean;
    /** The center has the keyboard: the cursor shows */
    active: boolean;
    now: number;
    thumbdeck: string;
    host: Host;
    badges: Record<string, string | null>;
    /** Bumped when a linked plugin changes: its cards load again */
    reloads: Record<string, number>;
    say: (text: string, error?: boolean) => void;
    openReview: (r: ReviewRequest) => void;
    /** Show the project's tab (a plugin's, by its id); false: the project has none */
    openTab: (plugin: string, tab: string) => boolean;
    /** The Toolkit pane gets the keyboard */
    toToolkit: () => void;
    /** A click in a card: the center gets the keyboard */
    onFocus: () => void;
    /** Hide a card in this project ("runs" or "<plugin>:<card>"), or show it again */
    hideCard: (id: string, hide: boolean) => void;
  } = $props();

  type Card = { kind: "runs"; id: string; name: string } | { kind: "plugin"; id: string; name: string; info: CardInfo; key: string };
  // Every card the project could show, and the ones you didn't hide (x)
  const all = $derived<Card[]>([
    ...(details.actions.length ? [{ kind: "runs", id: "runs", name: "Last runs" } as const] : []),
    ...pluginCards.map((info) => ({
      kind: "plugin" as const, id: `${info.plugin}:${info.id}`, name: info.name, info, key: `card:${info.plugin}:${info.id}:${project.path}`,
    })),
  ]);
  const cards = $derived(all.filter((c) => !details.hidden_cards.includes(c.id)));
  const hidden = $derived(all.filter((c) => details.hidden_cards.includes(c.id)));
  let showHidden = $state(false);
  let cursor = $state(0);
  $effect(() => {
    if (cursor >= cards.length) cursor = Math.max(0, cards.length - 1);
  });
  let frameRefs = $state<Record<string, { keymap(): Keymap; handleKey(e: KeyboardEvent): boolean } | null>>({});

  // The keys of a plugin's card (not thumbdeck's setup / help / leave, which it adds to each frame's)
  function cardKeys(card: Card | undefined): Keymap | null {
    if (card?.kind !== "plugin") return null;
    const map = frameRefs[card.key]?.keymap();
    return map ? { ...map, bindings: map.bindings.filter((b) => !["setup", "help", "leave"].includes(b.action)) } : null;
  }

  /** The keys right now: moving between cards, and the card's own */
  export function keymap(): Keymap {
    const own = cardKeys(cards[cursor]);
    if (!own?.bindings.length) return OVERVIEW;
    const taken = new Set(own.bindings.flatMap((b) => b.keys));
    const mine = OVERVIEW.bindings.map((b) => ({ ...b, keys: b.keys.filter((k) => !taken.has(k)) })).filter((b) => b.keys.length);
    return { name: own.name, grid: true, bindings: [...mine, ...own.bindings] };
  }

  function press(card: Card | undefined) {
    if (card?.kind === "runs") toToolkit();
    else if (card?.kind === "plugin" && card.info.opens && !openTab(card.info.plugin, card.info.opens)) {
      say(`Add the ${card.info.frame.plugin_name} tab (+ at the bottom) to see more`);
    }
  }

  let gridEl = $state<HTMLElement | null>(null);

  // h / j / k / l between the cards, as many to a row as the grid shows
  function move(action: string) {
    const n = cards.length;
    const cols = gridEl ? getComputedStyle(gridEl).gridTemplateColumns.split(" ").length : 1;
    const col = cursor % cols;
    const next =
      action === "first" ? 0
      : action === "last" ? n - 1
      : action === "left" ? (col > 0 ? cursor - 1 : cursor)
      : action === "right" ? (col < cols - 1 && cursor + 1 < n ? cursor + 1 : cursor)
      : action === "down" ? (cursor + cols < n ? cursor + cols : cursor)
      : cursor - cols >= 0 ? cursor - cols : cursor;
    cursor = Math.max(0, next);
    tick().then(() => document.querySelector(".ocard.cursor")?.scrollIntoView({ block: "nearest" }));
  }

  export function handleKey(e: KeyboardEvent): boolean {
    const action = actionFor(OVERVIEW, e);
    if (action && ["down", "up", "left", "right", "first", "last"].includes(action)) return move(action), true;
    const card = cards[cursor];
    if (card?.kind === "plugin" && cardKeys(card) && actionFor(cardKeys(card)!, e)) return frameRefs[card.key]!.handleKey(e);
    if (action === "press") return press(card), true;
    if (action === "hide" && card) return hideCard(card.id, true), say(`${card.name} hidden in this project: it's listed below the cards`), true;
    return false;
  }

  // How an action ran last: running now (this session), or the result remembered
  function lastRun(label: string) {
    const live = runs.find((r) => r.label === label);
    if (live?.endedAt === null) return { state: "running", text: `running ${duration((now - live.startedAt) / 1000)}` };
    const last = details.last_runs[label];
    if (!last) return { state: "never", text: "never run" };
    const how = last.code === 0 ? "" : last.code === -1 ? "stopped · " : `exit ${last.code} · `;
    return { state: last.code === 0 ? "done" : "failed", text: `${how}${duration(last.seconds)} · ${ago(last.ended, now)}` };
  }
  const ICON: Record<string, string> = { running: "●", done: "✓", failed: "✗", never: "·" };
</script>

<div class="overview" class:hidden={!visible}>
  {#if !cards.length}
    <p class="empty">{hidden.length ? "Every card is hidden in this project: bring them back below." : "Nothing here yet: add your own action in the Toolkit (a), or plugins with cards (Git) in Settings › Plugins."}</p>
  {/if}
  <div class="grid" bind:this={gridEl}>
    {#each cards as card, i (card.kind === "plugin" ? `${card.key}#${reloads[card.info.plugin] ?? 0}` : card.kind)}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <section class="ocard" class:cursor={active && i === cursor} role="button" tabindex="-1"
               onclick={() => ((cursor = i), onFocus())} ondblclick={() => press(card)}>
        {#if card.kind === "runs"}
          <h3><span class="dot orange"></span>Last runs</h3>
          <ul>
            {#each details.actions as a (a.id)}
              {@const r = a.tmux ? { state: "never", text: `runs in tmux (${a.tmux})` } : lastRun(a.label)}
              <li class="row"><span class="st {r.state}">{ICON[r.state]}</span><span class="label">{a.label}</span><span class="when">{r.text}</span></li>
            {/each}
          </ul>
        {:else}
          <h3><span class="dot blue"></span>{card.info.name}{#if badges[card.key]}<span class="badge">{badges[card.key]}</span>{/if}</h3>
          <PluginFrame bind:this={frameRefs[card.key]} info={card.info.frame} frameKey={card.key} surface="card" surfaceId={card.info.id}
                       {project} setup={null} active={false} {visible} lines={card.info.lines} {thumbdeck} {host} {say}
                       onActivate={() => ((cursor = i), onFocus())} onRelease={() => {}} onEditSetup={() => {}} {openReview} />
        {/if}
      </section>
    {/each}
  </div>
  {#if hidden.length}
    <button class="fold" onclick={() => (showHidden = !showHidden)}>
      <span class="caret">{showHidden ? "▾" : "▸"}</span>hidden cards ({hidden.length})
    </button>
    {#if showHidden}
      <ul class="hidden-list">
        {#each hidden as c (c.id)}
          <li><span class="label">{c.name}</span><button class="icon" title="Show again" onclick={() => hideCard(c.id, false)}>↺</button></li>
        {/each}
      </ul>
    {/if}
  {/if}
</div>

<style>
  .overview { flex: 1; min-height: 0; overflow: auto; padding: 16px; }
  .overview.hidden { display: none; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(max(280px, calc(50% - 6px)), 1fr)); gap: 12px; align-items: start; }
  .ocard { min-width: 0; display: flex; flex-direction: column; padding: 12px 14px; border-radius: 10px; background: var(--bg0); border: 1px solid var(--bg2); cursor: default; outline: none; }
  .ocard.cursor { border-color: var(--grey-dim); box-shadow: inset 2px 0 var(--cursor); }
  h3 { display: flex; align-items: center; gap: 8px; margin: 0 0 8px; font: 600 12.5px var(--sans); color: var(--fg); }
  .dot { width: 8px; height: 8px; border-radius: 50%; flex: none; }
  .dot.orange { background: var(--orange); } .dot.blue { background: var(--blue); }
  .badge { padding: 0 6px; border-radius: 8px; background: var(--bg2); color: var(--fg); font: 11px var(--mono); }
  ul { margin: 0; padding: 0; list-style: none; display: flex; flex-direction: column; gap: 3px; }
  li { display: flex; align-items: baseline; gap: 8px; min-width: 0; font: 12px var(--mono); }
  .label { min-width: 4ch; flex: 1 0 auto; max-width: 70%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .st { flex: none; width: 12px; text-align: center; color: var(--grey); }
  .st.running { color: var(--running); } .st.done { color: var(--green); } .st.failed { color: var(--red); }
  .when { flex: 0 1 auto; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--grey); font-size: 11px; }
  .empty { color: var(--grey); }
  .fold { margin-top: 14px; padding: 2px 4px; border: 0; background: none; color: var(--grey); font: 11.5px var(--mono); cursor: pointer; }
  .fold:hover { color: var(--fg); }
  .caret { display: inline-block; width: 12px; }
  .hidden-list { margin: 4px 0 0 16px; }
  .hidden-list li { color: var(--grey); }
  .icon { padding: 0 6px; border: 0; background: none; color: var(--grey); cursor: pointer; }
  .icon:hover { color: var(--fg); }
</style>
