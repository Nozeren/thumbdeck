<script lang="ts">
  // Setting up the Git tab for a project
  import type { SetupProps } from "../index.ts";
  import type { Setup } from "./types.ts";

  let { setup, isNew, project, onSave, onRemove, onCancel }:
    Omit<SetupProps, "setup" | "onSave"> & { setup: Setup; onSave: (setup: Setup) => void } = $props();

  // svelte-ignore state_referenced_locally (a copy to edit; saved on Save)
  let s = $state({ ...setup });

  function save(e: SubmitEvent) {
    e.preventDefault();
    onSave({ title: s.title.trim() || "Git", commits: Math.max(1, Math.round(Number(s.commits) || 100)) });
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" onclick={(e) => e.target === e.currentTarget && onCancel()}>
  <form class="dialog" onsubmit={save}>
    <h2><span class="dot orange"></span>{isNew ? "Add a Git tab" : "Set up the Git tab"} <span class="for">{project}</span></h2>
    <p class="hint">The project's repo at a glance: changes and their diffs, the branch, commits, branches and stashes.
      It only looks: it never changes the repo.</p>
    <label>Tab title <input bind:value={s.title} placeholder="Git" {@attach (el) => el.select()} /></label>
    <label>Recent commits to list <input type="number" min="1" bind:value={s.commits} /></label>
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
