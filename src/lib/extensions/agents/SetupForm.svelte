<script lang="ts">
  // Setting up the Agents tab for a project
  import type { SetupProps } from "../index.ts";
  import type { Setup } from "./types.ts";

  let { setup, isNew, project, onSave, onRemove, onCancel }:
    Omit<SetupProps, "setup" | "onSave"> & { setup: Setup; onSave: (setup: Setup) => void } = $props();

  // svelte-ignore state_referenced_locally (a copy to edit; saved on Save)
  let title = $state(setup.title);
  // svelte-ignore state_referenced_locally
  let userAgents = $state(setup.user_agents);
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" onclick={(e) => e.target === e.currentTarget && onCancel()}>
  <form class="dialog" onsubmit={(e) => { e.preventDefault(); onSave({ title: title.trim() || "Agents", user_agents: userAgents }); }}>
    <h2><span class="dot orange"></span>{isNew ? "Add an Agents tab" : "Set up the Agents tab"} <span class="for">{project}</span></h2>
    <p class="hint">The subagents and skills in this project's .claude/ folder, each startable on a task, and the
      Claude Code sessions run in this project.</p>
    <label>Tab title <input bind:value={title} placeholder="Agents" {@attach (el) => el.select()} /></label>
    <label class="check"><input type="checkbox" bind:checked={userAgents} /> Also list your own subagents and skills (~/.claude/agents, ~/.claude/skills)</label>
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
