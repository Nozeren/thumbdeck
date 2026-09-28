<script lang="ts">
  // Settings: for now its one section, Plugins. Every installed plugin (on, off or broken):
  // what it adds, its settings, its README; adding one from a git URL or a folder, updating,
  // turning off and removing.
  import { invoke } from "@tauri-apps/api/core";
  import { ask, open } from "@tauri-apps/plugin-dialog";
  import { marked } from "marked";
  import { onMount, tick } from "svelte";
  import FieldsForm from "../plugins/FieldsForm.svelte";
  import type { PluginInfo } from "../plugins/types.ts";

  let { updates, onUpdates, onChanged, onClose }: {
    /** Plugins with a newer release: id -> its tag */
    updates: Record<string, string>;
    onUpdates: (updates: Record<string, string>) => void;
    /** Plugins changed: the Toolkit and icons may have too */
    onChanged: () => void;
    onClose: () => void;
  } = $props();

  let plugins = $state<PluginInfo[]>([]);
  let loaded = $state(false);
  // The plugin shown on the right, or "add"
  let shown = $state<string>("add");
  let busy = $state(""); // what's happening ("Installing…"); buttons wait meanwhile
  let error = $state("");
  let source = $state("");
  let values = $state<Record<string, any>>({});
  let readmeEl = $state<HTMLElement | null>(null);

  const plugin = $derived(plugins.find((p) => p.id === shown) ?? null);
  const changed = $derived(!!plugin && JSON.stringify(values) !== JSON.stringify(plugin.settings));

  function show(id: string) {
    shown = id;
    error = "";
    const p = plugins.find((x) => x.id === id);
    values = p ? structuredClone($state.snapshot(p.settings)) : {};
  }

  // Runs a change, then shows the new list
  async function change(what: string, run: () => Promise<PluginInfo[]>, after?: (list: PluginInfo[]) => void) {
    busy = what;
    error = "";
    try {
      plugins = await run();
      after?.(plugins);
      onChanged();
    } catch (err) {
      error = String(err);
    } finally {
      busy = "";
    }
  }

  function add(link: boolean, from: string) {
    const before = new Set(plugins.map((p) => p.id));
    change(link ? "Linking…" : "Installing…", () => invoke("plugin_add", { source: from, link }), (list) => {
      source = "";
      const added = list.find((p) => !before.has(p.id));
      if (added) show(added.id);
    });
  }

  async function linkFolder() {
    const folder = await open({ directory: true, title: "A plugin's folder (the one with plugin.toml)" });
    if (typeof folder === "string") add(true, folder);
  }

  function edit(id: string, what: "enable" | "disable" | "update") {
    const label = { enable: "Turning on…", disable: "Turning off…", update: "Updating…" }[what];
    change(label, () => invoke("plugin_edit", { id, change: what }), () => {
      if (what === "update") {
        const { [id]: _, ...rest } = updates;
        onUpdates(rest);
      }
      show(id);
    });
  }

  async function remove(p: PluginInfo) {
    const what = p.linked ? "Its folder stays where it is." : "Its files are deleted.";
    if (!(await ask(`${what} Its settings are forgotten.`, { title: `Remove ${p.name}?`, kind: "warning", okLabel: "Remove" }))) return;
    change("Removing…", () => invoke("plugin_edit", { id: p.id, change: "remove" }), () => show("add"));
  }

  function saveSettings() {
    if (!plugin) return;
    const id = plugin.id;
    change("Saving…", () => invoke("plugin_save_settings", { id, settings: $state.snapshot(values) }), () => show(id));
  }

  async function checkUpdates() {
    busy = "Checking for updates…";
    try {
      onUpdates(await invoke<Record<string, string>>("plugins_check_updates"));
    } finally {
      busy = "";
    }
  }

  // A README's own images are in the plugin's folder: thumbdeck reads them in
  $effect(() => {
    const el = readmeEl;
    const folder = plugin?.folder;
    void plugin?.readme;
    if (!el || !folder) return;
    tick().then(() => {
      for (const img of el.querySelectorAll("img")) {
        const src = img.getAttribute("src") ?? "";
        if (!src || /^([a-z]+:|\/\/)/i.test(src)) continue;
        img.removeAttribute("src");
        invoke<string>("readme_image", { path: folder, src }).then((url) => (img.src = url)).catch(() => (img.alt = src));
      }
    });
  });

  onMount(async () => {
    plugins = await invoke<PluginInfo[]>("plugins_list");
    loaded = true;
    show(plugins[0]?.id ?? "add");
  });

  const status = (p: PluginInfo) => (p.problems.length ? "broken" : p.enabled ? "on" : "off");
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" onclick={(e) => e.target === e.currentTarget && !busy && onClose()}>
  <div class="dialog settings">
    <h2><span class="dot purple"></span>Settings <span class="for">Esc to close</span></h2>
    <div class="body">
      <nav class="list">
        <h3>Plugins</h3>
        {#each plugins as p (p.id)}
          <button class="item" class:on={shown === p.id} onclick={() => show(p.id)}>
            <span class="state {status(p)}" title={{ on: "on", off: "off", broken: "not loaded" }[status(p)]}>
              {status(p) === "broken" ? "!" : status(p) === "on" ? "●" : "○"}
            </span>
            <span class="pname">{p.name}</span>
            {#if updates[p.id]}<span class="new" title="A newer release: {updates[p.id]}">↑</span>{/if}
            <span class="ver">{p.version ?? ""}</span>
          </button>
        {/each}
        {#if loaded && plugins.length === 0}<p class="hint none">No plugins yet.</p>{/if}
        <button class="item add" class:on={shown === "add"} onclick={() => show("add")}>+ Add a plugin</button>
        <button class="ghost small check" disabled={!!busy || plugins.length === 0} onclick={checkUpdates}>Check for updates</button>
      </nav>

      <section class="detail">
        {#if busy}<p class="busy">{busy}</p>{/if}
        {#if error}<p class="hint problem">{error}</p>{/if}

        {#if shown === "add"}
          <h3>Add a plugin</h3>
          <form class="add-form" onsubmit={(e) => { e.preventDefault(); if (source.trim()) add(false, source); }}>
            <label>From a git URL
              <input bind:value={source} placeholder="https://github.com/someone/thumbdeck-pomodoro" {@attach (el) => el.focus()} />
            </label>
            <p class="hint">Its latest release is installed. For a plugin in a folder of a repository:
              <code>url#plugins/django</code>.</p>
            <div class="buttons"><button type="submit" class="primary" disabled={!source.trim() || !!busy}>Install</button></div>
          </form>
          <div class="or">
            <p>Or use a folder on disk, as it is: for a plugin you're writing (it isn't copied).</p>
            <button class="ghost" disabled={!!busy} onclick={linkFolder}>Use a folder…</button>
          </div>
          <p class="hint trust">A plugin runs with your rights, like an editor plugin: it can read and change your files
            and run commands. Install plugins you trust.</p>
        {:else if plugin}
          <header>
            <div>
              <h3 class="title">{plugin.name} <span class="ver">{plugin.version ?? ""}</span></h3>
              {#if plugin.description}<p class="desc">{plugin.description}</p>{/if}
              <p class="hint src">{plugin.linked ? "linked from" : "from"} {plugin.source}{plugin.tag ? ` · ${plugin.tag}` : ""}</p>
            </div>
            <span class="spacer"></span>
            {#if updates[plugin.id]}
              <button class="primary" disabled={!!busy} onclick={() => edit(plugin.id, "update")}>Update to {updates[plugin.id]}</button>
            {/if}
            <button class="ghost" disabled={!!busy} onclick={() => edit(plugin.id, plugin.enabled ? "disable" : "enable")}>
              {plugin.enabled ? "Turn off" : "Turn on"}
            </button>
          </header>

          {#if plugin.problems.length}
            <div class="problems">
              <p>It isn't loaded:</p>
              <ul>{#each plugin.problems as p}<li>{p}</li>{/each}</ul>
            </div>
          {:else if plugin.adds.length}
            <p class="adds">Adds {plugin.adds.join(" · ")}</p>
          {/if}

          {#if plugin.fields.length}
            <form class="fields" onsubmit={(e) => { e.preventDefault(); saveSettings(); }}>
              <h3>Its settings</h3>
              <FieldsForm fields={plugin.fields} bind:values />
              <div class="buttons">
                <button type="button" class="ghost" disabled={!changed} onclick={() => show(plugin.id)}>Undo</button>
                <button type="submit" class="primary" disabled={!changed || !!busy}>Save</button>
              </div>
            </form>
          {/if}

          {#if plugin.readme}
            <article class="readme" bind:this={readmeEl}>{@html marked.parse(plugin.readme)}</article>
          {/if}

          <div class="foot">
            <span class="hint">{plugin.folder}</span>
            <span class="spacer"></span>
            <button class="ghost danger" disabled={!!busy} onclick={() => remove(plugin)}>Remove</button>
          </div>
        {/if}
      </section>
    </div>
  </div>
</div>

<style>
  .settings { width: min(980px, 94vw); height: min(680px, 88vh); gap: 10px; }
  h2 { display: flex; align-items: center; gap: 8px; margin: 0; font: 600 13px var(--mono); }
  h3 { margin: 4px 0 8px; font: 500 11px var(--mono); color: var(--grey); text-transform: uppercase; letter-spacing: 0.08em; }
  .body { flex: 1; min-height: 0; display: grid; grid-template-columns: 240px 1fr; gap: 12px; }
  .list { display: flex; flex-direction: column; gap: 2px; overflow: auto; border-right: 1px solid var(--bg2); padding-right: 10px; }
  .item { display: flex; align-items: center; gap: 8px; padding: 6px 8px; border-radius: 8px; text-align: left; font: 13px var(--mono); }
  .item:hover { background: var(--bg1); }
  .item.on { background: var(--bg2); }
  .item.add { color: var(--orange); margin-top: 6px; }
  .pname { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ver { color: var(--grey); font: 400 11px var(--mono); }
  .state { width: 12px; text-align: center; }
  .state.on { color: var(--green); } .state.off { color: var(--grey); } .state.broken { color: var(--red); font-weight: 700; }
  .new { color: var(--green); font-weight: 700; }
  .none { padding: 4px 8px; margin: 0; }
  .check { margin-top: auto; align-self: flex-start; }
  .small { padding: 5px 10px; }

  .detail { min-width: 0; overflow: auto; display: flex; flex-direction: column; gap: 12px; padding-right: 4px; }
  header { display: flex; align-items: flex-start; gap: 8px; }
  header button { white-space: nowrap; }
  .title { margin: 0; font: 600 16px var(--mono); color: var(--fg); text-transform: none; letter-spacing: 0; }
  .desc { margin: 4px 0 0; }
  .src { margin: 4px 0 0; word-break: break-all; }
  .spacer { flex: 1; }
  .adds { margin: 0; color: var(--fg); font-size: 13px; }
  .problems { border: 1px solid color-mix(in srgb, var(--red) 40%, var(--bg2)); border-radius: 8px; padding: 8px 12px; color: var(--red); font-size: 13px; }
  .problems p { margin: 0; }
  .problems ul { margin: 6px 0 0; padding-left: 18px; }
  .fields { display: flex; flex-direction: column; gap: 10px; border-top: 1px solid var(--bg2); padding-top: 10px; }
  .readme { border-top: 1px solid var(--bg2); padding-top: 4px; font-size: 13.5px; }
  .readme :global(img) { max-width: 100%; }
  .readme :global(code) { font-family: var(--mono); background: var(--bg1); padding: 1px 5px; border-radius: 4px; color: var(--aqua); }
  .readme :global(pre) { background: var(--bg1); padding: 12px; border-radius: 8px; overflow: auto; }
  .readme :global(pre code) { background: none; padding: 0; color: var(--fg); }
  .readme :global(a) { color: var(--blue); }
  .foot { display: flex; align-items: center; gap: 8px; margin-top: auto; border-top: 1px solid var(--bg2); padding-top: 10px; }
  .foot .hint { font: 11px var(--mono); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .danger { color: var(--red); }
  .busy { margin: 0; color: var(--orange); font: 12px var(--mono); }
  .problem { color: var(--red); margin: 0; }
  .add-form { display: flex; flex-direction: column; gap: 10px; }
  .or { border-top: 1px solid var(--bg2); padding-top: 12px; font-size: 13px; }
  .or p { margin: 0 0 8px; }
  .trust { margin-top: auto; }
  code { font-family: var(--mono); color: var(--aqua); }
</style>
