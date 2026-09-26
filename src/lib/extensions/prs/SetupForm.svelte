<script lang="ts">
  // Setting up the Pull requests tab for a project
  import type { SetupProps } from "../index.ts";
  import type { Setup } from "./types.ts";
  import { splitList } from "./format.ts";

  let { setup, isNew, project, onSave, onRemove, onCancel }:
    Omit<SetupProps, "setup" | "onSave"> & { setup: Setup; onSave: (setup: Setup) => void } = $props();

  // svelte-ignore state_referenced_locally (a copy to edit; saved on Save)
  let s = $state({ ...setup, bots: setup.bots.join(", ") });

  function save(e: SubmitEvent) {
    e.preventDefault();
    onSave({
      title: s.title.trim() || "Pull requests",
      repo: s.repo.trim(),
      stale_days: Math.max(0, Math.round(Number(s.stale_days) || 0)),
      bots: splitList(s.bots),
      trim_suffix: s.trim_suffix.trim(),
      refresh_minutes: Math.max(1, Math.round(Number(s.refresh_minutes) || 5)),
    });
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" onclick={(e) => e.target === e.currentTarget && onCancel()}>
  <form class="dialog" onsubmit={save}>
    <h2><span class="dot orange"></span>{isNew ? "Add a Pull requests tab" : "Set up the Pull requests tab"} <span class="for">{project}</span></h2>
    <p class="hint">The repo's open PRs that concern you, read with the GitHub CLI (gh) and your gh login.</p>
    <label>Tab title <input bind:value={s.title} placeholder="Pull requests" {@attach (el) => el.select()} /></label>
    <label>Repo <input bind:value={s.repo} placeholder="owner/name (empty: the project's git remote; demo: sample PRs)" /></label>
    <label>Stale after (days, 0: never) <input type="number" min="0" bind:value={s.stale_days} /></label>
    <label>Refresh every (minutes) <input type="number" min="1" bind:value={s.refresh_minutes} /></label>
    <label>Reviewers to leave out (bots, comma-separated) <input bind:value={s.bots} /></label>
    <label>Cut from the end of user names <input bind:value={s.trim_suffix} placeholder="e.g. _company" /></label>
    <div class="buttons">
      {#if !isNew}<button type="button" class="ghost danger" onclick={onRemove}>Remove tab</button>{/if}
      <span class="spacer"></span>
      <button type="button" class="ghost" onclick={onCancel}>Cancel</button>
      <button type="submit" class="primary">{isNew ? "Add" : "Save"}</button>
    </div>
  </form>
</div>

<style>
  h2 { display: flex; align-items: center; gap: 8px; margin: 0; font: 600 13px var(--mono); }
  .hint { margin: 0; }
  .spacer { flex: 1; }
  .danger { color: var(--red); }
</style>
