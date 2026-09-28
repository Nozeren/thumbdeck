<script lang="ts">
  // Setting up a plugin's tab for a project: its title, and the fields its plugin.toml declares
  // ([[tab.setup]]), drawn by thumbdeck.
  import FieldsForm from "./FieldsForm.svelte";
  import PluginFrame from "./PluginFrame.svelte";
  import type { Field, FrameInfo, Host } from "./types.ts";

  let { name, pluginName, description, fields, setup, isNew, project, onSave, onRemove, onCancel, frame, tabId, projectInfo, host, thumbdeck, say }: {
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
    /** The tab's plugin: when it has a setup_page, that page is the form */
    frame: FrameInfo;
    tabId: string;
    projectInfo: { path: string; name: string; branch: string | null };
    host: Host;
    thumbdeck: string;
    say: (text: string, error?: boolean) => void;
  } = $props();

  // svelte-ignore state_referenced_locally (a copy to edit; saved on Save)
  let values = $state($state.snapshot(setup));

  function save(e: SubmitEvent) {
    e.preventDefault();
    onSave({ ...$state.snapshot(values), title: String(values.title ?? "").trim() || name });
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
{#if frame.setup_page}
<div class="backdrop" onclick={(e) => e.target === e.currentTarget && onCancel()}>
  <div class="dialog own">
    <h2><span class="dot orange"></span>{isNew ? `Add a ${name} tab` : `Set up the ${name} tab`} <span class="for">{project}</span></h2>
    <div class="page">
      <PluginFrame info={frame} page={frame.setup_page} frameKey="setup:{frame.plugin}:{tabId}" surface="tab" surfaceId={tabId}
                   project={projectInfo} {setup} active={false} visible={true} {thumbdeck} {host} {say} autofocus
                   onActivate={() => {}} onRelease={() => {}} onEditSetup={() => {}} openReview={() => {}}
                   onSaveSetup={(value) => onSave({ ...value, title: String(value?.title ?? "").trim() || name })} />
    </div>
    <div class="buttons">
      {#if !isNew}<button type="button" class="ghost danger" onclick={onRemove}>Remove tab</button>{/if}
      <span class="spacer"></span>
      <button type="button" class="ghost" onclick={onCancel}>Cancel</button>
    </div>
  </div>
</div>
{:else}
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
{/if}

<style>
  h2 { display: flex; align-items: center; gap: 8px; margin: 0; font: 600 13px var(--mono); }
  .hint { margin: 0; }
  .from { opacity: 0.7; }
  .spacer { flex: 1; }
  .danger { color: var(--red); }
  form { max-height: 88vh; overflow: auto; }
  .own { width: min(780px, 94vw); height: min(760px, 88vh); }
  .page { flex: 1; min-height: 0; display: flex; border: 1px solid var(--bg2); border-radius: 10px; overflow: hidden; }
</style>
