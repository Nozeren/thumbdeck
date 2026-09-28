<script lang="ts">
  import "$lib/theme.css";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { marked } from "marked";
  import { onMount, tick, untrack } from "svelte";
  import { ask, open } from "@tauri-apps/plugin-dialog";
  import type { Action, CustomAction, Details, Extension, Project, ProjectList, Run, Tab, TabInfo, Update } from "$lib/types";
  import type { Host } from "$lib/plugins/types";
  import PluginFrame from "$lib/plugins/PluginFrame.svelte";
  import PluginSetupForm from "$lib/plugins/PluginSetupForm.svelte";
  import { getVersion } from "@tauri-apps/api/app";
  import { extensions, type TabExports } from "$lib/extensions";
  import Avatar from "$lib/avatar/Avatar.svelte";
  import ReviewPage from "$lib/review/ReviewPage.svelte";
  import StatusBar from "$lib/StatusBar.svelte";
  import SettingsDialog from "$lib/settings/SettingsDialog.svelte";
  import { actionFor, keysLabel } from "$lib/keys/keys";
  import { DIALOG, FILTER, MAIN, REVIEW, TOOLKIT } from "$lib/keys/maps";
  import type { ReviewRequest } from "$lib/review/types";
  import { combine, pickMood } from "$lib/avatar/mood";
  import { avatarSignals } from "$lib/avatar/signals.svelte";

  let projects = $state<Project[]>([]);
  let roots = $state<string[]>([]);
  // Folded sections of the tree (not saved: each start folds all but the one in use)
  let collapsed = $state<string[]>([]);
  let addMenu = $state(false);
  // Form for adding / editing a custom action (null when closed)
  let form = $state<CustomAction | null>(null);
  let showHiddenActions = $state(false);
  // Short message at the bottom (e.g. after opening a project in tmux)
  let toast = $state<{ text: string; error: boolean } | null>(null);
  let toastTimer: ReturnType<typeof setTimeout> | undefined;
  function say(text: string, error = false) {
    toast = { text, error };
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toast = null), error ? 8000 : 4000); // shown in the status bar
  }

  async function openInTmux() {
    if (!selected) return;
    try {
      say(await invoke<string>("open_in_tmux", { path: selected.path, name: selected.name }));
    } catch (err) {
      say(String(err), true);
    }
  }
  let addMenuEl = $state<HTMLElement | null>(null);

  // Close the + menu on a click anywhere else, or on Esc
  function closeMenuOutside(e: MouseEvent) {
    if (addMenu && addMenuEl && !addMenuEl.contains(e.target as Node)) addMenu = false;
    if (tabMenu && tabMenuEl && !tabMenuEl.contains(e.target as Node)) tabMenu = false;
  }

  // A click outside the center gives the keyboard back to thumbdeck
  function mouseDown(e: MouseEvent) {
    if (keysToTab && centerEl && !centerEl.contains(e.target as Node)) keysToTab = false;
  }
  // ------------------------------------------------------------ extension tabs
  let available = $state<Extension[]>([]);
  // Extension tab shown in the center (its index in details.tabs); null: README or a run
  let shownTab = $state<number | null>(null);
  // The shown tab has the keyboard (a click in it, or its number key); Esc gives it back
  let keysToTab = $state(false);
  let tabRef = $state<TabExports | null>(null);
  // The center takes the whole window (z): more room for a log, a README, a run's output
  let wide = $state(false);
  let tabMenu = $state(false);
  let tabMenuEl = $state<HTMLElement | null>(null);
  let centerEl = $state<HTMLElement | null>(null);
  // Setup form for a tab: index null for a new one (saved on Add)
  let setupForm = $state<{ index: number | null; tab: TabInfo } | null>(null);

  // ------------------------------------------------------------ plugin tabs (frames)
  // A plugin tab's frame is kept once shown (switching back is instant) and unloaded after
  // 10 minutes hidden. Its key: project, position, plugin and tab.
  interface LiveFrame { key: string; project: Project; index: number; tab: TabInfo; lastShown: number }
  let liveFrames = $state<LiveFrame[]>([]);
  let frameRefs = $state<Record<string, TabExports | null>>({});
  let badges = $state<Record<string, string | null>>({});
  let version = $state("");
  const frameKeyOf = (path: string, i: number, t: Tab) => `${path}|${i}|${t.plugin}:${t.extension}`;
  const shownFrameKey = $derived.by(() => {
    if (!tabShown || !selected || !details || shownTab === null) return null;
    const t = details.tabs[shownTab];
    return t?.frame ? frameKeyOf(selected.path, shownTab, t) : null;
  });
  // The shown tab's keys: a plugin frame's, or a built-in tab's
  const activeTab = $derived(shownFrameKey ? (frameRefs[shownFrameKey] ?? null) : tabRef);

  // Start the shown plugin tab's frame, or mark it shown again
  $effect(() => {
    const key = shownFrameKey;
    if (!key || !selected || !details || shownTab === null) return;
    const [project, index, tab] = [selected, shownTab, details.tabs[shownTab]];
    untrack(() => {
      const f = liveFrames.find((x) => x.key === key);
      if (f) f.lastShown = Date.now();
      else liveFrames.push({ key, project, index, tab, lastShown: Date.now() });
    });
  });
  // New details for a project: its frames get their tab's new setup, or go with their tab
  $effect(() => {
    const d = details;
    const path = selected?.path;
    if (!d || !path) return;
    untrack(() => {
      liveFrames = liveFrames.filter((f) => {
        if (f.project.path !== path) return true;
        const t = d.tabs[f.index];
        if (!t?.frame || frameKeyOf(path, f.index, t) !== f.key) return false;
        f.tab = t;
        return true;
      });
    });
  });
  function unloadIdleFrames() {
    const now = Date.now();
    liveFrames = liveFrames.filter((f) => f.key === shownFrameKey || now - f.lastShown < 10 * 60_000);
  }

  // What plugin frames can ask of the page
  const host: Host = {
    projects: () => projects.filter((p) => !p.hidden).map(({ path, name, branch }) => ({ path, name, branch })),
    selected: () => (selected ? { path: selected.path, name: selected.name, branch: selected.branch } : null),
    startRun: (p, command, label, source) => startRun(p, command, label, source, false),
    stopRun: (id) => void invoke("stop_run", { id }),
    run: (id) => {
      const r = runs.find((x) => x.id === id);
      return r ? { label: r.label, projectPath: r.projectPath } : null;
    },
    refreshToolkit: async (plugin) => {
      await invoke("plugin_refresh_actions", { plugin });
      if (selected) details = await invoke<Details>("project_details", { path: selected.path });
    },
    badge: (key, value) => (badges[key] = value),
    mood: (key, signal, value) => (pluginMoods[key] = { ...pluginMoods[key], [signal]: value }),
    forget: (key) => {
      delete pluginMoods[key];
      delete badges[key];
    },
  };
  // What plugin frames and backends tell the avatar: sender -> signal -> value
  let pluginMoods = $state<Record<string, Record<string, unknown>>>({});

  function showTab(i: number | null, keys = true) {
    shownRun = null;
    shownTab = i;
    keysToTab = i !== null && keys;
  }

  async function addTab(extension: string) {
    tabMenu = false;
    try {
      setupForm = { index: null, tab: await invoke<TabInfo>("new_tab", { extension }) };
    } catch (err) {
      say(String(err), true);
    }
  }

  async function openTabMenu() {
    tabMenu = !tabMenu;
    if (tabMenu && selected) available = await invoke<Extension[]>("tabs_available", { path: selected.path });
  }

  async function saveTab(change: "add" | "save" | "remove", setup: unknown = null) {
    if (!selected || !setupForm) return;
    const { index, tab } = setupForm;
    setupForm = null;
    details = await invoke<Details>("edit_tab", {
      path: selected.path, change, index: index ?? 0,
      tab: setup === null ? null : { extension: tab.extension, plugin: tab.plugin, setup },
    });
    if (change === "add") showTab(details.tabs.length - 1);
    else if (change === "remove") showTab(null);
  }

  // ------------------------------------------------------------ settings (plugins)
  let settingsOpen = $state(false);
  // Plugins with a newer release: id -> its tag (looked for a little after start)
  let pluginUpdates = $state<Record<string, string>>({});

  function openSettings() {
    addMenu = false;
    settingsOpen = true;
  }

  // Plugins changed: the Toolkit and the project icons may have too
  async function pluginsChanged() {
    applyList(await invoke<ProjectList>("list_projects"));
    if (selected) details = await invoke<Details>("project_details", { path: selected.path });
  }

  // ------------------------------------------------------------ keyboard
  let hints = $state(false); // Space pressed: Toolkit buttons show their letters
  let helpOpen = $state(false);
  let filterEl = $state<HTMLInputElement | null>(null);
  const HINT_KEYS = "asdfghjkl;qwertyuiopzxcvbnm";

  // Projects in the order the tree shows them (a pinned project counts once, where it's pinned)
  const navOrder = $derived.by(() => {
    const seen = new Set<string>();
    const out: Project[] = [];
    for (const sec of sections) {
      if (sec.id === "hidden") continue;
      for (const p of sec.items) if (!seen.has(p.path)) (seen.add(p.path), out.push(p));
    }
    return out;
  });

  function move(delta: number | "first" | "last") {
    if (!navOrder.length) return;
    const i = navOrder.findIndex((p) => p.path === selected?.path);
    const next =
      delta === "first" ? 0 : delta === "last" ? navOrder.length - 1
      : Math.min(navOrder.length - 1, Math.max(0, (i < 0 ? 0 : i) + delta));
    const p = navOrder[next];
    collapsed = collapsed.filter((id) => id !== sectionOf(p)); // unfold its section
    select(p);
  }

  function cycleRuns(delta: number) {
    const list = runTabs; // oldest first
    if (!list.length) return;
    const i = list.findIndex((r) => r.id === shownRun);
    showRun(list[(i < 0 ? list.length - 1 : i + delta + list.length) % list.length].id);
  }

  function onKey(e: KeyboardEvent) {
    // The review page covers everything: all keys are its own (Esc and q close it)
    if (review) {
      if (!e.ctrlKey && !e.metaKey && !e.altKey) e.preventDefault();
      reviewRef?.handleKey(e);
      return;
    }
    const typing = e.target instanceof HTMLElement && e.target.closest("input, textarea, select");
    const dialog = form || setupForm || helpOpen || updateDialog || settingsOpen;
    // 1 README, 2… extension tabs
    if (/^[1-9]$/.test(e.key) && !typing && !dialog && !e.ctrlKey && !e.metaKey && !e.altKey && details) {
      const n = Number(e.key);
      const runIndex = n - 2 - details.tabs.length;
      if (n === 1) showTab(null);
      else if (details.tabs[n - 2]) showTab(n - 2);
      else if (runTabs[runIndex]) showRun(runTabs[runIndex].id);
      e.preventDefault();
      return;
    }
    if (e.key === "z" && !typing && !dialog && !e.ctrlKey && !e.metaKey && !e.altKey) {
      wide = !wide;
      e.preventDefault();
      return;
    }
    // The shown tab has the keyboard: its keys, and nothing of thumbdeck's but Esc
    if (keysToTab && tabShown && activeTab && !typing && !dialog) {
      if (!activeTab.handleKey(e) && e.key === "Escape") keysToTab = false;
      e.preventDefault();
      return;
    }
    // Enter / Space on a button you reached with the keyboard (Tab) press it, not thumbdeck's
    // Enter (open in tmux) or Space (hints). A clicked button keeps the focus too, but not
    // :focus-visible, so after a click the shortcuts work as usual.
    const button = e.target instanceof HTMLElement ? e.target.closest("button") : null;
    if ((e.key === "Enter" || e.key === " ") && button?.matches(":focus-visible")) return;
    if (e.key === "Escape") {
      addMenu = false;
      tabMenu = false;
      form = null;
      setupForm = null;
      settingsOpen = false;
      if (updateState !== "installing") updateDialog = false;
      hints = false;
      helpOpen = false;
      if (typing) (e.target as HTMLElement).blur();
      return;
    }
    if (helpOpen && e.key === "?") return void (helpOpen = false);
    if (typing || dialog || e.ctrlKey || e.metaKey || e.altKey) return;

    if (hints) {
      e.preventDefault();
      hints = false;
      const i = HINT_KEYS.indexOf(e.key);
      const a = details?.actions[i];
      if (i >= 0 && a) run(a);
      return;
    }
    switch (actionFor(MAIN, e)) {
      case "toolkit": hints = (details?.actions.length ?? 0) > 0; break;
      case "down": move(1); break;
      case "up": move(-1); break;
      case "first": move("first"); break;
      case "last": move("last"); break;
      case "filter": filterEl?.focus(); break;
      case "pin": if (selected) edit("pin", selected.path); break;
      case "remove": if (selected) edit("remove", selected.path); break;
      case "add": if (selected) openForm(); break;
      case "stop": if (current && current.endedAt === null) stop(current); break;
      case "run": cycleRuns(e.key === "[" ? -1 : 1); break;
      case "help": helpOpen = !helpOpen; break;
      case "outside": openInTmux(); break;
      case "settings": openSettings(); break;
      default: return;
    }
    e.preventDefault();
  }

  // Enter in the filter opens the first match
  function filterKey(e: KeyboardEvent) {
    if (e.key === "Enter" && navOrder.length) {
      select(navOrder[0]);
      filter = "";
      (e.target as HTMLElement).blur();
    }
  }

  let filter = $state("");
  let selected = $state<Project | null>(null);
  let details = $state<Details | null>(null);
  let runs = $state<Run[]>([]);
  let shownRun = $state<number | null>(null); // run whose output is in the center; null = README
  let now = $state(Date.now());
  let outputEl = $state<HTMLElement | null>(null);
  // macOS shows its window buttons over the top-left of the app (overlay title bar)
  const mac = navigator.userAgent.includes("Mac");

  const visible = $derived(
    projects.filter((p) => !p.hidden && p.name.toLowerCase().includes(filter.toLowerCase())),
  );
  // The project tree: Pinned, one section per scan folder, Added, Hidden
  const sections = $derived.by(() => {
    const match = (p: Project) => p.name.toLowerCase().includes(filter.toLowerCase());
    const shown = projects.filter((p) => !p.hidden && match(p));
    const list = [
      { id: "pinned", label: "★ Pinned", root: null as string | null, items: shown.filter((p) => p.pinned) },
      ...roots.map((r) => ({ id: r, label: home(r), root: r as string | null, items: shown.filter((p) => p.root === r) })),
      { id: "added", label: "Added", root: null, items: shown.filter((p) => p.root === null) },
      { id: "hidden", label: "Hidden", root: null, items: projects.filter((p) => p.hidden && match(p)) },
    ];
    // Scan folders always show (so they can be removed); the others only when they have projects
    return list.filter((sec) => sec.items.length > 0 || sec.root !== null);
  });
  const folded = (id: string) => collapsed.includes(id) && !filter;

  const icons: Record<string, [string, string]> = {
    django: ["", "green"], python: ["", "yellow"], android: ["", "aqua"],
    node: ["", "green"], tauri: ["", "orange"], rust: ["", "orange"],
    go: ["", "blue"], nvim: ["", "green"], folder: ["", "grey"],
  };

  const home = (path: string) => path.replace(/^\/(home|Users)\/[^/]+/, "~");

  // Section of the tree a project sits in
  const sectionOf = (p: Project) => p.root ?? "added";

  function applyList(list: ProjectList, starting = false) {
    projects = list.projects;
    roots = list.roots;
    avatar = list.avatar;
    const shown = projects.filter((p) => !p.hidden);
    // Keep the selection if it's still visible; on start, reopen the last project used
    let next = selected && shown.find((p) => p.path === selected!.path);
    if (starting) {
      next = shown.find((p) => p.path === list.last) ?? shown.find((p) => p.pinned) ?? shown[0];
      // Fold every section except Pinned and the one the project is in
      const inUse = next ? sectionOf(next) : null;
      collapsed = [...roots, "added", "hidden"].filter((id) => id !== inUse);
    }
    if (next) {
      if (next.path !== selected?.path) select(next);
    } else {
      selected = null;
    }
  }

  function toggleFold(id: string) {
    collapsed = collapsed.includes(id) ? collapsed.filter((c) => c !== id) : [...collapsed, id];
  }

  async function edit(change: string, path: string) {
    applyList(await invoke<ProjectList>("edit_projects", { change, path }));
  }

  async function pickFolder(change: "add" | "add-root") {
    addMenu = false;
    const title = change === "add" ? "Add a project folder" : "Add a folder to scan for projects";
    const path = await open({ directory: true, title });
    if (typeof path === "string") await edit(change, path);
  }
  // ------------------------------------------------------------ updates
  let update = $state<Update | null>(null); // a newer release (checked on start)

  // ------------------------------------------------------------ the avatar (top bar)
  let review = $state<ReviewRequest | null>(null); // the review page, over everything
  let filtering = $state(false); // the filter box has the keyboard

  // A README's own images (docs/screenshot.png) are in the project's folder, which the page
  // can't reach: thumbdeck reads them and puts them in. Web images load as they are.
  let readmeEl = $state<HTMLElement | null>(null);
  $effect(() => {
    const el = readmeEl;
    const path = selected?.path;
    void details?.readme;
    if (!el || !path) return;
    tick().then(() => {
      for (const img of el.querySelectorAll("img")) {
        const src = img.getAttribute("src") ?? "";
        if (!src || /^([a-z]+:|\/\/)/i.test(src)) continue; // http(s):, data:, //host
        img.removeAttribute("src");
        invoke<string>("readme_image", { path, src })
          .then((url) => selected?.path === path && (img.src = url))
          .catch(() => (img.alt = `${img.alt || src} (can't show it)`));
      }
    });
  });

  // The selected project's branch and uncommitted files, for the status bar (looked at every 5s)
  let branchState = $state<{ name: string | null; ahead: number; behind: number; gone: boolean; changed: number } | null>(null);
  $effect(() => {
    const path = selected?.path;
    if (!path) return void (branchState = null);
    const look = () =>
      invoke<{ name: string | null; ahead: number; behind: number; gone: boolean; changed: number }>("branch_status", { path })
        .then((s) => (branchState = s))
        .catch(() => (branchState = null)); // not a git repo
    look();
    const timer = setInterval(() => !document.hidden && look(), 5000);
    return () => clearInterval(timer);
  });
  let reviewRef = $state<{ handleKey(e: KeyboardEvent): boolean } | null>(null);
  let avatar = $state(""); // the character (settings); "none": no character
  let avatarRef = $state<{ pickCharacter(): void } | null>(null);
  let lastInput = $state(Date.now());
  let claudeLive = $state<{ project: string; status: string }[]>([]);
  $effect(() => {
    const look = () =>
      invoke<{ cwd: string; status: string }[]>("claude_live")
        .then((l) => (claudeLive = l.map((c) => ({ project: c.cwd.split("/").filter(Boolean).at(-1) ?? c.cwd, status: c.status }))))
        .catch(() => (claudeLive = []));
    look();
    const timer = setInterval(look, 4000);
    const input = () => (lastInput = Date.now());
    window.addEventListener("keydown", input, true);
    window.addEventListener("pointerdown", input, true);
    return () => {
      clearInterval(timer);
      window.removeEventListener("keydown", input, true);
      window.removeEventListener("pointerdown", input, true);
    };
  });
  const avatarMood = $derived(
    pickMood(
      {
        runs,
        plugins: combine([
          ...Object.values(pluginMoods),
          // The built-in tabs', until they're plugins too
          {
            reading: avatarSignals.logsReading > 0,
            waiting: claudeLive.find((c) => c.status === "waiting") && `Claude in ${claudeLive.find((c) => c.status === "waiting")!.project}`,
            working: claudeLive.find((c) => c.status === "busy") && `Claude in ${claudeLive.find((c) => c.status === "busy")!.project}`,
          },
        ]),
        update: update?.version ?? null,
        lastInput,
      },
      now,
    ),
  );
  let updateDialog = $state(false);
  let updateState = $state<"idle" | "installing" | "installed">("idle");
  let updateError = $state("");

  async function checkForUpdates() {
    addMenu = false;
    try {
      update = await invoke<Update | null>("check_update");
      if (update) updateDialog = true;
      else say("thumbdeck is up to date");
    } catch (err) {
      say(String(err), true);
    }
  }

  async function installUpdate() {
    updateState = "installing";
    updateError = "";
    try {
      await invoke<string>("install_update");
      updateState = "installed";
    } catch (err) {
      updateError = String(err);
      updateState = "idle";
    }
  }

  const projectRuns = $derived(runs.filter((r) => r.projectPath === selected?.path).toReversed());
  const current = $derived(runs.find((r) => r.id === shownRun) ?? null);
  // Runs whose tab you closed (they keep running; the Running panel opens them again)
  let closedRuns = $state<number[]>([]);
  // The project's runs as tabs, oldest first
  const runTabs = $derived(projectRuns.toReversed().filter((r) => !closedRuns.includes(r.id)));

  function showRun(id: number) {
    closedRuns = closedRuns.filter((x) => x !== id);
    shownRun = id;
    keysToTab = false;
  }

  function closeRun(r: Run) {
    const i = runTabs.findIndex((x) => x.id === r.id);
    closedRuns = [...closedRuns, r.id];
    if (shownRun === r.id) {
      const next = runTabs[i] ?? runTabs[i - 1]; // runTabs no longer has r
      if (next) shownRun = next.id;
      else showTab(shownTab !== null && details?.tabs[shownTab] ? shownTab : null, false);
    }
  }
  // An extension tab is in the center (not the README or a run's output)
  const tabShown = $derived(shownRun === null && shownTab !== null && !!details?.tabs[shownTab]);
  /** Who has the keyboard, for the status bar at the bottom */
  const barKeys = $derived(
    review ? REVIEW
    : form || setupForm || helpOpen || updateDialog || settingsOpen ? DIALOG
    : filtering ? FILTER
    : hints ? TOOLKIT
    : keysToTab && tabShown && activeTab ? activeTab.keymap()
    : MAIN,
  );
  // Toolkit grouped by where each action came from: yours, then one group per pack
  const groups = $derived.by(() => {
    const byGroup = new Map<string, Action[]>();
    for (const a of details?.actions ?? []) byGroup.set(a.source, [...(byGroup.get(a.source) ?? []), a]);
    return [...byGroup.values()].map((actions) => [actions[0].group, actions] as const);
  });

  async function select(p: Project) {
    selected = p;
    invoke("edit_projects", { change: "last", path: p.path }); // remembered for the next start
    shownRun = null;
    showTab(null);
    details = null;
    details = await invoke<Details>("project_details", { path: p.path });
  }

  async function editActions(change: string, id = "", action: CustomAction | null = null) {
    if (!selected) return;
    details = await invoke<Details>("edit_actions", { path: selected.path, change, id, action });
  }

  function openForm(a?: Action) {
    form = a
      ? { id: a.id.replace(/^custom:/, ""), name: a.label, command: a.command, confirm: a.confirm, tmux: a.tmux }
      : { id: "", name: "", command: "", confirm: false, tmux: null };
  }

  async function saveForm() {
    if (!form || !form.name.trim() || !form.command.trim()) return;
    const name = form.name.trim();
    // An empty window name means: named after the action
    const tmux = form.tmux === null ? null : form.tmux.trim() || name;
    await editActions("save", "", { ...form, name, command: form.command.trim(), tmux });
    form = null;
  }

  /** Start a command in a project: a tab with its output (shown when `show`), in Running */
  async function startRun(p: { path: string }, command: string, label: string, source: string, show: boolean) {
    const id = await invoke<number>("run_action", { path: p.path, command, label });
    runs.push({ id, projectPath: p.path, label, source, command, startedAt: Date.now(), endedAt: null, code: null, lines: [] });
    if (show) shownRun = id;
    return id;
  }

  async function run(a: Action) {
    if (!selected) return;
    if (a.confirm && !(await ask(`${a.command}`, { title: `Run "${a.label}"?`, kind: "warning", okLabel: "Run" }))) return;
    if (a.tmux) {
      // Runs in the project's tmux session; thumbdeck only says how it went
      try {
        say(await invoke<string>("run_in_tmux", { path: selected.path, name: selected.name, window: a.tmux, command: a.command }));
      } catch (err) {
        say(String(err), true);
      }
      return;
    }
    await startRun(selected, a.command, a.label, a.source, true);
  }

  function stop(r: Run) {
    invoke("stop_run", { id: r.id });
  }

  function elapsed(r: Run) {
    const s = Math.max(0, Math.floor(((r.endedAt ?? now) - r.startedAt) / 1000));
    return s < 60 ? `${s}s` : `${Math.floor(s / 60)}m ${s % 60}s`;
  }

  function status(r: Run) {
    if (r.endedAt === null) return "running";
    return r.code === 0 ? "done" : "failed";
  }

  onMount(() => {
    invoke<ProjectList>("list_projects").then((list) => applyList(list, true));
    getVersion().then((v) => (version = v));
    const idle = setInterval(unloadIdleFrames, 60_000);
    const unlistenOut = listen<{ id: number; line: string; stderr: boolean }>("run-output", async (e) => {
      const r = runs.find((x) => x.id === e.payload.id);
      if (!r) return;
      r.lines.push({ text: e.payload.line, stderr: e.payload.stderr });
      if (r.id === shownRun) {
        await tick();
        outputEl?.scrollTo({ top: outputEl.scrollHeight });
      }
    });
    const unlistenUpdate = listen<Update>("update-available", (e) => (update = e.payload));
    // A backend changed its Toolkit buttons, or has something to say
    const unlistenActions = listen<string>("plugin-actions-changed", async () => {
      if (selected) details = await invoke<Details>("project_details", { path: selected.path });
    });
    const unlistenBackendUi = listen<{ plugin: string; method: string; params: any }>("plugin-backend-ui", (e) => {
      const { plugin, method, params } = e.payload;
      if (method === "ui.say") say(String(params?.text ?? ""), !!params?.error);
      else if (method === "ui.mood") host.mood(`backend:${plugin}`, String(params?.signal), params?.value ?? null);
      else if (method === "ui.badge") {
        for (const f of liveFrames) if (f.tab.plugin === plugin) badges[f.key] = params?.value == null ? null : String(params.value);
      }
    });
    const unlistenPlugins = listen<Record<string, string>>("plugin-updates", (e) => {
      pluginUpdates = e.payload;
      const n = Object.keys(e.payload).length;
      say(`${n} plugin${n === 1 ? " has" : "s have"} a newer release: Settings (,) › Plugins`);
    });
    const unlistenExit = listen<{ id: number; code: number | null }>("run-exit", (e) => {
      const r = runs.find((x) => x.id === e.payload.id);
      if (r) {
        r.endedAt = Date.now();
        r.code = e.payload.code ?? -1;
      }
    });
    const timer = setInterval(() => (now = Date.now()), 1000);
    return () => {
      unlistenOut.then((f) => f());
      unlistenExit.then((f) => f());
      unlistenUpdate.then((f) => f());
      unlistenPlugins.then((f) => f());
      unlistenActions.then((f) => f());
      unlistenBackendUi.then((f) => f());
      clearInterval(timer);
      clearInterval(idle);
    };
  });
</script>

<svelte:window onclick={closeMenuOutside} onmousedown={mouseDown} onkeydown={onKey} />
<div class="drag" data-tauri-drag-region></div>
<main class:mac class:wide>
  <!-- ------------------------------------------------------------ projects -->
  <aside class="panel left">
    <header class="app" data-tauri-drag-region>
      <button class="title" title="Pick a character, or none" onclick={() => avatarRef?.pickCharacter()}>thumbdeck</button>
      <Avatar bind:this={avatarRef} mood={avatarMood.mood} caption={avatarMood.caption} character={avatar}
        onPick={async (id) => applyList(await invoke<ProjectList>("edit_projects", { change: "avatar", path: id }))} />
      {#if update}
        <button class="update-dot" title="thumbdeck {update.version} is available" onclick={() => (updateDialog = true)}>● {update.version}</button>
      {/if}
    </header>
    <input class="filter" placeholder="Filter projects…   /" bind:value={filter} bind:this={filterEl} onkeydown={filterKey}
           onfocus={() => (filtering = true)} onblur={() => (filtering = false)} />
    <section class="card helm">
      <h2>
        <span class="dot purple"></span>Projects <span class="count">{visible.length}</span>
        <span class="menu-anchor" bind:this={addMenuEl}>
          <button class="icon" title="Add…" onclick={() => (addMenu = !addMenu)}>+</button>
          {#if addMenu}
            <div class="menu">
              <button onclick={() => pickFolder("add")}>Add project…</button>
              <button onclick={() => pickFolder("add-root")}>Add folder to scan…</button>
              <button onclick={openSettings}>Settings…{#if Object.keys(pluginUpdates).length}<span class="menu-new"> ↑ {Object.keys(pluginUpdates).length}</span>{/if}</button>
              <button onclick={checkForUpdates}>Check for updates</button>
            </div>
          {/if}
        </span>
      </h2>
      {#each sections as sec (sec.id)}
        <div class="section">
          <div class="section-head">
            <button class="fold" onclick={() => toggleFold(sec.id)}>
              <span class="caret">{folded(sec.id) ? "▸" : "▾"}</span>{sec.label}
              <span class="n">{sec.items.length}</span>
            </button>
            {#if sec.root}
              <button class="icon hide" title="Stop scanning this folder" onclick={() => edit("remove-root", sec.root!)}>×</button>
            {/if}
          </div>
          {#if !folded(sec.id)}
            <ul>
              {#each sec.items as p (p.path)}
                {@const [glyph, color] = icons[p.kind] ?? icons.folder}
                <li class="project" class:muted={p.hidden}>
                  <button class="pick" class:active={selected?.path === p.path && !p.hidden}
                          disabled={p.hidden} onclick={() => select(p)}>
                    {#if p.icon}<img class="kind" src={p.icon} alt="" />{:else}<span class="kind" style="color: var(--{color})">{glyph}</span>{/if}
                    <span class="name">{p.name}</span>
                    {#if p.dirty}<span class="changes" title="uncommitted changes"></span>{/if}
                  </button>
                  {#if p.hidden}
                    <button class="icon" title="Show again" onclick={() => edit("unhide", p.path)}>↺</button>
                  {:else}
                    <button class="icon hide star" class:on={p.pinned} title={p.pinned ? "Unpin" : "Pin"}
                            onclick={() => edit("pin", p.path)}>{p.pinned ? "★" : "☆"}</button>
                    <button class="icon hide" title={p.root === null ? "Remove from the list" : "Hide"}
                            onclick={() => edit("remove", p.path)}>×</button>
                  {/if}
                </li>
              {/each}
            </ul>
          {/if}
        </div>
      {/each}
    </section>
  </aside>

  <!-- ------------------------------------------------------------ center -->
  <section class="panel center" bind:this={centerEl}>
    {#if selected}
      <nav class="crumbs">
        <span class="path" title={selected.path}>{home(selected.path).split("/").slice(0, -1).join(" / ")}</span>
        <span> / </span><strong class="name">{selected.name}</strong>
        {#if selected.branch}<span class="branch">  {selected.branch}</span>{/if}
        <span class="spacer"></span>
        <button class="tab open" title="Open in tmux (Enter)" onclick={openInTmux}> Open in tmux</button>
        <button class="tab" title={wide ? "Show the side panels again (z)" : "Expand: the center takes the whole window (z)"}
                onclick={() => (wide = !wide)}>{wide ? "⤡" : "⤢"}</button>
      </nav>
      <div class="view">
      {#each liveFrames as f (f.key)}
        <PluginFrame bind:this={frameRefs[f.key]} info={f.tab.frame!} frameKey={f.key} surface="tab" surfaceId={f.tab.extension}
                     project={f.project} setup={f.tab.setup} active={keysToTab && shownFrameKey === f.key}
                     visible={shownFrameKey === f.key && !current} thumbdeck={version} {host} {say}
                     onActivate={() => shownFrameKey === f.key && (keysToTab = true)} onRelease={() => (keysToTab = false)}
                     onEditSetup={() => f.project.path === selected?.path && (setupForm = { index: f.index, tab: f.tab })}
                     openReview={(r) => (review = r)} />
      {/each}
      {#if current}
        <pre class="output" bind:this={outputEl}><span class="cmd">$ {current.command}</span>
{#each current.lines as l}<span class:err={l.stderr}>{l.text}</span>
{/each}{#if current.endedAt !== null}<span class={current.code === 0 ? "ok" : "err"}>{current.code === 0 ? "✓ finished" : `✗ exited with ${current.code}`} after {elapsed(current)}</span>{/if}</pre>
      {:else if tabShown && details && shownTab !== null}
        {@const t = details.tabs[shownTab]}
        {@const ext = extensions[t.extension]}
        {#if t.frame}
          <!-- a plugin's tab: its frame is above, kept alive -->
        {:else if t.missing}
          <div class="empty">
            <p>{t.missing}</p>
            <button class="ghost" onclick={() => { setupForm = { index: shownTab, tab: t }; saveTab("remove"); }}>Remove this tab</button>
          </div>
        {:else if ext && !t.plugin}
          {#key `${selected.path}:${shownTab}:${JSON.stringify(t.setup)}`}
            <ext.tab bind:this={tabRef} path={selected.path} project={selected.name} setup={t.setup} active={keysToTab} {say}
                     onActivate={() => (keysToTab = true)} onRelease={() => (keysToTab = false)}
                     onEditSetup={() => (setupForm = { index: shownTab, tab: t })} openReview={(r) => (review = r)} />
          {/key}
        {:else}
          <div class="empty">
            <p>The "{t.extension}" tab isn't built in any more: it's a plugin now. Add the plugin in Settings (,) › Plugins,
              then add its tab again.</p>
            <button class="ghost" onclick={() => { setupForm = { index: shownTab, tab: t }; saveTab("remove"); }}>Remove this tab</button>
          </div>
        {/if}
      {:else if details?.readme}
        <article class="readme" bind:this={readmeEl}>{@html marked.parse(details.readme)}</article>
      {:else}
        <p class="empty">{details ? "No README in this project." : "Loading…"}</p>
      {/if}
      </div>
      <!-- Tabs: README, the project's extension tabs, the runs you started (1, 2, …) -->
      <nav class="tabbar">
        <div class="tabs">
        <button class="btab" class:on={shownRun === null && !tabShown} title="README (1)" onclick={() => showTab(null)}>
          <span class="bicon">≡</span>README
        </button>
        {#each details?.tabs ?? [] as t, i}
          <button class="btab" class:on={shownRun === null && shownTab === i} class:keys={keysToTab && tabShown && shownTab === i}
                  title="{t.title} ({i + 2})" onclick={() => showTab(i)}><span class="bicon">▤</span>{t.title}{#if selected && badges[frameKeyOf(selected.path, i, t)]}<span class="tbadge">{badges[frameKeyOf(selected.path, i, t)]}</span>{/if}</button>
        {/each}
        {#each runTabs as r, i (r.id)}
          {@const n = 2 + (details?.tabs.length ?? 0) + i}
          <span class="btab brun" class:on={shownRun === r.id}>
            <button class="blabel" title="{r.command}{n <= 9 ? ` (${n})` : ''}" onclick={() => showRun(r.id)}>
              <span class="bicon st {status(r)}">{r.endedAt === null ? "●" : r.code === 0 ? "✓" : "✗"}</span>{r.label}
            </button>
            <button class="bclose" title="Close the tab{r.endedAt === null ? ' (the command keeps running)' : ''}"
                    onclick={() => closeRun(r)}>×</button>
          </span>
        {/each}
        </div>
        <span class="menu-anchor" bind:this={tabMenuEl}>
          <button class="btab add" title="Add a tab to this project" onclick={openTabMenu}>+</button>
          {#if tabMenu}
            <div class="menu up">
              {#each available as x}
                <button title={x.description} onclick={() => addTab(x.id)}>{x.name}<span class="menu-sub">{x.description}</span></button>
              {/each}
            </div>
          {/if}
        </span>
        <span class="spacer"></span>
        {#if keysToTab && tabShown && details && shownTab !== null}
          <span class="keys-hint">keys go to {details.tabs[shownTab].title} · Esc gives them back</span>
        {/if}
      </nav>
    {:else}
      <p class="empty">No projects found in ~/dev, ~/projects or your home folder.</p>
    {/if}
  </section>

  <!-- ------------------------------------------------------------ running + toolkit -->
  <aside class="panel right">
    <section class="card running">
      <h2><span class="dot green"></span>Running</h2>
      {#if projectRuns.length === 0}
        <p class="hint">Nothing yet: pick something from the toolkit.</p>
      {/if}
      {#each projectRuns as r (r.id)}
        <div class="run" class:shown={r.id === shownRun} role="button" tabindex="0"
             onclick={() => showRun(r.id)} onkeydown={(e) => e.key === "Enter" && showRun(r.id)}>
          <span class="avatar {r.source}">{r.source.slice(0, 2)}</span>
          <span class="what">
            <span class="label">{r.label}</span>
            <span class="pill {status(r)}">{status(r)}</span>
          </span>
          <span class="time">{elapsed(r)}</span>
          {#if r.endedAt === null}
            <button class="stop" title="Stop" onclick={(e) => { e.stopPropagation(); stop(r); }}>■</button>
          {/if}
        </div>
      {/each}
    </section>

    <section class="card toolkit">
      <h2>
        <span class="dot orange"></span>Toolkit {#if selected}<span class="for">{selected.name}</span>{/if}
        {#if selected}<button class="icon" title="Add your own action" onclick={() => openForm()}>+</button>{/if}
      </h2>
      {#if details && groups.length === 0}
        <p class="hint">Nothing detected here. Add your own action with +.</p>
      {/if}
      {#each details?.problems ?? [] as problem}
        <p class="hint problem">{problem}</p>
      {/each}
      {#each groups as [group, actions] (actions[0].source)}
        <h3>{group}</h3>
        <div class="grid">
          {#each actions as a (a.id)}
            {@const hint = HINT_KEYS[details?.actions.indexOf(a) ?? -1]}
            <div class="action-wrap">
              <button class="action" class:custom={a.source === "custom"} onclick={() => run(a)}
                      title={[a.description, a.command, a.tmux && `runs in tmux window '${a.tmux}'`].filter(Boolean).join("\n")}>
                {#if hints && hint}
                  <span class="hint-key">{hint}</span>
                {:else}
                  <span class="play">{a.confirm ? "!" : a.tmux ? "↗" : "▷"}</span>
                {/if}
                <span class="alabel">{a.label}</span>
              </button>
              <span class="action-tools">
                {#if a.source === "custom"}
                  <button class="icon" title="Edit" onclick={() => openForm(a)}>✎</button>
                  <button class="icon" title="Delete" onclick={() => editActions("delete", a.id)}>×</button>
                {:else}
                  <button class="icon" title="Hide from this project" onclick={() => editActions("hide", a.id)}>×</button>
                {/if}
              </span>
            </div>
          {/each}
        </div>
      {/each}
      {#if details?.hidden.length}
        <button class="fold hidden-actions" onclick={() => (showHiddenActions = !showHiddenActions)}>
          <span class="caret">{showHiddenActions ? "▾" : "▸"}</span>hidden ({details.hidden.length})
        </button>
        {#if showHiddenActions}
          <ul class="hidden-list">
            {#each details.hidden as a (a.id)}
              <li class="project muted">
                <span class="name">{a.group} · {a.label}</span>
                <button class="icon" title="Show again" onclick={() => editActions("unhide", a.id)}>↺</button>
              </li>
            {/each}
          </ul>
        {/if}
      {/if}
    </section>
  </aside>
</main>


<StatusBar mode={barKeys.name} project={selected?.name ?? null} branch={branchState} message={toast} />

{#if review}
  <ReviewPage bind:this={reviewRef} request={review} onClose={() => (review = null)} />
{/if}

{#if helpOpen}
  <div class="backdrop" role="presentation" onclick={() => (helpOpen = false)}>
    <div class="dialog help">
      <h2><span class="dot purple"></span>Keys <span class="for">? or Esc to close</span></h2>
      <dl>
        {#each MAIN.bindings as b}<dt>{keysLabel(b)}</dt><dd>{b.does}</dd>{/each}
        <dt>in a tab</dt><dd>the same keys mean the same things: j k g G move, Enter / l open, h back, Tab next list,
          v review, o open outside, d / u scroll, r refresh, S setup, ? its keys, Esc or q give the keyboard back</dd>
      </dl>
    </div>
  </div>
{/if}

{#if updateDialog && update}
  <div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && updateState !== "installing" && (updateDialog = false)}>
    <div class="dialog">
      <h2><span class="dot green"></span>thumbdeck {update.version} <span class="for">you have {update.current}</span></h2>
      {#if update.notes}<article class="readme notes">{@html marked.parse(update.notes)}</article>{/if}
      {#if updateError}<p class="hint problem">{updateError}</p>{/if}
      {#if updateState === "installed"}
        <p class="hint">Installed. It runs after a restart.</p>
      {/if}
      <div class="buttons">
        {#if updateState === "installed"}
          <button class="ghost" onclick={() => (updateDialog = false)}>Later</button>
          <button class="primary" onclick={() => invoke("restart")} {@attach (el) => el.focus()}>Restart now</button>
        {:else}
          <button class="ghost" disabled={updateState === "installing"} onclick={() => (updateDialog = false)}>Later</button>
          <button class="primary" disabled={updateState === "installing"} onclick={installUpdate} {@attach (el) => el.focus()}>
            {updateState === "installing" ? "Updating…" : "Update"}
          </button>
        {/if}
      </div>
    </div>
  </div>
{/if}

{#if setupForm && selected}
  {@const ext = extensions[setupForm.tab.extension]}
  {@const frame = setupForm.tab.frame}
  {#if setupForm.tab.plugin && frame}
    <PluginSetupForm name={frame.name} pluginName={frame.plugin_name} description="" fields={frame.fields} setup={setupForm.tab.setup}
                     isNew={setupForm.index === null} project={selected.name}
                     onSave={(setup) => saveTab(setupForm?.index === null ? "add" : "save", setup)}
                     onRemove={() => saveTab("remove")} onCancel={() => (setupForm = null)} />
  {:else if setupForm.tab.plugin}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div class="backdrop" onclick={(e) => e.target === e.currentTarget && (setupForm = null)}>
      <div class="dialog">
        <p>{setupForm.tab.missing}</p>
        <div class="buttons">
          <button class="ghost" onclick={() => saveTab("remove")}>Remove tab</button>
          <button class="primary" onclick={() => (setupForm = null)}>Close</button>
        </div>
      </div>
    </div>
  {:else if ext}
    <ext.setup setup={setupForm.tab.setup} isNew={setupForm.index === null} project={selected.name}
               onSave={(setup) => saveTab(setupForm?.index === null ? "add" : "save", setup)}
               onRemove={() => saveTab("remove")} onCancel={() => (setupForm = null)} />
  {/if}
{/if}

{#if settingsOpen}
  <SettingsDialog updates={pluginUpdates} onUpdates={(u) => (pluginUpdates = u)} onChanged={pluginsChanged}
                  onClose={() => (settingsOpen = false)} />
{/if}

{#if form && selected}
  <div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && (form = null)}>
    <form class="dialog" onsubmit={(e) => { e.preventDefault(); saveForm(); }}>
      <h2><span class="dot orange"></span>{form.id ? "Edit action" : "New action"} <span class="for">{selected.name}</span></h2>
      <label>Name <input bind:value={form.name} placeholder="Backup DB" {@attach (el) => el.focus()} /></label>
      <label>Command
        <textarea bind:value={form.command} rows="3" placeholder="./scripts/backup.sh && ls backups/"
                  onkeydown={(e) => e.key === "Enter" && (e.ctrlKey || e.metaKey) && saveForm()}></textarea>
      </label>
      <p class="hint">Runs in {home(selected.path)} with your shell's environment.</p>
      <label class="check"><input type="checkbox" bind:checked={form.confirm} /> Ask before running</label>
      <label class="check">
        <input type="checkbox" checked={form.tmux !== null} onchange={(e) => (form!.tmux = e.currentTarget.checked ? "" : null)} />
        Run in the project's tmux session (for servers, watchers, shells)
      </label>
      {#if form.tmux !== null}
        <label>Window <input bind:value={form.tmux} placeholder={form.name.trim() || "named after the action"} /></label>
        <p class="hint">Made if needed; if it's already running something, it's left alone.</p>
      {/if}
      <div class="buttons">
        <button type="button" class="ghost" onclick={() => (form = null)}>Cancel</button>
        <button type="submit" class="primary" disabled={!form.name.trim() || !form.command.trim()}>Save</button>
      </div>
    </form>
  </div>
{/if}

<style>
  :global(html, body) { margin: 0; height: 100%; background: var(--bg-dim); color: var(--fg); font: 14px/1.5 var(--sans); }
  :global(*) { box-sizing: border-box; }
  :global(button) { font: inherit; color: inherit; background: none; border: 0; cursor: pointer; }

  /* invisible strip along the top to move the window by (macOS has no title bar here) */
  .drag { position: fixed; inset: 0 0 auto 0; height: 8px; z-index: 10; }
  main.mac .app { padding-top: 26px; }
  main { display: grid; grid-template-columns: 250px 1fr 340px; gap: 8px; height: calc(100vh - 22px); padding: 8px; } /* 22px: the status bar */
  main.wide { grid-template-columns: 1fr; }
  main.wide > .left, main.wide > .right { display: none; }
  .panel { min-height: 0; display: flex; flex-direction: column; gap: 8px; }
  .card { background: var(--bg0); border: 1px solid var(--bg2); border-radius: 12px; padding: 12px; }
  h2 { display: flex; align-items: center; gap: 8px; margin: 0 0 10px; font: 600 13px var(--mono); }
  h3 { margin: 10px 0 6px; font: 500 11px var(--mono); color: var(--grey); text-transform: uppercase; letter-spacing: 0.08em; }
  :global(.dot) { width: 8px; height: 8px; border-radius: 50%; }
  :global(.dot.purple) { background: var(--purple); } :global(.dot.green) { background: var(--green); } :global(.dot.orange) { background: var(--orange); }
  .count, :global(.for) { margin-left: auto; color: var(--grey); font-weight: 400; }
  :global(.hint), .empty { color: var(--grey); font-size: 13px; }
  .hint.problem { color: var(--red); font-size: 12px; margin: 0 0 6px; }
  .branch { color: var(--purple); font: 12px var(--mono); }

  /* left */
  .app .title { font: inherit; padding: 0; background: none; }
  .app { font: 700 22px var(--mono); padding: 4px 6px; display: flex; align-items: center; gap: 8px; }
  .update-dot { padding: 1px 8px; border-radius: 10px; background: var(--bg1); color: var(--green); font: 600 11px var(--mono); }
  .update-dot:hover { background: var(--bg2); }
  .notes { max-height: 40vh; padding: 0 4px; font-size: 13px; }
  .filter { width: 100%; background: var(--bg0); border: 1px solid var(--bg2); border-radius: 8px; padding: 7px 10px; color: var(--fg); font: 13px var(--mono); outline: none; }
  .filter:focus { border-color: var(--purple); }
  .helm { flex: 1; overflow: auto; }
  .section { margin-top: 6px; }
  .section-head { display: flex; align-items: center; }
  .section-head:hover .hide { opacity: 1; }
  :global(.fold) { flex: 1; display: flex; align-items: center; gap: 6px; text-align: left; padding: 4px 6px; font: 600 11px var(--mono); color: var(--grey); letter-spacing: 0.03em; }
  :global(.fold:hover) { color: var(--fg); }
  :global(.caret) { width: 10px; }
  .n { margin-left: auto; font-weight: 400; opacity: 0.7; }
  .helm ul { list-style: none; margin: 2px 0 4px; padding: 0 0 0 10px; }
  .project { display: flex; align-items: center; gap: 2px; }
  .pick { flex: 1; min-width: 0; display: flex; align-items: center; gap: 8px; text-align: left; padding: 5px 8px; border-radius: 8px; }
  .pick:hover:not(:disabled) { background: var(--bg1); }
  .pick.active { background: var(--bg2); }
  .pick:disabled { cursor: default; }
  .kind { width: 16px; text-align: center; font-family: var(--mono); }
  img.kind { height: 14px; object-fit: contain; }
  .name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .muted { opacity: 0.55; }
  :global(.icon) { width: 22px; height: 22px; flex: none; border-radius: 6px; color: var(--grey); font: 13px var(--mono); }
  :global(.icon:hover) { background: var(--bg2); color: var(--fg); }
  .hide { opacity: 0; }
  .project:hover .hide, .star.on { opacity: 1; }
  .star.on { color: var(--yellow); }
  h2 .icon { margin-left: 6px; }
  .menu-anchor { position: relative; }
  .menu { position: absolute; right: 0; top: 26px; z-index: 20; min-width: 190px; padding: 4px; border-radius: 10px; background: var(--bg1); border: 1px solid var(--bg3); box-shadow: 0 8px 24px #0008; }
  .menu button { display: block; width: 100%; text-align: left; padding: 6px 10px; border-radius: 6px; font: 12.5px var(--sans); }
  .menu button:hover { background: var(--bg2); }

  .name { font: 13px var(--mono); }
  .changes { width: 6px; height: 6px; border-radius: 50%; background: var(--orange); }


  /* center */
  .center { position: relative; background: var(--bg0); border: 1px solid var(--bg2); border-radius: 12px; overflow: hidden; }
  .crumbs { display: flex; align-items: center; gap: 4px; padding: 10px 14px; border-bottom: 1px solid var(--bg2); font: 12px var(--mono); color: var(--grey); }
  .crumbs strong { color: var(--fg); }
  /* One line: the path gives way first (… at its end), the name, branch and buttons stay whole */
  .crumbs > :not(.spacer) { flex: none; white-space: nowrap; }
  .crumbs > .path { flex: 0 1 auto; min-width: 2ch; overflow: hidden; text-overflow: ellipsis; }
  .spacer { flex: 1; }
  .tab { padding: 3px 10px; border-radius: 6px; font: 12px var(--mono); color: var(--grey); }
  .tab.on { background: var(--bg2); color: var(--fg); }
  .view { flex: 1; min-height: 0; display: flex; flex-direction: column; overflow: hidden; }
  .view > .readme { overflow: auto; }
  .tabbar { flex: none; display: flex; align-items: center; gap: 2px; padding: 4px 6px; border-top: 1px solid var(--bg2);
            font: 12px var(--mono); }
  /* The tabs keep their size and scroll sideways when there are many; + stays outside (its menu opens upward) */
  .tabs { flex: 0 1 auto; min-width: 0; display: flex; align-items: center; gap: 2px; overflow-x: auto; scrollbar-width: thin; }
  .btab { display: inline-flex; align-items: center; gap: 6px; max-width: 180px; padding: 3px 10px; border-radius: 6px;
          color: var(--grey); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; flex: none; }
  .btab:hover { background: var(--bg1); color: var(--fg); }
  .btab.on { background: var(--bg2); color: var(--fg); }
  .btab.keys { box-shadow: inset 0 -2px var(--orange); }
  .btab.brun { padding: 0 2px 0 0; gap: 0; }
  .btab.brun .blabel { display: inline-flex; align-items: center; gap: 6px; min-width: 0; padding: 3px 4px 3px 10px;
                      overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .bclose { flex: none; padding: 0 6px; color: var(--grey); border-radius: 4px; }
  .bclose:hover { color: var(--fg); background: var(--bg3); }
  .bicon { flex: none; color: var(--grey); }
  .st.running, .st.done { color: var(--green); } .st.failed { color: var(--red); }
  .btab.add { flex: none; padding: 3px 9px; }
  .menu.up { top: auto; bottom: 30px; left: 0; right: auto; }
  .menu-sub { display: block; color: var(--grey); font-size: 11px; }
  .menu-new { color: var(--green); font-weight: 600; }
  .tbadge { margin-left: 6px; padding: 0 6px; border-radius: 99px; background: var(--bg3); color: var(--fg); font-size: 10.5px; }
  .keys-hint { flex: none; padding: 2px 8px; border-radius: 6px; background: var(--bg1); color: var(--orange); font: 11px var(--mono); }
  .readme { padding: 8px 28px 28px; overflow: auto; max-width: 860px; }
  .readme :global(img) { max-width: 100%; height: auto; border-radius: 6px; }
  .readme :global(h1), .readme :global(h2) { font-family: var(--mono); border-bottom: 1px solid var(--bg2); padding-bottom: 6px; }
  .readme :global(code) { font-family: var(--mono); background: var(--bg1); padding: 1px 5px; border-radius: 4px; color: var(--aqua); }
  .readme :global(pre) { background: var(--bg1); padding: 12px; border-radius: 8px; overflow: auto; }
  .readme :global(pre code) { background: none; padding: 0; color: var(--fg); }
  .readme :global(a) { color: var(--blue); }
  .output { flex: 1; margin: 0; padding: 14px; overflow: auto; font: 12.5px/1.45 var(--mono); white-space: pre-wrap; word-break: break-word; }
  .output .cmd { color: var(--green); }
  .output .err { color: var(--red); } .output .ok { color: var(--green); }
  .empty { padding: 20px; }

  /* right */
  .running { max-height: 45%; overflow: auto; }
  .run { display: flex; align-items: center; gap: 10px; padding: 8px; border-radius: 10px; background: var(--bg1); border: 1px solid transparent; margin-bottom: 6px; cursor: pointer; }
  .run.shown { border-color: var(--green); }
  .avatar { width: 30px; height: 30px; border-radius: 8px; display: grid; place-items: center; font: 700 11px var(--mono); text-transform: uppercase; color: var(--bg0); background: var(--grey); }
  .avatar.npm { background: var(--red); } .avatar.django { background: var(--green); } .avatar.pytest { background: var(--yellow); }
  .avatar.compose { background: var(--blue); } .avatar.gradle { background: var(--aqua); } .avatar.make { background: var(--purple); }
  .avatar.cargo { background: var(--orange); } .avatar.go { background: var(--blue); }
  .what { flex: 1; display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .label { font: 13px var(--mono); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .pill { align-self: flex-start; font: 10px var(--mono); padding: 1px 7px; border-radius: 99px; }
  .pill.running { color: var(--green); background: color-mix(in srgb, var(--green) 15%, transparent); }
  .pill.done { color: var(--grey); background: var(--bg2); }
  .pill.failed { color: var(--red); background: color-mix(in srgb, var(--red) 15%, transparent); }
  .time { font: 11px var(--mono); color: var(--grey); }
  .stop { color: var(--red); padding: 2px 6px; border-radius: 6px; }
  .stop:hover { background: var(--bg2); }

  .toolkit { flex: 1; overflow: auto; border-color: color-mix(in srgb, var(--orange) 35%, var(--bg2)); }
  .grid { display: grid; grid-template-columns: 1fr 1fr; gap: 6px; }
  .action-wrap { position: relative; }
  .action { width: 100%; display: flex; align-items: center; gap: 8px; text-align: left; padding: 8px 10px; border-radius: 8px; background: var(--bg1); border: 1px solid var(--bg2); font: 12.5px var(--mono); }
  .action.custom { border-color: color-mix(in srgb, var(--yellow) 30%, var(--bg2)); }
  .alabel { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .action-tools { position: absolute; right: 4px; top: 50%; transform: translateY(-50%); display: flex; opacity: 0; background: var(--bg1); border-radius: 6px; }
  .action-wrap:hover .action-tools { opacity: 1; }
  .hidden-actions { margin-top: 12px; }
  .hidden-list { list-style: none; margin: 0; padding: 0 0 0 10px; font: 12px var(--mono); }
  .avatar.custom { background: var(--yellow); }
  .tab.open { color: var(--green); }
  .tab.open:hover { background: var(--bg2); }
  .hint-key { min-width: 16px; padding: 0 4px; border-radius: 4px; text-align: center; background: var(--orange); color: var(--bg0); font-weight: 700; }
  .hint-note { font: 400 11px var(--mono); color: var(--orange); }
  .help dl { display: grid; grid-template-columns: max-content 1fr; gap: 6px 16px; margin: 0; font-size: 13px; }
  .help dt { font-family: var(--mono); color: var(--orange); }
  .help dd { margin: 0; color: var(--fg); }

  :global(.backdrop) { position: fixed; inset: 0; z-index: 50; background: #0009; display: grid; place-items: center; }
  :global(.dialog) { width: min(520px, 90vw); background: var(--bg0); border: 1px solid var(--bg3); border-radius: 14px; padding: 18px; display: flex; flex-direction: column; gap: 12px; box-shadow: 0 20px 60px #000a; }
  :global(.dialog label) { display: flex; flex-direction: column; gap: 6px; font: 12px var(--mono); color: var(--grey); }
  :global(.dialog input:not([type]), .dialog textarea) { background: var(--bg1); border: 1px solid var(--bg2); border-radius: 8px; padding: 8px 10px; color: var(--fg); font: 13px var(--mono); outline: none; resize: vertical; }
  :global(.dialog input:not([type]):focus, .dialog textarea:focus) { border-color: var(--orange); }
  :global(.dialog .check) { flex-direction: row; align-items: center; gap: 8px; color: var(--fg); }
  :global(.dialog .hint) { margin: -6px 0 0; font-size: 12px; }
  :global(.buttons) { display: flex; justify-content: flex-end; gap: 8px; }
  :global(.ghost, .primary) { padding: 7px 16px; border-radius: 8px; font: 12.5px var(--mono); }
  :global(.ghost) { color: var(--grey); } :global(.ghost:hover) { background: var(--bg2); color: var(--fg); }
  :global(.primary) { background: var(--orange); color: var(--bg0); font-weight: 600; }
  :global(.primary:disabled) { opacity: 0.4; cursor: default; }
  .action:hover { border-color: var(--orange); }
  .play { color: var(--orange); }
</style>
