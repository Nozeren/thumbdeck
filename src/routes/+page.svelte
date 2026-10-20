<script lang="ts">
  import "$lib/theme.css";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { marked } from "marked";
  import { onMount, tick, untrack } from "svelte";
  import { ask, open } from "@tauri-apps/plugin-dialog";
  import type { Action, CustomAction, Details, Project, ProjectList, Run, Tab, TabInfo, Update } from "$lib/types";
  import type { CardInfo, CatalogEntry, FrameInfo, Host, PanelInfo, TabExports, View } from "$lib/plugins/types";
  import CatalogDialog from "$lib/plugins/CatalogDialog.svelte";
  import PluginFrame from "$lib/plugins/PluginFrame.svelte";
  import PluginSetupForm from "$lib/plugins/PluginSetupForm.svelte";
  import { getVersion } from "@tauri-apps/api/app";
  import Avatar from "$lib/avatar/Avatar.svelte";
  import ReviewPage from "$lib/review/ReviewPage.svelte";
  import StatusBar from "$lib/StatusBar.svelte";
  import SettingsDialog from "$lib/settings/SettingsDialog.svelte";
  import { bind, keyName, KeyReader, keysLabel, times, type Binding, type Fed, type Keymap, type Press } from "$lib/keys/keys";
  import { CENTER, COMMAND, DIALOG, FILTER, GLOBAL_ACTIONS, KIT, MAIN, MOVED, OUTPUT, OUTPUT_VISUAL, OVERVIEW, PLUGINS, PROJECTS, REVIEW, RUNNING, TOOLKIT } from "$lib/keys/maps";
  import { copyText, linesWord } from "$lib/clipboard";
  import Overview from "$lib/overview/Overview.svelte";
  import type { ReviewRequest } from "$lib/review/types";
  import { combine, pickMood } from "$lib/avatar/mood";

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
  }

  // A click in a pane gives it the keyboard (a click in a plugin's frame: see onActivate)
  function mouseDown(e: MouseEvent) {
    const p = (e.target as HTMLElement | null)?.closest<HTMLElement>("[data-pane]")?.dataset.pane;
    if (p) pane = p as Pane;
  }
  // ------------------------------------------------------------ extension tabs
  // Plugin tab shown in the center (its index in details.tabs); null: the Overview or the README
  // (shownPage says which), or a run
  let shownTab = $state<number | null>(null);
  let shownPage = $state<"overview" | "readme">("overview");
  let overviewRef = $state<{ run(p: import("$lib/keys/keys").Press): boolean; keymap(): import("$lib/keys/keys").Keymap } | null>(null);
  // The project's Overview cards from plugins
  let cards = $state<CardInfo[]>([]);
  // The pane with the keyboard, like a split in nvim or a pane in tmux: Ctrl+h/j/k/l moves
  // between them, a click or a tab's number key picks one, Esc goes back to the projects
  type Pane = "projects" | "plugins" | "center" | "running" | "toolkit";
  let pane = $state<Pane>("projects");
  // The center takes the whole window (z): more room for a log, a README, a run's output
  let wide = $state(false);
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
  const frameKeyOf = (path: string, i: number, t: Tab) => `${path}|${i}|${t.plugin}:${t.tab}`;
  const shownFrameKey = $derived.by(() => {
    if (!tabShown || !selected || !details || shownTab === null) return null;
    const t = details.tabs[shownTab];
    return t?.frame ? frameKeyOf(selected.path, shownTab, t) : null;
  });
  // ------------------------------------------------------------ plugins' views, pages, panels
  let views = $state<View[]>([]);
  let panels = $state<PanelInfo[]>([]);
  // The view shown in the center instead of the project (a plugin id), and the views kept alive
  let shownView = $state<string | null>(null);
  let liveViews = $state<{ key: string; plugin: string; frame: FrameInfo }[]>([]);
  let viewRefs = $state<Record<string, TabExports | null>>({});
  let statuses = $state<Record<string, string | null>>({});
  let paneCursor = $state(0);
  // A plugin's page over the whole window
  let pluginPage = $state<{ key: string; id: string; info: FrameInfo; data: unknown; project: { path: string; name: string; branch: string | null } | null } | null>(null);
  let pageRef = $state<TabExports | null>(null);
  // Bumped when a linked plugin's files change: its frames load again
  let reloads = $state<Record<string, number>>({});
  // The official plugins, offered on the first start
  let catalogOffer = $state<CatalogEntry[] | null>(null);

  const shownViewKey = $derived(shownView ? `${shownView}#${reloads[shownView] ?? 0}` : null);

  async function loadPluginViews() {
    views = (await invoke<View[]>("plugin_views")).sort((a, b) => a.name.localeCompare(b.name)); // A–Z
    // Statuses backends said before the page was listening
    const said = await invoke<Record<string, string | null>>("plugin_statuses");
    for (const [plugin, text] of Object.entries(said)) if (!(plugin in statuses)) statuses[plugin] = text;
    if (shownView && !views.some((v) => v.plugin === shownView)) shownView = null;
    paneCursor = Math.min(paneCursor, Math.max(0, views.length - 1));
  }

  async function loadPanels() {
    const path = selected?.path ?? null;
    panels = await invoke<PanelInfo[]>("plugin_panels", { path });
    const c = path ? await invoke<CardInfo[]>("plugin_cards", { path }) : [];
    if (path === (selected?.path ?? null)) cards = c;
  }

  function openView(plugin: string, keys = true) {
    const v = views.find((x) => x.plugin === plugin);
    if (!v) return;
    shownView = plugin;
    const key = `${plugin}#${reloads[plugin] ?? 0}`;
    if (!liveViews.some((x) => x.key === key)) liveViews.push({ key, plugin, frame: v.frame });
    if (keys) pane = "center";
  }

  async function openPluginPage(plugin: string, id: string, data: unknown, project: { path: string; name: string; branch: string | null } | null) {
    try {
      const info = await invoke<FrameInfo>("plugin_frame", { plugin, surface: "page", id });
      pluginPage = { key: `page:${plugin}:${id}:${Date.now()}`, id, info, data, project };
    } catch (err) {
      say(String(err), true);
    }
  }

  // The shown tab's (or view's) keys
  const activeTab = $derived(
    shownViewKey ? (viewRefs[shownViewKey] ?? null) : shownFrameKey ? (frameRefs[shownFrameKey] ?? null) : null,
  );
  // The center has the keyboard and shows a plugin's tab or view: the keys are its own
  const keysToTab = $derived(pane === "center" && !!activeTab);

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
    badge: (key, value) => {
      badges[key] = value;
    },
    mood: (key, signal, value) => {
      pluginMoods[key] = { ...pluginMoods[key], [signal]: value };
    },
    forget: (key) => {
      delete pluginMoods[key];
      delete badges[key];
    },
    openPage: (plugin, id, data, project) => openPluginPage(plugin, id, data, project),
    closePage: () => (pluginPage = null),
    status: (plugin, text) => (statuses[plugin] = text),
  };
  // What plugin frames and backends tell the avatar: sender -> signal -> value
  let pluginMoods = $state<Record<string, Record<string, unknown>>>({});

  /** Show a plugin tab (its index), the Overview or the README */
  function showTab(i: number | "overview" | "readme", keys = true) {
    shownView = null;
    shownRun = null;
    if (typeof i === "number") shownTab = i;
    else [shownTab, shownPage] = [null, i];
    if (keys) pane = "center";
  }

  async function addTab(id: string) {
    try {
      setupForm = { index: null, tab: await invoke<TabInfo>("new_tab", { id }) };
    } catch (err) {
      say(String(err), true);
    }
  }

  async function saveTab(change: "add" | "save" | "remove", setup: unknown = null) {
    if (!selected || !setupForm) return;
    const { index, tab } = setupForm;
    setupForm = null;
    details = await invoke<Details>("edit_tab", {
      path: selected.path, change, index: index ?? 0,
      tab: setup === null ? null : { plugin: tab.plugin, tab: tab.tab, setup },
    });
    if (change === "add") showTab(details.tabs.length - 1);
    else if (change === "remove") showTab("overview");
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
    loadPluginViews();
    loadPanels();
    applyList(await invoke<ProjectList>("list_projects"));
    if (selected) details = await invoke<Details>("project_details", { path: selected.path });
  }

  // ------------------------------------------------------------ keyboard
  let hints = $state(false); // Space pressed: Toolkit buttons show their letters
  let helpOpen = $state(false);
  let filterEl = $state<HTMLInputElement | null>(null);
  // The Toolkit's letters (Space, then the letter): not the shared Space letters
  const HINT_KEYS = "dgjklqrucvbnmxzDGJKLQUCVBNMXZ";
  // The first start of 1.0: what changed in the keys
  let whatsNew = $state(false);


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

  // Ctrl+h/j/k/l: the pane that way. The left and right columns remember the pane last used.
  const lastInColumn: Pane[] = ["projects", "center", "toolkit"];
  function goPane(dir: "h" | "j" | "k" | "l") {
    const columns: Pane[][] = wide ? [[], ["center"], []]
      : [views.length ? ["projects", "plugins"] : ["projects"], ["center"], shownView ? [] : ["running", "toolkit"]];
    const c = columns.findIndex((col) => col.includes(pane));
    if (c < 0) return void (pane = "center");
    lastInColumn[c] = pane;
    const col = columns[c];
    if (dir === "j" || dir === "k") {
      pane = col[col.indexOf(pane) + (dir === "j" ? 1 : -1)] ?? pane;
    } else {
      const next = columns[c + (dir === "l" ? 1 : -1)];
      if (next?.length) pane = next.includes(lastInColumn[c + (dir === "l" ? 1 : -1)]) ? lastInColumn[c + (dir === "l" ? 1 : -1)] : next[0];
    }
  }

  // The Running and Toolkit panes' cursors
  let runCursor = $state(0);
  let kitCursor = $state(0);
  function moveCursor(cursor: number, length: number, delta: number | "first" | "last") {
    const next = delta === "first" ? 0 : delta === "last" ? length - 1 : cursor + delta;
    tick().then(() => document.querySelector(".cursor")?.scrollIntoView({ block: "nearest" }));
    return Math.max(0, Math.min(length - 1, next));
  }

  // The Toolkit's buttons as rows, two to a row as the grid shows them (indices into kitOrder)
  const kitRows = $derived.by(() => {
    let i = 0;
    return [...groups.map(([, actions]) => actions), pages].flatMap((list) =>
      Array.from({ length: Math.ceil(list.length / 2) }, (_, r) => list.slice(r * 2, r * 2 + 2).map(() => i++)),
    );
  });
  // h / j / k / l on the Toolkit's grid: j / k keep the column when the row has one
  function kitMove(action: string) {
    const last = kitTotal - 1;
    if (action === "first" || action === "last") return moveCursor(kitCursor, kitTotal, action);
    const r = kitRows.findIndex((row) => row.includes(kitCursor));
    if (r < 0) return 0;
    const c = kitRows[r].indexOf(kitCursor);
    const row = kitRows[r + (action === "down" ? 1 : action === "up" ? -1 : 0)];
    if (!row) return kitCursor;
    const next = action === "left" ? row[c - 1] : action === "right" ? row[c + 1] : row[Math.min(c, row.length - 1)];
    return moveCursor(next ?? kitCursor, last + 1, 0);
  }

  // The center's README or run output, scrolled from the keyboard (n times)
  function scrollCenter(how: string, n = 1) {
    const el = current ? outputEl : readmeEl;
    if (!el) return;
    const half = el.clientHeight / 2;
    const by: Record<string, number> = { down: 40 * n, up: -40 * n, "page-down": half * n, "page-up": -half * n, "full-down": el.clientHeight * n, "full-up": -el.clientHeight * n, first: -el.scrollHeight, last: el.scrollHeight };
    if (how in by) el.scrollBy({ top: by[how] });
  }

  // Lines selected in a run's output, the vim way (V, then j / k, then y): indexes into its lines
  let visual = $state<{ anchor: number; cursor: number } | null>(null);
  const visualFrom = $derived(visual ? Math.min(visual.anchor, visual.cursor) : -1);
  const visualTo = $derived(visual ? Math.max(visual.anchor, visual.cursor) : -1);
  $effect(() => {
    void shownRun;
    visual = null; // another run: no selection
  });

  async function yank(lines: string[], what: string) {
    if (!lines.length) return say("Nothing to copy yet");
    try {
      await copyText(lines.join("\n"));
      say(`Copied ${what}`);
    } catch {
      say("Couldn't copy", true);
    }
  }

  function outputKey(p: Press): boolean {
    const { action } = p;
    const lines = current?.lines ?? [];
    if (visual) {
      const half = Math.max(1, Math.floor((outputEl?.clientHeight ?? 400) / 2 / 18));
      const n = times(p);
      const to = { down: visual.cursor + n, up: visual.cursor - n, first: 0, last: p.count ? p.count - 1 : lines.length - 1, "page-down": visual.cursor + half * n, "page-up": visual.cursor - half * n, "full-down": visual.cursor + 2 * half * n, "full-up": visual.cursor - 2 * half * n }[action];
      if (to !== undefined) {
        visual = { ...visual, cursor: Math.max(0, Math.min(lines.length - 1, to)) };
        tick().then(() => outputEl?.querySelector(".vcur")?.scrollIntoView({ block: "nearest" }));
      } else if (action === "yank") {
        const picked = lines.slice(visualFrom, visualTo + 1).map((l) => l.text);
        visual = null;
        yank(picked, linesWord(picked.length));
      } else if (action === "visual" || action === "visual-line") visual = null;
      else return false;
      return true;
    }
    if (action === "visual" || action === "visual-line") {
      if (!lines.length) return say("No output to select yet"), true;
      visual = { anchor: lines.length - 1, cursor: lines.length - 1 };
      tick().then(() => outputEl?.querySelector(".vcur")?.scrollIntoView({ block: "nearest" }));
    } else if (action === "yank") yank(lines.slice(-times(p)).map((l) => l.text), times(p) === 1 ? "the last line" : `the last ${linesWord(times(p))}`);
    else if (action === "yank-all") yank([`$ ${current!.command}`, ...lines.map((l) => l.text)], `the whole output (${linesWord(lines.length)})`);
    else if (action === "delete") {
      if (current && current.endedAt === null) stop(current);
      else say("It has ended already");
    } else return readmeKey(p);
    return true;
  }

  // gt / gT: the next / previous page (then the run shown last), round and round. The
  // keyboard stays where it is.
  function cycleTabs(delta: number) {
    const list = shownPages.map((p) => p.page);
    if (!list.length) return;
    const count = list.length + (outputTab ? 1 : 0);
    const at = shownView ? -1 : current ? list.length : list.findIndex(pageOn);
    const next = (Math.max(at, delta > 0 ? -1 : 0) + delta + count) % count;
    if (next < list.length) showPageItem(list[next], false);
    else if (outputTab) showRun(outputTab.id);
  }

  function cycleRuns(delta: number) {
    const list = projectRuns.toReversed(); // oldest first
    if (!list.length) return;
    const i = list.findIndex((r) => r.id === shownRun);
    showRun(list[(i < 0 ? list.length - 1 : i + delta + list.length) % list.length].id);
  }

  // ------------------------------------------------------------ reading keys
  // One KeyReader puts presses together (sequences like gg or ]c, counts like 5j, a letter
  // after f) against the keymap of the place with the keyboard plus MAIN's; a plugin's tab or
  // view gets the finished presses (runPress), its other keys raw.
  const reader = new KeyReader();
  let keyPending = $state(""); // what's typed so far, in the status bar (vim's showcmd)
  let keyMenu = $state<[string, Binding][]>([]); // after Space: what can come next
  let flushTimer: ReturnType<typeof setTimeout> | undefined;

  // Toolkit buttons: Space (on thumbdeck's own panes) or Space Space (anywhere), then the letter
  // on the button. Not the shared Space letters (s stop, p pin, f filter…), so they never meet.
  const kitBindings = $derived.by((): Binding[] => {
    if (shownView || !details) return [];
    const n = Math.min(HINT_KEYS.length, details.actions.length + pages.length);
    return Array.from({ length: n }, (_, i) => {
      const a = details!.actions[i];
      const label = a ? a.label : (pages[i - details!.actions.length]?.title ?? "");
      return bind([`Space Space ${HINT_KEYS[i]}`, ...(keysToTab ? [] : [`Space ${HINT_KEYS[i]}`])], `toolkit:${i}`, label);
    });
  });

  /** The keys right now: the place's, then MAIN's (in a plugin's tab only the global ones), then the Toolkit's letters */
  function placeMap(): Keymap {
    if (pluginPage && pageRef) return pageRef.keymap();
    const place = keysToTab && activeTab ? activeTab.keymap() : pane === "center" && overviewShown ? (overviewRef?.keymap() ?? OVERVIEW) : paneKeys;
    const taken = new Set(place.bindings.flatMap((b) => b.keys));
    const main = (keysToTab ? MAIN.bindings.filter((b) => GLOBAL_ACTIONS.has(b.action)) : MAIN.bindings)
      .map((b) => ({ ...b, keys: b.keys.filter((k) => !taken.has(k)) }))
      .filter((b) => b.keys.length);
    return { ...place, bindings: [...place.bindings, ...main, ...kitBindings] };
  }
  const legacyKeys = () => !!((pluginPage && pageRef?.legacy()) || (keysToTab && activeTab?.legacy()));

  function onKey(e: KeyboardEvent) {
    // The review page covers everything: it reads its own keys (but : is the page's command line)
    if (review) {
      if (e.key === ":" && !(e.target instanceof HTMLElement && e.target.closest("input, textarea"))) return void (e.preventDefault(), openCmd(":"));
      if (reviewRef?.handleKey(e)) e.preventDefault();
      return;
    }
    if (cmd) return; // the : or / line has the keyboard (its field takes the keys)
    const typing = e.target instanceof HTMLElement && e.target.closest("input, textarea, select");
    const dialog = form || setupForm || helpOpen || updateDialog || settingsOpen || catalogOffer || whatsNew;
    // Ctrl+h/j/k/l: the next pane that way, from anywhere (a plugin's tab too)
    if (e.ctrlKey && !e.altKey && !e.metaKey && !dialog && !pluginPage && ["h", "j", "k", "l"].includes(e.key)) {
      e.preventDefault();
      if (typing) (e.target as HTMLElement).blur();
      reader.reset();
      keyPending = "";
      keyMenu = [];
      hints = false;
      goPane(e.key as "h" | "j" | "k" | "l");
      return;
    }
    // In a form, :w (or Ctrl+s) saves
    if (form && e.ctrlKey && e.key === "s") return void (e.preventDefault(), saveForm());
    // Enter / Space on a button you reached with the keyboard (Tab) press it, not thumbdeck's
    // Enter or Space. A clicked button keeps the focus too, but not :focus-visible.
    const button = e.target instanceof HTMLElement ? e.target.closest("button") : null;
    if ((e.key === "Enter" || e.key === " ") && button?.matches(":focus-visible")) return;
    if (e.key === "Escape" && !reader.waiting && (addMenu || dialog || typing)) {
      // Esc closes what's open
      addMenu = false;
      form = null;
      setupForm = null;
      settingsOpen = false;
      whatsNew = false;
      if (updateState !== "installing") updateDialog = false;
      helpOpen = false;
      if (typing) (e.target as HTMLElement).blur();
      return;
    }
    if (helpOpen && e.key === "?") return void (helpOpen = false);
    if (typing || dialog || e.metaKey) return;
    // ? help over a plugin's frame: the next key closes it
    const frame = pluginPage ? pageRef : keysToTab ? activeTab : null;
    if (frame?.helpShown()) return void (e.preventDefault(), frame.closeHelp());
    const map = placeMap();
    if (handleFed(reader.feed(map, keyName(e), { counts: !legacyKeys() }), e, map)) e.preventDefault();
  }

  /** What the reader made of a key; true: it was thumbdeck's (or a plugin's) */
  function handleFed(fed: Fed, e: KeyboardEvent | null, map: Keymap): boolean {
    clearTimeout(flushTimer);
    if (fed.kind === "wait") {
      keyPending = fed.pending;
      keyMenu = fed.pending.startsWith("Space") ? reader.next(map) : [];
      hints = fed.pending === "Space Space " || (fed.pending === "Space " && !keysToTab && !pluginPage);
      // a shorter key that's waiting runs after a second (vim's timeoutlen)
      flushTimer = setTimeout(() => {
        const f = reader.flush();
        if (f) handleFed({ kind: "action", press: f }, null, map);
      }, 1000);
      return true;
    }
    keyPending = "";
    keyMenu = [];
    hints = false;
    if (fed.kind === "action") {
      runPress(fed.press);
      if (fed.then) handleFed(fed.then, e, map);
      return true;
    }
    if (fed.cancelled) {
      const moved = MOVED[fed.cancelled[0]];
      if (moved) say(moved);
      return true;
    }
    if (!e) return false;
    // Not a key here: a plugin's own listeners get it; on thumbdeck's panes, a key that moved says where
    const frame = pluginPage ? pageRef : keysToTab ? activeTab : null;
    if (frame) {
      frame.rawKey(e);
      return !e.ctrlKey && !e.altKey;
    }
    const name = keyName(e);
    if (MOVED[name]) return say(MOVED[name]), true;
    return false;
  }

  /** A finished press: thumbdeck's everywhere-keys, then the place's */
  function runPress(p: Press) {
    if (p.action.startsWith("toolkit:")) {
      const i = Number(p.action.slice(8));
      const a = details?.actions[i];
      const page = pages[i - (details?.actions.length ?? 0)];
      if (a) run(a);
      else if (page) showPageItem(page.page);
      return;
    }
    switch (p.action) {
      case "pane-side": case "pane-updown": return goPane(p.key.slice(-1) as "h" | "j" | "k" | "l");
      case "wide":
        wide = !wide;
        if (wide) pane = "center"; // the others are hidden
        return;
      case "page": {
        if (!details) return;
        const page = shownPages[Number(p.key.slice(-1)) - 1];
        if (page) showPageItem(page.page);
        pane = "center";
        return;
      }
      case "cycle-page": return cycleTabs(p.key === "gT" ? -times(p) : times(p));
      case "next-run": return cycleRuns(times(p));
      case "previous-run": return cycleRuns(-times(p));
      case "settings": return openSettings();
      case "command": return void openCmd(":");
      case "toolkit": return; // Space Space alone: the letters showed while it waited
      case "plugins":
        if (!views.length) return say("No plugin has a view: plugins with one show in the Plugins pane");
        wide = false;
        pane = "plugins";
        return;
    }
    if (pluginPage && pageRef) return pageRef.runPress(p);
    if (keysToTab && activeTab) return activeTab.runPress(p);
    if (paneAction(p)) return;
    switch (p.action) {
      case "add": if (selected) openForm(); break;
      case "stop": if (current && current.endedAt === null) stop(current); break;
      case "help": helpOpen = !helpOpen; break;
    }
  }

  /** f / F then a letter: the next (previous) item starting with it, round the list */
  function jumpLetter<T>(list: T[], at: number, p: Press, name: (x: T) => string): T | null {
    const back = p.action === "find-letter-back";
    const c = (p.arg ?? "").toLowerCase();
    for (let k = 1; k <= list.length; k++) {
      const i = (((at + (back ? -k : k)) % list.length) + list.length) % list.length;
      if (name(list[i]).toLowerCase().startsWith(c)) return list[i];
    }
    say(`Nothing here starts with ${p.arg}`);
    return null;
  }

  // The keys of the pane with the keyboard; false: not one of them (MAIN's may be)
  function paneAction(p: Press): boolean {
    const { action } = p;
    const n = times(p);
    if (action === "leave" && pane === "center" && visual) {
      visual = null;
      return true;
    }
    if (action === "leave") {
      if (pane === "center" && !shownView && !overviewShown && details) showTab("overview");
      else pane = "projects";
      return true;
    }
    if (pane === "projects") {
      switch (action) {
        case "down": move(n); break;
        case "up": move(-n); break;
        case "first": move("first"); break;
        case "last":
          if (p.count) (move("first"), move(p.count - 1));
          else move("last");
          break;
        case "open": pane = "center"; break;
        case "main": case "terminal": openInTmux(); break;
        case "search": openFilter(); break;
        case "find-letter": case "find-letter-back": {
          const to = jumpLetter(navOrder, navOrder.findIndex((x) => x.path === selected?.path), p, (x) => x.name);
          if (to) select(to);
          break;
        }
        case "new": pickFolder("add"); break;
        case "delete": if (selected) edit("remove", selected.path); break;
        case "pin": if (selected) edit("pin", selected.path); break;
        case "refresh": void invoke<ProjectList>("list_projects").then(applyList); say("Looked for projects again"); break;
        default: return false;
      }
      return true;
    }
    if (pane === "plugins") {
      const step = { down: n, up: -n, first: "first", last: "last" }[action] as number | "first" | "last" | undefined;
      if (step !== undefined) paneCursor = moveCursor(paneCursor, views.length, step);
      else if (action === "open" && views[paneCursor]) openView(views[paneCursor].plugin);
      else if (action === "find-letter" || action === "find-letter-back") {
        const to = jumpLetter(views, paneCursor, p, (v) => v.name);
        if (to) paneCursor = views.indexOf(to);
      } else return false;
      return true;
    }
    if (pane === "center") {
      if (overviewShown) return overviewRef?.run(p) ?? false;
      if (current) return outputKey(p);
      return readmeKey(p);
    }
    if (pane === "running") {
      const step = { down: n, up: -n, first: "first", last: "last" }[action] as number | "first" | "last" | undefined;
      const r = runList[runCursor];
      if (step !== undefined) runCursor = moveCursor(runCursor, runList.length, step);
      else if (action === "open" && r) showRun(r.id);
      else if ((action === "delete" || action === "stop") && r?.endedAt === null) stop(r);
      else return false;
      return true;
    }
    if (pane === "toolkit") {
      const a = kitOrder[kitCursor];
      const page = pages[kitCursor - kitOrder.length];
      if (["down", "up", "left", "right"].includes(action)) for (let i = 0; i < n; i++) kitCursor = kitMove(action);
      else if (action === "first" || action === "last") kitCursor = kitMove(action);
      else if (action === "press" && a) run(a);
      else if (action === "press" && page) showPageItem(page.page);
      else if (action === "edit" && a?.source === "custom") openForm(a);
      else if (action === "new") openForm();
      else if (action === "delete" && a) {
        const custom = a.source === "custom";
        void ask(custom ? `Delete your action ${a.label}?` : `Hide ${a.label} in this project? (Toolkit › show hidden brings it back)`, { title: a.label, kind: "warning", okLabel: custom ? "Delete" : "Hide" })
          .then((yes) => {
            if (yes) editActions(custom ? "delete" : "hide", a.id);
          });
      } else return false;
      return true;
    }
    return false;
  }

  // ------------------------------------------------------------ the : and / line
  let cmd = $state<{ kind: ":" | "/"; text: string } | null>(null);
  let cmdEl = $state<HTMLInputElement | null>(null);
  async function openCmd(kind: ":" | "/") {
    reader.reset();
    keyPending = "";
    cmd = { kind, text: "" };
    await tick();
    cmdEl?.focus();
  }
  /** :name: thumbdeck's, a plugin view's name, or one of the place's actions */
  const COMMANDS: Record<string, string> = {
    settings: "Settings › Plugins", plugins: "Settings › Plugins", q: "leave", w: "save the form", e: "refresh", wide: "only the center",
    help: "the keys here", action: "add your own action",
  };
  function commandNames(): string[] {
    const own = placeMap().bindings.map((b) => b.action).filter((a) => !a.startsWith("toolkit:"));
    return [...new Set([...Object.keys(COMMANDS), ...views.map((v) => v.name), ...own])];
  }
  function cmdKey(e: KeyboardEvent) {
    if (!cmd) return;
    if (e.key === "Escape") {
      e.preventDefault();
      cmd = null;
    } else if (e.key === "Enter") {
      e.preventDefault();
      const c = cmd;
      cmd = null;
      if (c.kind === "/") findText(c.text, 1);
      else command(c.text.trim());
    } else if (e.key === "Tab" && cmd.kind === ":") {
      e.preventDefault();
      const t = cmd.text.toLowerCase();
      const m = commandNames().filter((n) => n.toLowerCase().startsWith(t));
      if (m.length === 1) cmd.text = m[0];
      else if (m.length) say(m.slice(0, 14).join("  "));
    }
  }
  function command(text: string) {
    if (!text) return;
    if (review) {
      if (!reviewRef?.command(text)) say(`There's no :${text} on the review page (:w posts, :approve, :request, :q)`);
      return;
    }
    const [name, ...rest] = text.startsWith("!") ? ["!", text.slice(1)] : text.split(" ");
    const arg = rest.join(" ");
    const press = (action: string) => runPress({ action, key: `:${name}`, count: null, ...(arg ? { arg } : {}) });
    switch (name) {
      case "settings": case "plugins": return openSettings();
      case "q": case "quit": return press("leave");
      case "w": case "write":
        if (form) return void saveForm();
        break;
      case "e": case "edit": return press("refresh");
      case "wide": return press("wide");
      case "help": return void (helpOpen = true);
      case "action": if (selected) openForm(); return;
    }
    const view = views.find((v) => v.name.toLowerCase() === name.toLowerCase());
    if (view) return openView(view.plugin);
    if (placeMap().bindings.some((b) => b.action === name)) return press(name);
    const frame = pluginPage ? pageRef : keysToTab ? activeTab : null;
    if (frame) return frame.command(name, arg);
    say(`There's no :${name} here (Tab completes)`);
  }

  // ------------------------------------------------------------ the README and a run's output
  // / searches the text shown; n / N go to the next / previous match
  let searchFor = "";
  let searchAt = -1;
  function findText(text: string | null, by: number) {
    if (text !== null) [searchFor, searchAt] = [text, -1];
    const el = current ? outputEl : readmeEl;
    const q = searchFor.toLowerCase();
    if (!el || !q) return say("Nothing to look for: / first");
    const all: Range[] = [];
    const walk = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
    for (let node = walk.nextNode(); node; node = walk.nextNode()) {
      const t = node.textContent!.toLowerCase();
      for (let i = t.indexOf(q); i >= 0; i = t.indexOf(q, i + q.length)) {
        const r = document.createRange();
        r.setStart(node, i);
        r.setEnd(node, i + q.length);
        all.push(r);
      }
    }
    if (!all.length) return say(`"${searchFor}" isn't there`);
    searchAt = (((searchAt + by) % all.length) + all.length) % all.length;
    const r = all[searchAt];
    getSelection()?.removeAllRanges();
    getSelection()?.addRange(r);
    const box = r.getBoundingClientRect();
    const view = el.getBoundingClientRect();
    if (box.top < view.top || box.bottom > view.bottom) el.scrollBy({ top: box.top - view.top - view.height / 3 });
    say(`${searchAt + 1} of ${all.length}: ${searchFor}`);
  }

  function readmeKey(p: Press): boolean {
    switch (p.action) {
      case "search": openCmd("/"); return true;
      case "next-match": findText(null, times(p)); return true;
      case "previous-match": findText(null, -times(p)); return true;
      case "down": case "up": case "page-down": case "page-up": case "full-down": case "full-up": case "first": case "last":
        scrollCenter(p.action, times(p));
        return true;
    }
    return false;
  }

  // / (or the / by the title): the filter, in the Projects title
  let filterOpen = $state(false);
  async function openFilter() {
    filterOpen = true;
    await tick();
    filterEl?.focus();
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
  // The selected project, as frames get it
  const selectedInfo = $derived(selected ? { path: selected.path, name: selected.name, branch: selected.branch } : null);
  let runs = $state<Run[]>([]);
  let shownRun = $state<number | null>(null); // run whose output is in the center; null = README
  let now = $state(Date.now());
  let outputEl = $state<HTMLElement | null>(null);
  // macOS shows its window buttons over the top-left of the app (overlay title bar)
  const mac = navigator.userAgent.includes("Mac");

  const visible = $derived(
    projects.filter((p) => !p.hidden && p.name.toLowerCase().includes(filter.toLowerCase())),
  );
  // The project tree: Pinned, one section per scan folder, Added, Hidden.
  // A pinned project shows only under Pinned, so it's never on two rows at once.
  const sections = $derived.by(() => {
    const match = (p: Project) => p.name.toLowerCase().includes(filter.toLowerCase());
    const shown = projects.filter((p) => !p.hidden && match(p));
    const rest = shown.filter((p) => !p.pinned);
    const list = [
      { id: "pinned", label: "★ Pinned", root: null as string | null, items: shown.filter((p) => p.pinned) },
      ...roots.map((r) => ({ id: r, label: home(r), root: r as string | null, items: rest.filter((p) => p.root === r) })),
      { id: "added", label: "Added", root: null, items: rest.filter((p) => p.root === null) },
      { id: "hidden", label: "Hidden", root: null, items: projects.filter((p) => p.hidden && match(p)) },
    ];
    // Scan folders always show (so they can be removed); the others only when they have projects
    return list.filter((sec) => sec.items.length > 0 || sec.root !== null);
  });
  const folded = (id: string) => collapsed.includes(id) && !filter;
  // Projects in the order the tree shows them (each is in one section only)
  const navOrder = $derived(sections.filter((sec) => sec.id !== "hidden").flatMap((sec) => sec.items));

  const icons: Record<string, [string, string]> = {
    django: ["", "green"], python: ["", "yellow"], android: ["", "aqua"],
    node: ["", "green"], tauri: ["", "orange"], rust: ["", "orange"],
    go: ["", "blue"], nvim: ["", "green"], folder: ["", "grey"],
  };

  const home = (path: string) => path.replace(/^\/(home|Users)\/[^/]+/, "~");

  // Section of the tree a project sits in
  const sectionOf = (p: Project) => (p.hidden ? "hidden" : p.pinned ? "pinned" : (p.root ?? "added"));

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
  let reviewRef = $state<{ handleKey(e: KeyboardEvent): boolean; keymap(): import("$lib/keys/keys").Keymap; command(text: string): boolean; typedKeys(): string } | null>(null);
  let avatar = $state(""); // the character (settings); "none": no character
  let avatarRef = $state<{ pickCharacter(): void } | null>(null);
  let lastInput = $state(Date.now());
  // Your last key or click: the avatar falls asleep when there's none for a while
  $effect(() => {
    const input = () => (lastInput = Date.now());
    window.addEventListener("keydown", input, true);
    window.addEventListener("pointerdown", input, true);
    return () => {
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
        ]),
        update: update?.version ?? null,
        lastInput,
      },
      now,
    ),
  );
  // Where a click on the character goes, by what it shows (null: nowhere)
  const avatarGo = $derived.by((): { label: string; act: () => void } | null => {
    const m = avatarMood.mood;
    const ended = runs.filter((r) => r.endedAt !== null).sort((a, b) => b.endedAt! - a.endedAt!)[0];
    const live = runs.find((r) => r.endedAt === null);
    const agents = details?.tabs.findIndex((t) => t.plugin === "agents") ?? -1;
    if ((m === "done" || m === "failed") && ended) return { label: "its output", act: () => goToRun(ended) };
    if (m === "running" && live) return { label: "its output", act: () => goToRun(live) };
    if (m === "review" && views.some((v) => v.plugin === "prs")) return { label: "Pull requests", act: () => openView("prs") };
    if ((m === "waiting" || m === "thinking") && agents >= 0) return { label: "Agents", act: () => showTab(agents) };
    if (m === "update" && update) return { label: "what's new", act: () => (updateDialog = true) };
    return null;
  });
  /** A run's output, in its project */
  async function goToRun(r: Run) {
    const p = projects.find((x) => x.path === r.projectPath);
    if (p && p.path !== selected?.path) await select(p);
    showRun(r.id);
    pane = "center";
  }
  let updateDialog = $state(false);
  let updateState = $state<"idle" | "installing" | "installed">("idle");
  let updateError = $state("");

  let checking = $state(false);
  async function checkForUpdates() {
    checking = true;
    try {
      update = await invoke<Update | null>("check_update");
      if (update) updateDialog = true;
      else say("thumbdeck is up to date");
    } catch (err) {
      say(String(err), true);
    } finally {
      checking = false;
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
  // The Running pane lists only what's running, so it's one line when nothing is; ▸ (or giving
  // it the keyboard) shows the finished runs too
  const liveRuns = $derived(projectRuns.filter((r) => r.endedAt === null).length);
  let allRuns = $state(false);
  const runList = $derived(allRuns || pane === "running" ? projectRuns : projectRuns.filter((r) => r.endedAt === null));
  // Runs aren't tabs: one Output tab shows the run picked last in Running (at first the newest),
  // so the tab row stays the same length however many runs there are
  let outputRun = $state<number | null>(null);
  const outputTab = $derived(projectRuns.find((r) => r.id === outputRun) ?? projectRuns[0] ?? null);

  function showRun(id: number) {
    outputRun = id;
    shownRun = id;
  }
  // An extension tab is in the center (not the README or a run's output)
  const tabShown = $derived(shownRun === null && shownTab !== null && !!details?.tabs[shownTab]);
  // The Overview is in the center
  const overviewShown = $derived(!shownView && shownRun === null && !tabShown && shownPage === "overview" && !!details);
  // The keys of the pane with the keyboard (the center's, named after what it shows)
  const paneKeys = $derived(
    pane === "plugins" ? PLUGINS
    : pane === "center" && overviewShown ? (overviewRef?.keymap() ?? OVERVIEW)
    : pane === "center" && current ? (visual ? OUTPUT_VISUAL : OUTPUT)
    : pane === "center" ? { ...CENTER, name: shownView ? "VIEW" : tabShown ? "TAB" : "README" }
    : pane === "running" ? RUNNING
    : pane === "toolkit" ? KIT
    : PROJECTS,
  );
  /** Who has the keyboard, for the status bar at the bottom */
  const barKeys = $derived(
    review ? (reviewRef?.keymap() ?? REVIEW)
    : pluginPage && pageRef ? pageRef.keymap()
    : cmd ? { ...COMMAND, name: cmd.kind === "/" ? "SEARCH" : "COMMAND" }
    : form || setupForm || helpOpen || updateDialog || settingsOpen || catalogOffer || whatsNew ? DIALOG
    : filtering ? FILTER
    : hints ? TOOLKIT
    : keysToTab && activeTab ? activeTab.keymap()
    : paneKeys,
  );
  // Toolkit grouped by where each action came from: yours, then one group per pack
  const groups = $derived.by(() => {
    const byGroup = new Map<string, Action[]>();
    for (const a of details?.actions ?? []) byGroup.set(a.source, [...(byGroup.get(a.source) ?? []), a]);
    return [...byGroup.values()].map((actions) => [actions[0].group, actions] as const);
  });
  // The Toolkit's buttons in the order they show (j / k go through them)
  const kitOrder = $derived(groups.flatMap(([, actions]) => actions));

  // Pages: what the center can show for the project, listed in the Toolkit (and on 1–9):
  // Overview, README, then its plugin tabs A–Z; then the ones it could add (+, A–Z, they open
  // their setup). There's no tab row: the center's header names the page it shows.
  type Page = { kind: "overview" | "readme" } | { kind: "tab"; index: number } | { kind: "add"; id: string };
  interface PageItem { page: Page; title: string; icon: string | null; glyph: string; badge: string | null }
  const pages = $derived.by((): PageItem[] => {
    if (!details || !selected) return [];
    const path = selected.path;
    const tabs = details.tabs.map((t, index) => ({ t, index })).sort((a, b) => a.t.title.localeCompare(b.t.title));
    const added = new Set(details.tabs.map((t) => `${t.plugin}:${t.tab}`));
    return [
      { page: { kind: "overview" }, title: "Overview", icon: null, glyph: "◉", badge: null },
      { page: { kind: "readme" }, title: "README", icon: null, glyph: "≡", badge: null },
      ...tabs.map(({ t, index }): PageItem => ({ page: { kind: "tab", index }, title: t.title, icon: t.icon ?? null, glyph: "▤", badge: badges[frameKeyOf(path, index, t)] ?? null })),
      ...details.addable.filter((a) => !added.has(a.id)).sort((a, b) => a.name.localeCompare(b.name))
        .map((a): PageItem => ({ page: { kind: "add", id: a.id }, title: a.name, icon: a.icon ?? null, glyph: "+", badge: null })),
    ];
  });
  // The Toolkit's buttons and pages, for its cursor
  const kitTotal = $derived(kitOrder.length + pages.length);
  // The pages 1–9 pick (not the ones still to add)
  const shownPages = $derived(pages.filter((p) => p.page.kind !== "add"));
  function showPageItem(p: Page, keys = true) {
    if (p.kind === "add") return void addTab(p.id);
    showTab(p.kind === "tab" ? p.index : p.kind, keys);
  }
  const pageOn = (p: Page) =>
    !shownView && shownRun === null && (p.kind === "tab" ? tabShown && shownTab === p.index : p.kind !== "add" && !tabShown && shownPage === p.kind);
  // What the center shows, for its header (a run's output: current)
  const pageNow = $derived(current ? null : (pages.find((x) => pageOn(x.page)) ?? null));

  async function select(p: Project) {
    selected = p;
    shownView = null;
    invoke("edit_projects", { change: "last", path: p.path }); // remembered for the next start
    shownRun = null;
    runCursor = 0;
    kitCursor = 0;
    showTab("overview", false);
    details = null;
    loadPanels();
    const d = await invoke<Details>("project_details", { path: p.path });
    if (selected?.path !== p.path) return; // another project was picked meanwhile
    details = d;
    // Open on the tab shown last in this project
    const last = d.tabs.findIndex((t) => `${t.plugin}:${t.tab}` === d.last_tab);
    if (last >= 0) showTab(last, false);
    else if (d.last_tab === "readme") showTab("readme", false);
  }

  // The tab shown, remembered for the project's next opening (a run's output isn't: runs end
  // with the app)
  $effect(() => {
    const path = selected?.path;
    const d = details;
    if (!path || !d || shownView) return;
    const t = shownTab === null ? null : d.tabs[shownTab];
    const id = t ? `${t.plugin}:${t.tab}` : shownPage;
    if (id === untrack(() => d.last_tab)) return;
    d.last_tab = id;
    invoke("remember_tab", { path, tab: id });
  });

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

  /** Start a command in a project: in Running, its output in the Output tab (shown when `show`) */
  async function startRun(p: { path: string }, command: string, label: string, source: string, show: boolean) {
    const id = await invoke<number>("run_action", { path: p.path, command, label });
    runs.push({ id, projectPath: p.path, label, source, command, startedAt: Date.now(), endedAt: null, code: null, lines: [] });
    if (show) showRun(id);
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

  // The first start of 1.0 shows what changed in the keys (once)
  onMount(() => {
    try {
      if (!localStorage.getItem("thumbdeck.keys-1.0")) {
        localStorage.setItem("thumbdeck.keys-1.0", "seen");
        whatsNew = true;
      }
    } catch {}
  });
  onMount(() => {
    invoke<ProjectList>("list_projects").then((list) => applyList(list, true));
    getVersion().then((v) => (version = v));
    loadPluginViews();
    // The first start with plugins: offer the official ones (quietly skipped when offline)
    invoke<{ plugins: CatalogEntry[]; offered: boolean }>("plugin_catalog")
      .then((c) => !c.offered && c.plugins.length && (catalogOffer = c.plugins))
      .catch(() => {});
    // A linked plugin's files changed: its frames load again
    const unlistenChanged = listen<string>("plugin-changed", async (e) => {
      const id = e.payload;
      reloads[id] = (reloads[id] ?? 0) + 1;
      liveFrames = liveFrames.filter((f) => f.tab.plugin !== id);
      liveViews = liveViews.filter((v) => v.plugin !== id);
      if (shownView === id) openView(id, keysToTab);
      if (pluginPage?.info.plugin === id) pluginPage = null;
      say(`${id} changed: reloaded`);
      await pluginsChanged();
    });
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
      else if (method === "ui.status") statuses[plugin] = params?.text ?? null;
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
        // Remembered for the Overview's Last runs
        const last = { code: r.code, ended: r.endedAt, seconds: Math.round((r.endedAt - r.startedAt) / 1000) };
        invoke("remember_run", { path: r.projectPath, label: r.label, run: last });
        if (details && selected?.path === r.projectPath) details.last_runs[r.label] = last;
      }
    });
    const timer = setInterval(() => (now = Date.now()), 1000);
    return () => {
      unlistenOut.then((f) => f());
      unlistenExit.then((f) => f());
      unlistenUpdate.then((f) => f());
      unlistenPlugins.then((f) => f());
      unlistenChanged.then((f) => f());
      unlistenActions.then((f) => f());
      unlistenBackendUi.then((f) => f());
      clearInterval(timer);
      clearInterval(idle);
    };
  });
</script>

<svelte:window onclick={closeMenuOutside} onmousedown={mouseDown} onkeydown={onKey} />
<div class="drag" data-tauri-drag-region></div>
<main class:mac class:wide class:viewing={!!shownView}>
  <!-- ------------------------------------------------------------ projects -->
  <aside class="panel left">
    <header class="app" data-tauri-drag-region>
      <button class="title" title="Pick a character (it sits on the Toolkit), or none" onclick={() => avatarRef?.pickCharacter()}>thumbdeck</button>
      <span class="spacer"></span>
      {#if update}
        <button class="update-dot" title="thumbdeck {update.version} is available" onclick={() => (updateDialog = true)}>● {update.version}</button>
      {/if}
      <button class="header-btn" title="Check for a newer thumbdeck" disabled={checking} onclick={checkForUpdates}>{checking ? "…" : "↻"}</button>
    </header>
    <section class="card helm" class:focus={pane === "projects"} data-pane="projects">
      <h2>
        <span class="dot purple"></span>
        <!-- The filter takes the title's place while you use it (/), and stays while it has text -->
        {#if filterOpen || filter}
          <input class="filter" placeholder="Filter projects…" bind:value={filter} bind:this={filterEl} onkeydown={filterKey}
                 onfocus={() => (filtering = true)} onblur={() => { filtering = false; filterOpen = false; }} />
        {:else}
          Projects <button class="icon find" title="Filter the projects (/)" onclick={openFilter}>/</button>
        {/if}
        <span class="count">{visible.length}</span>
        <span class="menu-anchor" bind:this={addMenuEl}>
          <button class="icon" title="Add…" onclick={() => (addMenu = !addMenu)}>+</button>
          {#if addMenu}
            <div class="menu">
              <button onclick={() => pickFolder("add")}>Add project…</button>
              <button onclick={() => pickFolder("add-root")}>Add folder to scan…</button>
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
                  <button class="pick" class:active={selected?.path === p.path && !p.hidden && !shownView}
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
    <!-- The plugins with a view of their own (Ctrl+p), and the plugins' settings (,) -->
    <section class="card pane" class:focus={pane === "plugins"} data-pane="plugins">
        <h2>
          <span class="dot aqua"></span>Plugins
          <button class="icon settings-btn" title="Settings: plugins (Space ,){Object.keys(pluginUpdates).length ? ` · ${Object.keys(pluginUpdates).length} plugin update${Object.keys(pluginUpdates).length === 1 ? '' : 's'}` : ''}"
                  onclick={openSettings}>⚙{#if Object.keys(pluginUpdates).length}<span class="menu-new">↑{Object.keys(pluginUpdates).length}</span>{/if}</button>
        </h2>
        {#if !views.length}<p class="hint">Plugins with a view of their own show here.</p>{/if}
        <ul>
          {#each views as v, i (v.plugin)}
            <li class="project">
              <button class="pick" class:active={shownView === v.plugin} class:cursor={pane === "plugins" && i === paneCursor}
                      onclick={() => openView(v.plugin)}>
                {#if v.icon}<img class="kind picon" src={v.icon} alt="" />{:else}<span class="kind">◇</span>{/if}<span class="name">{v.name}</span>
                {#if v.status && statuses[v.plugin]}<span class="vstatus">{statuses[v.plugin]}</span>{/if}
              </button>
            </li>
          {/each}
        </ul>
    </section>
  </aside>

  <!-- ------------------------------------------------------------ center -->
  <section class="panel center" class:focus={pane === "center"} bind:this={centerEl} data-pane="center">
    <!-- A plugin's view (from the Plugins pane), in place of the project; kept alive -->
    <div class="side" class:gone={!shownView}>
      <nav class="crumbs">
        <span class="path">Plugins</span><span> / </span>
        <strong class="name">{views.find((v) => v.plugin === shownView)?.name ?? ""}</strong>
        <span class="spacer"></span>
        {#if selected}<button class="tab" title="Back to the project (1)" onclick={() => showTab(shownPage, false)}>‹ {selected.name}</button>{/if}
      </nav>
      <div class="view">
        {#each liveViews as v (v.key)}
          <PluginFrame bind:this={viewRefs[v.key]} info={v.frame} frameKey="view:{v.plugin}" surface="view" surfaceId=""
                       project={selectedInfo} setup={null} active={keysToTab && shownViewKey === v.key} visible={shownViewKey === v.key}
                       thumbdeck={version} {host} {say} onActivate={() => shownViewKey === v.key && (pane = "center")}
                       onRelease={() => (pane = "projects")} onEditSetup={() => {}} openReview={(r) => (review = r)} />
        {/each}
      </div>
    </div>
    <div class="side" class:gone={!!shownView}>
    {#if selected}
      <nav class="crumbs">
        <span class="path" title={selected.path}>{home(selected.path).split("/").slice(0, -1).join(" / ")}</span>
        <span> / </span><strong class="name">{selected.name}</strong>
        {#if selected.branch}<span class="branch">  {selected.branch}</span>{/if}
        <!-- What the center shows (the pages are in the Toolkit) -->
        <span class="pagenow">›
          {#if current}<span class="st {status(current)}">{current.endedAt === null ? "●" : current.code === 0 ? "✓" : "✗"}</span> Output · {current.label}
          {:else if pageNow}{#if pageNow.icon}<img class="picon" src={pageNow.icon} alt="" />{:else}<span class="bicon">{pageNow.glyph}</span>{/if} {pageNow.title}{/if}
        </span>
        <span class="spacer"></span>
        <button class="tab open" title="Open in tmux (Enter)" onclick={openInTmux}> <span class="bword">Open in tmux</span></button>
        <button class="tab" title={wide ? "Show the side panels again (Ctrl+w o)" : "Expand: the center takes the whole window (Ctrl+w o)"}
                onclick={() => (wide = !wide)}>{wide ? "⤡" : "⤢"}</button>
      </nav>
      <div class="view">
      {#each liveFrames as f (f.key)}
        <PluginFrame bind:this={frameRefs[f.key]} info={f.tab.frame!} frameKey={f.key} surface="tab" surfaceId={f.tab.tab}
                     project={f.project} setup={f.tab.setup} active={keysToTab && shownFrameKey === f.key}
                     visible={shownFrameKey === f.key && !current} thumbdeck={version} {host} {say}
                     onActivate={() => shownFrameKey === f.key && (pane = "center")} onRelease={() => (pane = "projects")}
                     onEditSetup={() => f.project.path === selected?.path && (setupForm = { index: f.index, tab: f.tab })}
                     openReview={(r) => (review = r)} />
      {/each}
      {#if current}
        <pre class="output" bind:this={outputEl}><span class="cmd">$ {current.command}</span>
{#each current.lines as l, i}<span class:err={l.stderr} class:vsel={i >= visualFrom && i <= visualTo} class:vcur={visual?.cursor === i}>{l.text}</span>
{/each}{#if current.endedAt !== null}<span class={current.code === 0 ? "ok" : "err"}>{current.code === 0 ? "✓ finished" : `✗ exited with ${current.code}`} after {elapsed(current)}</span>{/if}</pre>
      {:else if tabShown && details && shownTab !== null}
        {@const t = details.tabs[shownTab]}
        {#if !t.frame}
          <!-- (a working plugin's tab is its frame, above, kept alive) -->
          <div class="empty">
            <p>{t.missing}</p>
            <button class="ghost" onclick={() => { setupForm = { index: shownTab, tab: t }; saveTab("remove"); }}>Remove this tab</button>
          </div>
        {/if}
      {:else if overviewShown}
        <!-- (the Overview, below, kept while the project is selected) -->
      {:else if details?.readme}
        <article class="readme" bind:this={readmeEl}>{@html marked.parse(details.readme)}</article>
      {:else}
        <p class="empty">{details ? "No README in this project." : "Loading…"}</p>
      {/if}
      <!-- The Overview stays while the project is selected (its cards don't load again each time) -->
      {#if details}
        <Overview bind:this={overviewRef} project={selected} {details} runs={projectRuns} {cards} visible={overviewShown}
                  active={pane === "center"} {now} thumbdeck={version} {host} {badges} {reloads} {say}
                  openReview={(r) => (review = r)} toToolkit={() => (pane = "toolkit")} onFocus={() => (pane = "center")}
                  hideCard={async (card, hide) => {
                    const path = selected?.path;
                    const d = await invoke<Details>("hide_card", { path, card, hide });
                    if (selected?.path === path) details = d;
                  }}
                  openTab={(plugin, tab) => {
                    const i = details?.tabs.findIndex((t) => t.plugin === plugin && t.tab === tab) ?? -1;
                    if (i >= 0) showTab(i);
                    return i >= 0;
                  }} />
      {/if}
      </div>
    {:else}
      <p class="empty">No projects found in ~/dev, ~/projects or your home folder.</p>
    {/if}
    </div>
  </section>

  <!-- ------------------------------------------------------------ running + toolkit -->
  <aside class="panel right">
    <section class="card running" class:focus={pane === "running"} class:idle={runList.length === 0} data-pane="running">
      <h2>
        <span class="dot blue"></span>Running
        {#if liveRuns === 0}<span class="for">{projectRuns.length ? "nothing now" : "nothing yet"}</span>{:else}<span class="for live">{liveRuns} running</span>{/if}
        {#if projectRuns.length > liveRuns}
          <button class="icon" title={allRuns ? "Hide the finished runs" : "Show the finished runs"}
                  onclick={() => (allRuns = !allRuns)}>{allRuns ? "▾" : `▸${projectRuns.length - liveRuns}`}</button>
        {/if}
      </h2>
      {#each runList as r, i (r.id)}
        <div class="run" class:shown={r.id === shownRun} class:cursor={pane === "running" && i === runCursor} role="button" tabindex="0"
             onclick={() => showRun(r.id)} onkeydown={(e) => e.key === "Enter" && showRun(r.id)}>
          <span class="st {status(r)}" title={status(r)}>{r.endedAt === null ? "●" : r.code === 0 ? "✓" : "✗"}</span>
          <span class="label">{r.label}</span>
          <span class="time">{elapsed(r)}</span>
          {#if r.endedAt === null}
            <button class="stop" title="Stop" onclick={(e) => { e.stopPropagation(); stop(r); }}>■</button>
          {/if}
        </div>
      {/each}
    </section>

    <!-- The Toolkit, with the character perched on its bottom edge -->
    <div class="kit-wrap">
    <section class="card toolkit" class:focus={pane === "toolkit"} data-pane="toolkit">
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
              <button class="action" class:custom={a.source === "custom"} class:cursor={pane === "toolkit" && kitOrder[kitCursor] === a} onclick={() => run(a)}
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
      {#if pages.length}
        <h3>Pages</h3>
        <div class="grid">
          {#each pages as p, j (p.page.kind === "tab" ? `t${p.page.index}` : p.page.kind === "add" ? `a${p.page.id}` : p.page.kind)}
            {@const hint = HINT_KEYS[(details?.actions.length ?? 0) + j]}
            {@const n = shownPages.indexOf(p) + 1}
            <button class="action page" class:add={p.page.kind === "add"} class:on={pageOn(p.page)} class:cursor={pane === "toolkit" && kitCursor === kitOrder.length + j}
                    title={p.page.kind === "add" ? `Add ${p.title} to this project (opens its setup)` : `Show ${p.title}${n && n <= 9 ? ` (${n})` : ""}`}
                    onclick={() => showPageItem(p.page)}>
              {#if hints && hint}<span class="hint-key">{hint}</span>
              {:else if p.icon}<img class="picon" class:dim={p.page.kind === "add"} src={p.icon} alt="" />
              {:else}<span class="play">{p.glyph}</span>{/if}
              <span class="alabel">{p.page.kind === "add" && p.icon ? `+ ${p.title}` : p.title}</span>
              {#if p.badge}<span class="tbadge">{p.badge}</span>{:else if n && n <= 9}<span class="pnum">{n}</span>{/if}
            </button>
          {/each}
        </div>
      {/if}
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
      <Avatar bind:this={avatarRef} mood={avatarMood.mood} caption={avatarMood.caption} character={avatar}
        go={avatarGo?.label ?? null} onGo={() => avatarGo?.act()}
        onPick={async (id) => applyList(await invoke<ProjectList>("edit_projects", { change: "avatar", path: id }))} />
    </div>
    {#each panels as p (`${p.plugin}:${p.id}:${p.scope === "project" ? selected?.path : ""}:${reloads[p.plugin] ?? 0}`)}
      {@const key = `panel:${p.plugin}:${p.id}`}
      <section class="card ppanel">
        <h2><span class="dot blue"></span>{p.name}{#if badges[key]}<span class="tbadge">{badges[key]}</span>{/if}</h2>
        <PluginFrame info={p.frame} frameKey={key} surface="panel" surfaceId={p.id} project={selectedInfo} setup={null}
                     active={false} visible={true} lines={p.lines} thumbdeck={version} {host} {say}
                     onActivate={() => {}} onRelease={() => {}} onEditSetup={() => {}} openReview={(r) => (review = r)} />
      </section>
    {/each}
  </aside>
</main>


<StatusBar mode={barKeys.name} project={selected?.name ?? null} branch={branchState} message={toast} keys={review ? (reviewRef?.typedKeys() ?? "") : keyPending} />

{#if cmd}
  <!-- the : command line (or / search), over the status bar, like vim's -->
  <div class="cmdline">
    <span class="cmdkind">{cmd.kind}</span>
    <input bind:this={cmdEl} bind:value={cmd.text} onkeydown={cmdKey} onblur={() => (cmd = null)} spellcheck="false" autocomplete="off"
           aria-label={cmd.kind === ":" ? "Command" : "Search"} placeholder={cmd.kind === ":" ? "settings, wide, q, a plugin's name… (Tab completes)" : "search the text"} />
  </div>
{/if}

{#if keyMenu.length}
  <!-- after Space: what can come next, like which-key in nvim -->
  <div class="whichkey" role="status">
    <div class="wk-title">{keyPending.trim()} …</div>
    <div class="wk-grid">
      {#each keyMenu.filter(([k, b]) => b.does && !b.action.startsWith("toolkit:") && !k.includes(" ")) as [k, b] (k)}<span class="wk-key">{k}</span><span class="wk-does">{b.does}</span>{/each}
      {#if keyMenu.some(([k]) => k.startsWith("y "))}<span class="wk-key">y …</span><span class="wk-does">copy as: {keyMenu.filter(([k]) => k.startsWith("y ")).map(([k, b]) => `${k.slice(2)} ${b.does.replace(/^copy (it )?(as )?/, "")}`).join(" · ")}</span>{/if}
      {#if keyMenu.some(([, b]) => b.action === "page")}<span class="wk-key">1–9</span><span class="wk-does">a page (Overview, README, then the project's)</span>{/if}
      {#if keyMenu.some(([k, b]) => b.action.startsWith("toolkit:") && !k.includes(" "))}<span class="wk-key">letter</span><span class="wk-does">the Toolkit button with that letter (shown on the buttons)</span>{/if}
    </div>
  </div>
{/if}

{#if whatsNew}
  <div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && (whatsNew = false)}>
    <div class="dialog help">
      <h2><span class="dot green"></span>Keys in thumbdeck 1.0 <span class="for">Esc to close</span></h2>
      <p class="hint">Two rules now: a vim key means what it means in vim, everywhere; the rest is behind Space (letters shared by every place) or on the : line.</p>
      <dl>
        <dt>Ctrl+d / Ctrl+u</dt><dd>half a page (d / u alone did that: dd deletes, u undoes now)</dd>
        <dt>gg / G · 5j · 3dd</dt><dd>top / bottom, and counts in front</dd>
        <dt>v / V · yy · p</dt><dd>select, copy, paste</dd>
        <dt>dd · x · cc · o · i</dt><dd>delete (or stop, hide) · tick · rename · new · edit</dd>
        <dt>u · Ctrl+r · .</dt><dd>undo · redo · do it again</dd>
        <dt>]c ]t ]f ]e ]r</dt><dd>the next change, thread, file, error, run ([ for the previous)</dd>
        <dt>za · zR · zM</dt><dd>folds (z was wide: that's Ctrl+w o now)</dd>
        <dt>Enter · gd · gx · R</dt><dd>the main action · the diff · open outside · refresh</dd>
        <dt>Space</dt><dd>the place's actions: s stop, p pin, f filter, o sort, h hidden, i info, y copy as…</dd>
        <dt>Space Space</dt><dd>the Toolkit, from anywhere · Space 1–9 the pages · gt / gT the next / previous page</dd>
        <dt>:</dt><dd>commands: :settings, :wide, :q, :w, a plugin's name…</dd>
      </dl>
      <p class="hint">For a while, a key that moved says where it went instead of doing something. ? lists the keys wherever you are.</p>
    </div>
  </div>
{/if}

{#if review}
  <ReviewPage bind:this={reviewRef} request={review} onClose={() => (review = null)} {say} />
{/if}

{#if pluginPage}
  <div class="plugin-page">
    {#key pluginPage.key}
      <PluginFrame bind:this={pageRef} info={pluginPage.info} frameKey={pluginPage.key} surface="page" surfaceId={pluginPage.id}
                   project={pluginPage.project} data={pluginPage.data} setup={null} active={true} visible={true}
                   thumbdeck={version} {host} {say} onActivate={() => {}} onRelease={() => (pluginPage = null)}
                   onEditSetup={() => {}} openReview={(r) => (review = r)} />
    {/key}
  </div>
{/if}

{#if catalogOffer}
  <CatalogDialog plugins={catalogOffer} onDone={() => { catalogOffer = null; pluginsChanged(); }} />
{/if}

{#if helpOpen}
  <div class="backdrop" role="presentation" onclick={() => (helpOpen = false)}>
    <div class="dialog help">
      <h2><span class="dot purple"></span>Keys <span class="for">? or Esc to close</span></h2>
      <dl>
        <dt class="group">{paneKeys.name}</dt><dd></dd>
        {#each paneKeys.bindings as b}<dt>{keysLabel(b)}</dt><dd>{b.does}</dd>{/each}
        <dt class="group">EVERYWHERE</dt><dd></dd>
        {#each MAIN.bindings as b}<dt>{keysLabel(b)}</dt><dd>{b.does}</dd>{/each}
        <dt>in a tab</dt><dd>the same keys mean the same things: j k gg G move, l in, h out, Enter the main action, Tab the next list,
          / search, v select, yy copy, dd delete, o new, i edit, R refresh, gx open outside, Space its actions, Space S its setup, ? its keys, Esc or q give the keyboard back</dd>
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
  {@const frame = setupForm.tab.frame}
  {#if frame}
    <PluginSetupForm name={frame.name} pluginName={frame.plugin_name} description="" fields={frame.fields} setup={setupForm.tab.setup}
                     isNew={setupForm.index === null} project={selected.name} {frame} tabId={setupForm.tab.tab}
                     projectInfo={{ path: selected.path, name: selected.name, branch: selected.branch }} {host} thumbdeck={version} {say}
                     onSave={(setup) => saveTab(setupForm?.index === null ? "add" : "save", setup)}
                     onRemove={() => saveTab("remove")} onCancel={() => (setupForm = null)} />
  {:else}
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
  /* A plugin's view (not a project's): no Running and Toolkit */
  main.viewing { grid-template-columns: 250px 1fr; }
  main.viewing > .right { display: none; }
  main.wide { grid-template-columns: 1fr; }
  main.wide > .left, main.wide > .right { display: none; }
  .panel { min-height: 0; display: flex; flex-direction: column; gap: 8px; }
  .card { background: var(--bg0); border: 1px solid var(--bg2); border-radius: 12px; padding: 12px; }
  h2 { display: flex; align-items: center; gap: 8px; margin: 0 0 10px; font: 600 13px var(--mono); }
  h3 { margin: 10px 0 6px; font: 500 11px var(--mono); color: var(--grey); text-transform: uppercase; letter-spacing: 0.08em; }
  :global(.dot) { width: 8px; height: 8px; border-radius: 50%; }
  :global(.dot.purple) { background: var(--purple); } :global(.dot.green) { background: var(--green); } :global(.dot.orange) { background: var(--orange); }
  :global(.dot.aqua) { background: var(--aqua); } :global(.dot.blue) { background: var(--blue); }
  .count, :global(.for) { margin-left: auto; color: var(--grey); font-weight: 400; }
  :global(.hint), .empty { color: var(--grey); font-size: 13px; }
  .hint.problem { color: var(--red); font-size: 12px; margin: 0 0 6px; }
  .branch { color: var(--purple); font: 12px var(--mono); }

  /* left */
  .app .title { font: inherit; padding: 0; background: none; }
  .app { font: 700 22px var(--mono); padding: 4px 6px; display: flex; align-items: center; gap: 8px; }
  .update-dot { padding: 1px 8px; border-radius: 10px; background: var(--bg1); color: var(--green); font: 600 11px var(--mono); }
  .update-dot:hover { background: var(--bg2); }
  .header-btn { padding: 2px 7px; border-radius: 8px; background: none; color: var(--grey); font: 16px var(--sans); }
  .header-btn:hover { background: var(--bg1); color: var(--fg); }
  /* Settings, in the Plugins title (with the plugin updates waiting) */
  h2 .icon.settings-btn { margin-left: auto; width: auto; padding: 0 5px; font-size: 14px; }
  .settings-btn .menu-new { margin-left: 3px; font: 600 11px var(--mono); }
  .notes { max-height: 40vh; padding: 0 4px; font-size: 13px; }
  .filter { flex: 1; min-width: 0; margin: -4px 0; background: var(--bg-dim); border: 1px solid var(--bg2); border-radius: 6px; padding: 3px 8px; color: var(--fg); font: 12.5px var(--mono); outline: none; }
  h2 .icon.find { margin-left: 0; width: 18px; height: 18px; opacity: 0.6; }
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
  .crumbs { display: flex; align-items: center; gap: 4px; padding: 6px 10px 6px 14px; border-bottom: 1px solid var(--bg2); font: 12px var(--mono); color: var(--grey); overflow: hidden; container-type: inline-size; }
  .crumbs strong { color: var(--fg); }
  /* One line: the path gives way first (… at its end), the name, branch and buttons stay whole */
  .crumbs > :not(.spacer) { flex: none; white-space: nowrap; }
  .crumbs > .path { flex: 0 1000 auto; min-width: 2ch; overflow: hidden; text-overflow: ellipsis; }
  .spacer { flex: 1; }
  .tab { padding: 3px 10px; border-radius: 6px; font: 12px var(--mono); color: var(--grey); }
  .tab.on { background: var(--bg2); color: var(--fg); }
  .view { flex: 1; min-height: 0; display: flex; flex-direction: column; overflow: hidden; }
  .view > .readme { overflow: auto; }
  @container (max-width: 640px) { .crumbs .branch, .tab.open .bword { display: none; } }
  /* What the center shows, after the project's name */
  .crumbs > .pagenow { display: flex; align-items: center; gap: 6px; margin-left: 8px; color: var(--fg); }
  .bicon { flex: none; color: var(--grey); }
  .picon { width: 14px; height: 14px; flex: none; }
  .picon.dim { opacity: 0.55; }
  .st.running { color: var(--running); } .st.done { color: var(--green); } .st.failed { color: var(--red); }
  .menu-new { color: var(--green); font-weight: 600; }
  .side { flex: 1; min-height: 0; display: flex; flex-direction: column; }
  .side.gone { display: none; }
  .pane { flex: none; max-height: 35%; overflow: auto; }
  /* The pane with the keyboard: a 2px border (the shadow adds the second px, so nothing moves)
     and its title in the same colour, so it's plain which one it is at a glance */
  .card.focus, .center.focus { border-color: var(--pane-focus, var(--focus)); box-shadow: 0 0 0 1px var(--pane-focus, var(--focus)), 0 0 14px color-mix(in srgb, var(--pane-focus, var(--focus)) 25%, transparent); }
  .card.focus > h2 { color: var(--pane-focus, var(--focus)); }
  /* Each pane's focus in its dot's colour (the center keeps --focus) */
  .card[data-pane="projects"] { --pane-focus: var(--purple); }
  .card[data-pane="plugins"] { --pane-focus: var(--aqua); }
  .card[data-pane="running"] { --pane-focus: var(--blue); }
  .card[data-pane="toolkit"] { --pane-focus: var(--orange); }
  .center.focus .crumbs { background: color-mix(in srgb, var(--focus) 12%, transparent); }
  .center.focus .crumbs strong { color: var(--focus); }
  .run.cursor, .action.cursor { background: var(--cursor-bg); box-shadow: inset 2px 0 var(--cursor); }
  .helm.focus .pick.active { box-shadow: inset 2px 0 var(--cursor); }
  .pane ul { list-style: none; margin: 0; padding: 0; }
  .pick.cursor { box-shadow: inset 2px 0 var(--cursor); background: var(--cursor-bg); }
  .vstatus { margin-left: auto; color: var(--grey); font: 11px var(--mono); }
  .ppanel { flex: none; display: flex; flex-direction: column; padding-bottom: 8px; }
  .ppanel h2 { margin-bottom: 6px; }
  .plugin-page { position: fixed; inset: 0 0 22px 0; z-index: 40; background: var(--bg0); display: flex; }
  .tbadge { margin-left: 6px; padding: 0 6px; border-radius: 99px; background: var(--bg3); color: var(--fg); font-size: 10.5px; }
  .readme { padding: 8px 28px 28px; overflow: auto; max-width: 860px; }
  .readme :global(img) { max-width: 100%; height: auto; border-radius: 6px; }
  .readme :global(h1), .readme :global(h2) { font-family: var(--mono); border-bottom: 1px solid var(--bg2); padding-bottom: 6px; }
  .readme :global(code) { font-family: var(--mono); background: var(--bg1); padding: 1px 5px; border-radius: 4px; color: var(--aqua); }
  .readme :global(pre) { background: var(--bg1); padding: 12px; border-radius: 8px; overflow: auto; }
  .readme :global(pre code) { background: none; padding: 0; color: var(--fg); }
  .readme :global(a) { color: var(--blue); }
  .output { flex: 1; margin: 0; padding: 14px; overflow: auto; font: 12.5px/1.45 var(--mono); white-space: pre-wrap; word-break: break-word; }
  .output .cmd { color: var(--grey); }
  .output .err { color: var(--red); } .output .ok { color: var(--green); }
  .output .vsel { display: inline-block; width: 100%; background: color-mix(in srgb, var(--yellow) 18%, transparent); }
  .output .vcur { box-shadow: inset 2px 0 var(--cursor, var(--orange)); }
  .empty { padding: 20px; }

  /* right */
  .card.running { flex: none; max-height: 45%; overflow: auto; }
  /* nothing listed: just the title line */
  .card.running.idle { padding-block: 8px; }
  .card.running.idle h2 { margin: 0; }
  .card.running h2 .icon { margin-left: 0; width: auto; padding: 0 5px; }
  .for.live { color: var(--running); }
  .run { display: flex; align-items: center; gap: 8px; padding: 5px 8px; border-radius: 8px; border: 1px solid transparent; margin-bottom: 4px; cursor: pointer; }
  .run:hover { background: var(--bg1); }
  .run.shown { background: var(--bg1); border-color: var(--bg3); }
  .run .st { width: 12px; text-align: center; font: 12px var(--mono); }
  .label { flex: 1; min-width: 0; font: 13px var(--mono); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .time { font: 11px var(--mono); color: var(--grey); }
  .stop { color: var(--red); padding: 2px 6px; border-radius: 6px; }
  .stop:hover { background: var(--bg2); }

  .toolkit { flex: 1; overflow: auto; }
  /* The Toolkit scrolls, so the character sits on a wrapper around it (it hangs over the edge) */
  .kit-wrap { position: relative; flex: 1; min-height: 0; display: flex; flex-direction: column; }
  .grid { display: grid; grid-template-columns: 1fr 1fr; gap: 6px; }
  .action-wrap { position: relative; }
  .action { width: 100%; display: flex; align-items: center; gap: 8px; text-align: left; padding: 8px 10px; border-radius: 8px; background: var(--bg1); border: 1px solid var(--bg2); font: 12.5px var(--mono); }
  .action.custom { border-color: color-mix(in srgb, var(--yellow) 30%, var(--bg2)); }
  .alabel { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  /* Pages: they show something (the Run buttons above do something) */
  .action.page { border-color: color-mix(in srgb, var(--blue) 25%, var(--bg2)); }
  .action.page.on { background: var(--bg2); border-color: color-mix(in srgb, var(--blue) 60%, var(--bg2)); }
  .action.page.add { border-style: dashed; background: none; color: var(--grey); }
  .action.page .tbadge, .action.page .pnum { margin-left: auto; }
  .pnum { font: 10.5px var(--mono); color: var(--grey-dim); }
  .action-tools { position: absolute; right: 4px; top: 50%; transform: translateY(-50%); display: flex; opacity: 0; background: var(--bg1); border-radius: 6px; }
  .action-wrap:hover .action-tools { opacity: 1; }
  .hidden-actions { margin-top: 12px; }
  .hidden-list { list-style: none; margin: 0; padding: 0 0 0 10px; font: 12px var(--mono); }
  .tab.open:hover { background: var(--bg2); }
  .hint-key { min-width: 16px; padding: 0 4px; border-radius: 4px; text-align: center; background: var(--orange); color: var(--bg0); font-weight: 700; }
  .hint-note { font: 400 11px var(--mono); color: var(--orange); }
  .help dl { display: grid; grid-template-columns: max-content 1fr; gap: 6px 16px; margin: 0; font-size: 13px; }
  .help dt { font-family: var(--mono); color: var(--orange); }
  .help dd { margin: 0; color: var(--fg); }
  .help dt.group { margin-top: 6px; color: var(--grey); font: 600 11px var(--mono); letter-spacing: 0.5px; }

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
  .action:hover { border-color: var(--bg3); background: var(--bg2); }
  .play { color: var(--orange); }
  .cmdline { position: fixed; left: 0; right: 0; bottom: 0; z-index: 70; height: 22px; display: flex; align-items: center; gap: 4px; padding: 0 10px;
             background: var(--bg-dim); border-top: 1px solid var(--bg2); font: 12px var(--mono); }
  .cmdkind { color: var(--orange); font-weight: 700; }
  .cmdline input { flex: 1; background: none; border: 0; outline: none; padding: 0; color: var(--fg); font: 12px var(--mono); box-shadow: none; }
  .whichkey { position: fixed; right: 14px; bottom: 30px; z-index: 65; min-width: 260px; max-width: min(560px, calc(100vw - 28px)); max-height: 60vh; overflow: auto;
              background: var(--bg0); border: 1px solid var(--bg3); border-radius: 10px; box-shadow: 0 12px 40px rgba(0, 0, 0, 0.45); padding: 10px 14px; font: 12px var(--mono); }
  .wk-title { color: var(--orange); font-weight: 700; margin-bottom: 6px; }
  .wk-grid { display: grid; grid-template-columns: max-content 1fr; gap: 3px 12px; }
  .wk-key { color: var(--yellow); }
  .wk-does { color: var(--fg); }
</style>
