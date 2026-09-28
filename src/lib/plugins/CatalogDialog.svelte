<script lang="ts">
  // The first start with plugins: thumbdeck's own tabs and Toolkit buttons are plugins, so it
  // offers the official ones (the recommended ones ticked). Asked once; Settings › Plugins has
  // the same list later.
  import { invoke } from "@tauri-apps/api/core";
  import type { CatalogEntry } from "./types.ts";

  let { plugins, onDone }: { plugins: CatalogEntry[]; onDone: () => void } = $props();

  // svelte-ignore state_referenced_locally (which ones are ticked; starts from the catalog)
  let ticked = $state(new Set(plugins.filter((p) => p.recommended).map((p) => p.id)));
  let busy = $state("");
  let failed = $state<string[]>([]);

  function toggle(id: string) {
    const next = new Set(ticked);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    ticked = next;
  }

  async function install() {
    failed = [];
    for (const p of plugins.filter((x) => ticked.has(x.id))) {
      busy = `Installing ${p.name}…`;
      try {
        await invoke("plugin_add", { source: p.source, link: false });
      } catch (err) {
        failed = [...failed, `${p.name}: ${err}`];
      }
    }
    busy = "";
    await finish(failed.length === 0);
  }

  async function finish(close = true) {
    await invoke("plugin_catalog_offered");
    if (close) onDone();
  }
</script>

<div class="backdrop" role="presentation">
  <div class="dialog catalog">
    <h2><span class="dot purple"></span>Plugins <span class="for">first start</span></h2>
    <p class="hint">thumbdeck's tabs (Git, Logs, …) and Toolkit buttons (Django, npm, …) come from plugins. These are the
      official ones: pick the ones you want. Settings (,) › Plugins can add or remove them later.</p>
    <ul>
      {#each plugins as p (p.id)}
        <li>
          <label class="check">
            <input type="checkbox" checked={ticked.has(p.id)} onchange={() => toggle(p.id)} disabled={!!busy} />
            <span><strong>{p.name}</strong> <span class="desc">{p.description}</span></span>
          </label>
        </li>
      {/each}
    </ul>
    {#if busy}<p class="busy">{busy}</p>{/if}
    {#each failed as f}<p class="hint problem">{f}</p>{/each}
    <div class="buttons">
      {#if failed.length}
        <button class="primary" onclick={onDone}>Close</button>
      {:else}
        <button class="ghost" disabled={!!busy} onclick={() => finish()}>Not now</button>
        <button class="primary" disabled={!!busy || ticked.size === 0} onclick={install} {@attach (el) => el.focus()}>
          Install {ticked.size}
        </button>
      {/if}
    </div>
  </div>
</div>

<style>
  .catalog { width: min(640px, 92vw); max-height: 86vh; }
  h2 { display: flex; align-items: center; gap: 8px; margin: 0; font: 600 13px var(--mono); }
  .hint { margin: 0; }
  ul { list-style: none; margin: 0; padding: 0; overflow: auto; display: flex; flex-direction: column; gap: 6px; }
  .check { align-items: flex-start !important; font: 13px var(--sans) !important; }
  .check strong { font-family: var(--mono); }
  .desc { color: var(--grey); }
  .busy { margin: 0; color: var(--orange); font: 12px var(--mono); }
  .problem { color: var(--red); }
</style>
