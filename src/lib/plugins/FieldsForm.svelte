<script lang="ts">
  // A plugin's settings or a tab's setup, drawn from the fields its plugin.toml declares.
  // Edits `values` in place; the caller saves them.
  import { open } from "@tauri-apps/plugin-dialog";
  import type { Field } from "./types.ts";

  let { fields, values = $bindable() }: { fields: Field[]; values: Record<string, any> } = $props();

  const many = (f: Field) => f.type === "list" || f.type === "folders" || f.type === "files";
  const folderish = (f: Field) => f.type === "folder" || f.type === "folders";

  async function browse(f: Field, index: number | null) {
    const picked = await open({ directory: folderish(f), title: f.label });
    if (typeof picked !== "string") return;
    if (index === null) values[f.key] = picked;
    else values[f.key][index] = picked;
  }
</script>

{#each fields as f (f.key)}
  {#if f.type === "bool"}
    <label class="check"><input type="checkbox" bind:checked={values[f.key]} /> {f.label}</label>
  {:else if many(f)}
    <div class="field">
      <span class="name">{f.label}</span>
      {#each values[f.key] as _, i}
        <div class="row">
          <input bind:value={values[f.key][i]} />
          {#if f.type !== "list"}<button type="button" class="ghost small" onclick={() => browse(f, i)}>Browse…</button>{/if}
          <button type="button" class="icon" title="Remove" onclick={() => values[f.key].splice(i, 1)}>×</button>
        </div>
      {/each}
      <button type="button" class="ghost small add" onclick={() => values[f.key].push("")}>+ Add</button>
    </div>
  {:else}
    <label>{f.label}
      {#if f.type === "number"}
        <input type="number" min={f.min} max={f.max} bind:value={values[f.key]} />
      {:else if f.type === "choice"}
        <select bind:value={values[f.key]}>
          {#each f.choices as c}<option value={c.value}>{c.label}</option>{/each}
        </select>
      {:else if f.type === "text" && f.multiline}
        <textarea rows="4" bind:value={values[f.key]}></textarea>
      {:else if f.type === "folder" || f.type === "file"}
        <span class="row">
          <input bind:value={values[f.key]} />
          <button type="button" class="ghost small" onclick={() => browse(f, null)}>Browse…</button>
        </span>
      {:else}
        <input bind:value={values[f.key]} />
      {/if}
    </label>
  {/if}
  {#if f.help}<p class="hint">{f.help}</p>{/if}
{/each}

<style>
  .field { display: flex; flex-direction: column; gap: 6px; font: 12px var(--mono); color: var(--grey); }
  .row { display: flex; align-items: center; gap: 6px; }
  .row input { flex: 1; min-width: 0; }
  .field input, input[type="number"], select {
    background: var(--bg1); border: 1px solid var(--bg2); border-radius: 8px; padding: 8px 10px;
    color: var(--fg); font: 13px var(--mono); outline: none;
  }
  .field input:focus, input[type="number"]:focus, select:focus { border-color: var(--orange); }
  .small { padding: 5px 10px; }
  .add { align-self: flex-start; }
</style>
