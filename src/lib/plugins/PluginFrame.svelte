<script lang="ts">
  // A plugin's page in a frame (docs/plugin-spec.md): answers its calls to window.thumbdeck
  // (files, commands and storage go to the Rust side with the plugin and project attached),
  // sends it events, and gives it keys: its declared actions, or the raw key. Offers
  // handleKey / keymap like the built-in tabs.
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { ask } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";
  import { bind, keyName, type Keymap } from "../keys/keys.ts";
  import KeyHelp from "../keys/KeyHelp.svelte";
  import type { ReviewRequest } from "../review/types.ts";
  import type { FrameInfo, Host } from "./types.ts";
  import { frameUrl } from "./frame.ts";

  let { info, frameKey, surface, surfaceId, project, setup, active, visible, thumbdeck, host,
        say, onActivate, onRelease, onEditSetup, openReview, onSaveSetup, page, autofocus = false, lines = null, data = undefined }: {
    info: FrameInfo;
    /** Stable name of this frame in the page (for badges) */
    frameKey: string;
    surface: "tab" | "panel" | "page" | "view";
    surfaceId: string;
    project: { path: string; name: string; branch: string | null } | null;
    setup: any;
    /** Has the keyboard */
    active: boolean;
    /** Shown (a hidden frame is kept, not reloaded) */
    visible: boolean;
    /** thumbdeck's version */
    thumbdeck: string;
    host: Host;
    say: (text: string, error?: boolean) => void;
    onActivate: () => void;
    onRelease: () => void;
    onEditSetup: () => void;
    openReview: (r: ReviewRequest) => void;
    /** A setup page's frame: its setup.save */
    onSaveSetup?: (setup: any) => void;
    /** Another of the plugin's pages than info.page (a setup page) */
    page?: string;
    /** Take the focus when loaded (a setup page in a dialog) */
    autofocus?: boolean;
    /** A panel: how many lines tall (null: as tall as its page) */
    lines?: number | null;
    /** A page's data (from openPage) */
    data?: unknown;
  } = $props();

  let iframe = $state<HTMLIFrameElement | null>(null);
  let help = $state(false);

  // The page's address carries its context; it doesn't change while the frame lives
  // svelte-ignore state_referenced_locally
  const src = frameUrl(page ? { ...info, page } : info, { surface, id: surfaceId, project, data, api: 1, thumbdeck });

  // ------------------------------------------------------------ talking to the frame
  // Plain copies: the page's state (a setup, say) is a proxy that can't be sent as it is
  function post(message: object) {
    iframe?.contentWindow?.postMessage($state.snapshot({ __td: 1, ...message }), "*");
  }
  const event = (name: string, data: unknown = null) => post({ kind: "event", name, data });

  // Calls to the plugin's own functions (a review's diff loader)
  let nextCall = 1;
  const waiting = new Map<number, { resolve: (v: any) => void; reject: (e: Error) => void }>();
  function callFrame(callback: number, args: unknown[] = []): Promise<any> {
    const id = nextCall++;
    return new Promise((resolve, reject) => {
      waiting.set(id, { resolve, reject });
      post({ kind: "call", id, callback, args });
    });
  }

  // ------------------------------------------------------------ keys
  const maps = $derived(info.keymaps);
  let mapName = $state<string | null>(null);
  const current = $derived(maps.find(([n]) => n === mapName)?.[1] ?? maps[0]?.[1] ?? null);

  /** Its keys right now, with thumbdeck's own (setup, help, leave) for ? */
  export function keymap(): Keymap {
    const own = current?.bindings ?? [];
    const bound = new Set(own.flatMap((b) => b.keys));
    const extra = [bind("S", "setup", "set up this tab"), bind("?", "help", "all keys"), bind(["q", "Escape"], "leave", "give the keyboard back to thumbdeck")]
      .map((b) => ({ ...b, keys: b.keys.filter((k) => !bound.has(k)) }))
      .filter((b) => b.keys.length && (surface === "tab" || b.action !== "setup"));
    return { name: current?.name ?? info.name.toUpperCase(), bindings: [...own, ...extra] };
  }

  export function handleKey(e: KeyboardEvent): boolean {
    if (help) {
      help = false; // any key closes the help
      return true;
    }
    const key = keyName(e);
    const b = current?.bindings.find((x) => x.keys.includes(key));
    if (b) event("key", { action: b.action, key });
    else if (key === "?") help = true;
    else if (key === "S" && surface === "tab") onEditSetup();
    else if (key === "Escape" || key === "q") onRelease();
    else {
      const { key: k, code, ctrlKey, metaKey, altKey, shiftKey } = e;
      post({ kind: "rawkey", key: { key: k, code, ctrlKey, metaKey, altKey, shiftKey } });
    }
    return true;
  }

  // ------------------------------------------------------------ the calls it makes
  const RUST = new Set(["fs.read", "fs.write", "fs.stat", "fs.list", "exec", "storage.get", "storage.set", "storage.remove", "settings.get", "ui.notify", "ui.openUrl"]);
  const myRuns = new Set<number>();
  const watches = new Map<number, ReturnType<typeof setInterval>>();
  let nextWatch = 1;

  const rust = (method: string, params: unknown) =>
    invoke("plugin_call", { plugin: info.plugin, project: project?.path ?? null, method, params: params ?? null });

  function watch(path: string): number {
    const id = nextWatch++;
    let last: string | undefined;
    const look = async () => {
      let now: string;
      try {
        now = JSON.stringify(await rust("fs.stat", { path }));
      } catch {
        return;
      }
      if (last !== undefined && now !== last) {
        const kind = last === "null" ? "added" : now === "null" ? "removed" : "changed";
        event("fs.watch", { id, change: { path, kind } });
      }
      last = now;
    };
    look();
    watches.set(id, setInterval(look, 1000));
    return id;
  }

  async function handle(method: string, p: any): Promise<unknown> {
    if (RUST.has(method)) return rust(method, p);
    switch (method) {
      case "projects.selected": return host.selected();
      case "projects.list": return host.projects();
      case "setup.get": return setup;
      case "setup.edit": return onEditSetup();
      case "setup.save":
        if (!onSaveSetup) throw new Error("setup.save is for a tab's setup_page");
        return onSaveSetup(p.value);
      case "run": {
        if (!project) throw new Error("this frame has no project to run in");
        const id = await host.startRun(project, p.command, p.label || p.command, info.plugin);
        myRuns.add(id);
        return id;
      }
      case "run.stop": return host.stopRun(p.id);
      case "tmux": {
        if (!project) throw new Error("this frame has no project");
        if (!p.window) throw new Error("tmux needs a window name");
        const message = await invoke<string>("run_in_tmux", { path: project.path, name: project.name, window: p.window, command: p.command });
        if (p.show) await invoke("show_tmux_window", { path: project.path, name: project.name, window: p.window });
        return message;
      }
      case "ui.say": return say(p.text, p.error);
      case "ui.confirm":
        return ask(p.question, { title: info.plugin_name, kind: "warning", okLabel: p.yes ?? "Yes", cancelLabel: p.no ?? "No" });
      case "ui.openReview": {
        const diff = p.diff;
        openReview({
          title: p.title ?? "Changes", subtitle: p.subtitle, startFile: p.startFile,
          viewedKey: `plugin:${info.plugin}:${p.viewedKey ?? p.title}`,
          load: typeof diff === "string" ? async () => diff : () => callFrame(diff.callback),
        });
        return;
      }
      case "ui.badge": return host.badge(frameKey, p.value);
      case "ui.mood":
        if (!["waiting", "working", "reading", "review"].includes(p.signal)) throw new Error(`the avatar has no signal ${p.signal}`);
        return host.mood(frameKey, p.signal, p.value);
      case "ui.openPage":
        if (!info.plugin) throw new Error("no plugin");
        return host.openPage(info.plugin, p.id, p.data ?? null, project);
      case "ui.close":
        if (surface !== "page") throw new Error("only a page can close itself");
        return host.closePage();
      case "ui.status": return host.status(info.plugin, p.text ?? null);
      case "keys.use":
        if (!maps.some(([n]) => n === p.map)) throw new Error(`there's no keymap ${p.map} for this ${surface}`);
        mapName = p.map;
        return;
      case "keys.take": return onActivate();
      case "keys.release": return onRelease();
      case "fs.watch": return watch(p.path);
      case "fs.unwatch": clearInterval(watches.get(p.id)); watches.delete(p.id); return;
      case "actions.refresh": return host.refreshToolkit(info.plugin);
      case "backend.call":
        return invoke("plugin_backend_call", { plugin: info.plugin, project: project?.path ?? null, method: p.method, params: p.params ?? null });
      default: throw new Error(`there's no ${method} in the plugin API`);
    }
  }

  async function onMessage(e: MessageEvent) {
    if (!iframe || e.source !== iframe.contentWindow) return;
    const m = e.data;
    if (!m || m.__td !== 1) return;
    if (m.kind === "call") {
      try {
        post({ kind: "reply", id: m.id, result: (await handle(m.method, m.params)) ?? null });
      } catch (err) {
        post({ kind: "reply", id: m.id, error: String((err as Error)?.message ?? err) });
      }
    } else if (m.kind === "reply") {
      const w = waiting.get(m.id);
      waiting.delete(m.id);
      if ("error" in m) w?.reject(new Error(m.error));
      else w?.resolve(m.result);
    } else if (m.kind === "key") {
      // A key pressed with the focus in the frame: the page handles it like any other
      window.dispatchEvent(new KeyboardEvent("keydown", { ...m.key, bubbles: true, cancelable: true }));
    } else if (m.kind === "log") {
      invoke("plugin_log", { plugin: info.plugin, level: String(m.level), text: String(m.text) });
    } else if (m.kind === "activate") {
      onActivate();
    } else if (m.kind === "ready") {
      event("keyboard", active);
    } else if (m.kind === "height") {
      contentHeight = Number(m.px) || 0;
    }
  }

  // ------------------------------------------------------------ events it gets
  let wasActive: boolean | null = null;
  $effect(() => {
    if (wasActive !== null && active !== wasActive) event("keyboard", active);
    wasActive = active;
  });
  let wasVisible: boolean | null = null;
  $effect(() => {
    if (wasVisible !== null && visible !== wasVisible) event(visible ? "shown" : "hidden");
    wasVisible = visible;
  });
  // An app-wide frame follows the selected project
  let lastProject: string | null | undefined = undefined;
  $effect(() => {
    const path = project?.path ?? null;
    if (lastProject !== undefined && path !== lastProject && (surface === "view" || surface === "panel")) event("project", project);
    lastProject = path;
  });

  // A panel's height: its page's (up to half the column), or a number of lines
  let contentHeight = $state(0);
  const panelHeight = $derived(
    surface !== "panel" ? null : lines ? `${lines * 18 + 8}px` : `min(${Math.max(contentHeight, 24)}px, 45vh)`,
  );

  let lastSetup: string | null = null;
  $effect(() => {
    const now = JSON.stringify(setup);
    if (lastSetup !== null && now !== lastSetup) event("setup", setup);
    lastSetup = now;
  });

  onMount(() => {
    window.addEventListener("message", onMessage);
    const focus = () => event("focus", true);
    const blur = () => event("focus", false);
    window.addEventListener("focus", focus);
    window.addEventListener("blur", blur);
    const output = listen<{ id: number; line: string; stderr: boolean }>("run-output", (e) => {
      if (myRuns.has(e.payload.id)) event("run.output", e.payload);
    });
    const exit = listen<{ id: number; code: number | null }>("run-exit", (e) => {
      const { id, code } = e.payload;
      if (myRuns.has(id)) {
        event("run.exit", { id, code: code ?? -1 });
        myRuns.delete(id);
      }
      const r = host.run(id);
      if (r && project && r.projectPath === project.path) event("run", { id, label: r.label, code: code ?? -1 });
    });
    const backend = listen<{ plugin: string; name: string; data: unknown }>("plugin-backend-event", (e) => {
      if (e.payload.plugin === info.plugin) event("backend", { name: e.payload.name, data: e.payload.data });
    });
    return () => {
      host.forget(frameKey);
      backend.then((f) => f());
      window.removeEventListener("message", onMessage);
      window.removeEventListener("focus", focus);
      window.removeEventListener("blur", blur);
      output.then((f) => f());
      exit.then((f) => f());
      for (const t of watches.values()) clearInterval(t);
    };
  });
</script>

<div class="frame" class:active class:hidden={!visible} class:panel={surface === "panel"} style:height={panelHeight}>
  <iframe bind:this={iframe} {src} title={info.name} allow="clipboard-read; clipboard-write"
          onload={() => autofocus && iframe?.focus()}></iframe>
  {#if help}<KeyHelp map={keymap()} note={info.plugin_name} />{/if}
</div>

<style>
  .frame { position: relative; flex: 1; min-height: 0; display: flex; border-top: 2px solid transparent; }
  .frame.active { border-top-color: var(--orange); }
  .frame.hidden { display: none; }
  .frame.panel { flex: none; border-top: 0; }
  iframe { flex: 1; border: 0; width: 100%; height: 100%; background: var(--bg0); }
</style>
