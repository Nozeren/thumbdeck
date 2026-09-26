<script lang="ts">
  // The ? help of a place, drawn from its keymap (any key closes it).
  import { keysLabel, type Keymap } from "./keys.ts";

  let { map, note = "" }: { map: Keymap; note?: string } = $props();
</script>

<div class="keyhelp">
  <strong>{map.name}: keys</strong> <span class="dim">(any key closes this){note ? ` · ${note}` : ""}</span>
  <dl>
    {#each map.bindings as b}
      <dt>{keysLabel(b)}</dt><dd>{b.does}</dd>
    {/each}
  </dl>
</div>

<style>
  .keyhelp { position: absolute; inset: 0; background: var(--bg0); padding: 16px 20px; overflow: auto; z-index: 5; font: 12.5px var(--mono); }
  .dim { color: var(--grey); }
  dl { display: grid; grid-template-columns: max-content 1fr; gap: 4px 16px; }
  dt { color: var(--orange); } dd { margin: 0; }
</style>
