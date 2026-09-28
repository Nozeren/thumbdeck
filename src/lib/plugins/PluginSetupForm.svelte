<script lang="ts">
  // Setting up a plugin's tab for a project: its title, and the fields its plugin.toml declares
  // ([[tab.setup]]), drawn by thumbdeck.
  import FieldsForm from "./FieldsForm.svelte";
  import type { Field } from "./types.ts";

  let { name, pluginName, description, fields, setup, isNew, project, onSave, onRemove, onCancel }: {
    /** The tab's name (its default title) */
    name: string;
    pluginName: string;
    description: string;
    fields: Field[];
    setup: Record<string, any>;
    isNew: boolean;
    project: string;
    onSave: (setup: Record<string, any>) => void;
    onRemove: () => void;
    onCancel: () => void;
  } = $props();

  // svelte-ignore state_referenced_locally (a copy to edit; saved on Save)
  let values = $state($state.snapshot(setup));

  function save(e: SubmitEvent) {
    e.preventDefault();
    onSave({ ...$state.snapshot(values), title: String(values.title ?? "").trim() || name });
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" onclick={(e) => e.target === e.currentTarget && onCancel()}>
  <form class="dialog" onsubmit={save}>
    <h2><span class="dot orange"></span>{isNew ? `Add a ${name} tab` : `Set up the ${name} tab`} <span class="for">{project}</span></h2>
    {#if description}<p class="hint">{description} <span class="from">(the {pluginName} plugin)</span></p>{/if}
    <label>Tab title <input bind:value={values.title} placeholder={name} {@attach (el) => el.select()} /></label>
    <FieldsForm {fields} bind:values />
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
  .from { opacity: 0.7; }
  .spacer { flex: 1; }
  .danger { color: var(--red); }
  form { max-height: 88vh; overflow: auto; }
</style>
