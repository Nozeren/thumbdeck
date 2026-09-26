<script lang="ts">
  // The Pull requests tab: the repo's open PRs that concern you (to review again, to review,
  // reviewed, yours) on top, the highlighted one's details below, like fzf with a preview.
  // Enter opens it in the browser. Keys come through handleKey() while the tab has the keyboard.
  import { invoke } from "@tauri-apps/api/core";
  import { tick } from "svelte";
  import type { TabProps } from "../index.ts";
  import type { Pr, PrList, Setup } from "./types.ts";
  import { categoryLabel, checksText, isStale, reviewText } from "./format.ts";
  import { ago } from "../agents/format.ts";
  import { actionFor } from "../../keys/keys.ts";
  import { PRS } from "../../keys/maps.ts";
  import KeyHelp from "../../keys/KeyHelp.svelte";
  import { avatarSignals } from "../../avatar/signals.svelte.ts";

  let { path, setup, active, say, onActivate, onRelease, onEditSetup, openReview }: Omit<TabProps, "setup"> & { setup: Setup } = $props();

  let data = $state<PrList | null>(null);
  let error = $state("");
  let loading = $state(false);
  let cursor = $state(0);
  const pr = $derived<Pr | undefined>(data?.prs[cursor]);

  async function refresh() {
    if (loading) return;
    loading = true;
    try {
      data = await invoke<PrList>("prs_list", { path, setup });
      // The avatar in the top bar holds up a sign while PRs wait for your review
      avatarSignals.prsToReview = data.prs.filter((p) => p.category === "review" || p.category === "re-review").length;
      error = "";
      cursor = Math.min(cursor, Math.max(0, data.prs.length - 1));
    } catch (err) {
      error = String(err);
    } finally {
      loading = false;
    }
  }

  // Fetch now, then every few minutes while the tab is shown
  $effect(() => {
    refresh();
    const timer = setInterval(refresh, Math.max(1, setup.refresh_minutes) * 60_000);
    return () => clearInterval(timer);
  });

  function move(to: number) {
    cursor = Math.max(0, Math.min((data?.prs.length ?? 1) - 1, to));
    tick().then(() => document.getElementById(`prs-row-${cursor}`)?.scrollIntoView({ block: "nearest" }));
  }

  /** In the browser; its notification is marked read */
  async function openPr(p: Pr | undefined) {
    if (!p) return;
    if (data?.demo) {
      p.notification = null;
      return say(`demo: would open ${p.url}`);
    }
    try {
      await invoke("prs_open", { url: p.url, notification: p.notification });
      p.notification = null;
    } catch (err) {
      say(String(err), true);
    }
  }

  /** The review page for a PR's changes (fetched with gh, nothing is checked out) */
  function reviewPr(p: Pr | undefined) {
    if (!p || !data) return;
    openReview({
      title: `#${p.number} ${p.title}`,
      subtitle: `@${p.author} · ${data.repo}${p.draft ? " · draft" : ""}`,
      load: () => invoke<string>("prs_diff", { path, setup, number: p.number }),
      viewedKey: `pr:${data.repo}#${p.number}`,
    });
  }

  let help = $state(false);

  export const keymap = () => PRS;

  let detailsEl = $state<HTMLElement | null>(null);

  export function handleKey(e: KeyboardEvent): boolean {
    if (help) {
      help = false; // any key closes the help
      return true;
    }
    const page = (detailsEl?.clientHeight ?? 300) * 0.8;
    switch (actionFor(PRS, e)) {
      case "down": move(cursor + 1); break;
      case "up": move(cursor - 1); break;
      case "first": move(0); break;
      case "last": move((data?.prs.length ?? 0) - 1); break;
      case "open": case "review": reviewPr(pr); break;
      case "outside": openPr(pr); break;
      case "page-down": detailsEl?.scrollBy({ top: page }); break;
      case "page-up": detailsEl?.scrollBy({ top: -page }); break;
      case "refresh": refresh(); break;
      case "setup": onEditSetup(); break;
      case "help": help = true; break;
      case "leave": onRelease(); break;
      default: return false;
    }
    return true;
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="prs" class:active onmousedown={onActivate}>
  <header class="bar">
    <strong>{data?.repo ?? "…"}</strong>
    {#if data?.demo}<span class="yellow">sample PRs, not from GitHub</span>{/if}
    <span class="dim">{data ? `${data.prs.length} for @${data.user}` : ""}</span>
    <span class="spacer"></span>
    {#if loading}<span class="dim">refreshing…</span>{/if}
    <button class="tool" title="Refresh (r)" onclick={refresh}>↻</button>
    <button class="tool" title="Set up this tab (S)" onclick={onEditSetup}>⚙ setup</button>
  </header>
  {#if error}<p class="note-line err">{error}</p>{/if}

  <ul class="rows">
    {#each data?.prs ?? [] as p, i}
      <li id="prs-row-{i}">
        <button class="row" class:cursor={i === cursor} onclick={() => (cursor = i)} ondblclick={() => reviewPr(p)}>
          <span class="bell" title={p.notification ? "unread notification" : ""}>{p.notification ? "🔔" : ""}</span>
          <span class="cat {p.category}">{categoryLabel[p.category]}</span>
          <span class="author">@{p.author}</span>
          <span class="num {p.category}">#{p.number}</span>
          <span class="title" class:draft={p.draft}>{p.title}</span>
        </button>
      </li>
    {:else}
      <li class="dim empty">{data ? "No open PRs that concern you." : error ? "" : "Asking GitHub…"}</li>
    {/each}
  </ul>

  {#if pr}
    <section class="details" bind:this={detailsEl}>
      <strong class="big">{pr.title}</strong>
      <dl>
        <dt>Author</dt><dd>@{pr.author}</dd>
        <dt>Updated</dt><dd class="yellow">{ago(pr.updated)}</dd>
        <dt>Checks</dt><dd class="checks {pr.checks}">{checksText(pr.checks)}</dd>
        {#if pr.labels.length}<dt>Labels</dt><dd>{pr.labels.map((l) => `[${l}]`).join("  ")}</dd>{/if}
        {#if pr.comments}
          <dt>Comments</dt><dd>{pr.comments}
            {#if pr.last_comment}<span class="dim">(last: @{pr.last_comment.author} · {ago(pr.last_comment.updated)})</span>{/if}</dd>
        {/if}
      </dl>
      <p class="flags">
        {#if pr.draft}<span class="dim">DRAFT</span>{/if}
        {#if isStale(pr.updated, setup.stale_days)}<span class="yellow">⏰ STALE</span>{/if}
        {#if pr.mergeable === "CONFLICTING"}<span class="red">⚠ MERGE CONFLICTS</span>{/if}
      </p>
      <strong>Reviewers</strong>
      {#each pr.reviewers as r}
        <div class="reviewer"><span class="state {r.state}">@{r.name}</span> <span class="dim">{reviewText(r.state)}</span></div>
      {:else}
        <div class="dim reviewer">No reviewers assigned</div>
      {/each}
      <p class="dim url">{pr.url} · <button class="link" onclick={() => reviewPr(pr)}>Enter: review the changes</button> · <button class="link" onclick={() => openPr(pr)}>o: open in the browser</button></p>
    </section>
  {/if}

  {#if help}<KeyHelp map={PRS} note="RE-REVIEW: asked again after you reviewed · REVIEW: asked · REVIEWED: you reviewed · MINE: yours" />{/if}
</div>

<style>
  .prs { position: relative; flex: 1; min-height: 0; display: flex; flex-direction: column; font: 12.5px var(--mono);
         border-top: 2px solid transparent; }
  .prs.active { border-top-color: var(--orange); }
  .bar { display: flex; align-items: center; gap: 10px; padding: 6px 12px; border-bottom: 1px solid var(--bg2); white-space: nowrap; }
  .spacer { flex: 1; }
  .dim { color: var(--grey); }
  .err, .red { color: var(--red); }
  .yellow { color: var(--yellow); }
  .tool { padding: 2px 8px; border-radius: 6px; color: var(--grey); font: 12px var(--mono); }
  .tool:hover { background: var(--bg2); color: var(--fg); }
  .note-line { margin: 6px 12px; }

  .rows { list-style: none; margin: 0; padding: 6px; overflow: auto; flex: 0 1 auto; max-height: 45%; min-height: 60px; }
  .row { width: 100%; display: flex; gap: 10px; align-items: center; padding: 3px 8px; border-radius: 6px; text-align: left;
         font: 12.5px var(--mono); white-space: nowrap; }
  .row.cursor { background: var(--bg2); }
  .prs.active .row.cursor { box-shadow: inset 2px 0 var(--orange); }
  .bell { flex: none; width: 18px; }
  .cat { flex: none; width: 76px; }
  .author { flex: none; width: 150px; color: var(--grey); overflow: hidden; text-overflow: ellipsis; }
  .num { flex: none; width: 56px; font-weight: 600; }
  .title { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; }
  .title.draft { color: var(--grey); }
  .re-review { color: var(--red); } .review { color: var(--yellow); } .reviewed { color: var(--purple); } .mine { color: var(--green); }
  .num.review { color: var(--aqua); }
  .empty { padding: 10px; }

  .details { flex: 1; min-height: 0; overflow: auto; padding: 10px 16px; border-top: 1px solid var(--bg2); }
  .big { display: block; font-size: 13.5px; margin-bottom: 6px; }
  .details dl { display: grid; grid-template-columns: 90px 1fr; gap: 3px 10px; margin: 6px 0; }
  .details dt { color: var(--grey); } .details dd { margin: 0; }
  .checks.SUCCESS { color: var(--green); } .checks.FAILURE, .checks.ERROR { color: var(--red); } .checks.PENDING { color: var(--yellow); }
  .flags { display: flex; gap: 14px; margin: 6px 0 10px; }
  .flags:empty { display: none; }
  .reviewer { padding: 2px 0 2px 12px; }
  .state.APPROVED { color: var(--green); } .state.CHANGES_REQUESTED { color: var(--red); }
  .state.COMMENTED { color: var(--aqua); } .state.PENDING { color: var(--yellow); }
  .url { margin-top: 10px; word-break: break-all; }
  .link { padding: 0; color: var(--orange); font: inherit; }
  .link:hover { text-decoration: underline; }

</style>
