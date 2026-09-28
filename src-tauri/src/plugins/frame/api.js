// window.thumbdeck: the page API every plugin frame gets before its own scripts run
// (docs/plugin-spec.md). Everything goes to thumbdeck through postMessage; thumbdeck attaches
// the plugin and project itself. Injected by src-tauri/src/plugins/frame.rs.
(() => {
  "use strict";
  const parentWindow = window.parent;
  const post = (message) => parentWindow.postMessage({ __td: 1, ...message }, "*");

  // The page's console and errors go to the plugin's log (Settings › Plugins)
  const show = (v) => (v instanceof Error ? `${v.message}${v.stack ? ` (at ${v.stack.split("\n")[0]})` : ""}` : typeof v === "string" ? v : (() => {
    try {
      return JSON.stringify(v);
    } catch {
      return String(v);
    }
  })());
  for (const level of ["log", "info", "warn", "error", "debug"]) {
    const original = console[level].bind(console);
    console[level] = (...args) => {
      original(...args);
      post({ kind: "log", level, text: args.map(show).join(" ") });
    };
  }
  window.addEventListener("error", (e) => post({ kind: "log", level: "error", text: `${e.message} (${e.filename}:${e.lineno})` }));
  window.addEventListener("unhandledrejection", (e) => post({ kind: "log", level: "error", text: `unhandled: ${show(e.reason)}` }));

  // Where am I: sent by thumbdeck in the page's address (?td=<base64 JSON>)
  const context = (() => {
    try {
      const raw = new URLSearchParams(location.search).get("td") || "";
      const bytes = Uint8Array.from(atob(raw.replace(/-/g, "+").replace(/_/g, "/")), (c) => c.charCodeAt(0));
      return JSON.parse(new TextDecoder().decode(bytes));
    } catch {
      return { plugin: {}, surface: "tab", id: "", project: null, api: 1, thumbdeck: "" };
    }
  })();
  Object.freeze(context);

  // ------------------------------------------------------------ calls and their answers
  let nextId = 1;
  const waiting = new Map(); // call id -> { resolve, reject }

  function call(method, params) {
    const id = nextId++;
    return new Promise((resolve, reject) => {
      waiting.set(id, { resolve, reject });
      post({ kind: "call", id, method, params: params ?? null });
    });
  }

  /** A call whose answer nobody waits for; its error still shows in the plugin's log */
  function tell(method, params) {
    call(method, params).catch((e) => console.error(`thumbdeck.${method}:`, e.message));
  }

  // Functions the plugin hands to thumbdeck (e.g. a review's diff loader), by id
  const callbacks = new Map();
  let nextCallback = 1;
  function callback(fn) {
    const id = nextCallback++;
    callbacks.set(id, fn);
    return { callback: id };
  }

  // ------------------------------------------------------------ events
  const listeners = new Map(); // name -> Set of functions
  function on(name, fn) {
    if (!listeners.has(name)) listeners.set(name, new Set());
    listeners.get(name).add(fn);
    return () => listeners.get(name)?.delete(fn);
  }
  function emit(name, data) {
    for (const fn of listeners.get(name) ?? []) {
      try {
        fn(data);
      } catch (e) {
        console.error(e);
      }
    }
  }

  const runs = new Map(); // run id -> { output: Set, exit: Set, done }
  const watches = new Map(); // watch id -> function

  window.addEventListener("message", async (e) => {
    const m = e.data;
    if (e.source !== parentWindow || !m || m.__td !== 1) return;
    if (m.kind === "reply") {
      const w = waiting.get(m.id);
      if (!w) return;
      waiting.delete(m.id);
      if ("error" in m) w.reject(new Error(m.error));
      else w.resolve(m.result);
    } else if (m.kind === "call") {
      // thumbdeck calling one of the plugin's functions
      const fn = callbacks.get(m.callback);
      try {
        if (!fn) throw new Error("that function is gone");
        post({ kind: "reply", id: m.id, result: await fn(...(m.args ?? [])) });
      } catch (err) {
        post({ kind: "reply", id: m.id, error: String(err?.message ?? err) });
      }
    } else if (m.kind === "event") {
      const d = m.data;
      if (m.name === "run.output") runs.get(d.id)?.output.forEach((f) => f(d.line, d.stderr));
      else if (m.name === "run.exit") {
        const r = runs.get(d.id);
        if (r) {
          r.exit.forEach((f) => f(d.code));
          r.finish(d.code);
          runs.delete(d.id);
        }
      } else if (m.name === "fs.watch") watches.get(d.id)?.(d.change);
      else if (m.name === "backend") emit(`backend:${d.name}`, d.data);
      else {
        if (m.name === "keyboard") document.body?.classList.toggle("td-keys", !!d);
        emit(m.name, d);
      }
    } else if (m.kind === "rawkey") {
      // A key that isn't one of the plugin's actions: to its own keydown listeners
      const ev = new KeyboardEvent("keydown", { ...m.key, bubbles: true, cancelable: true });
      Object.defineProperty(ev, "fromThumbdeck", { value: true });
      (document.activeElement ?? document.body ?? document).dispatchEvent(ev);
    }
  });

  // ------------------------------------------------------------ keys
  // Keys pressed in the frame go through thumbdeck too (its keys, the plugin's actions),
  // except while a field has focus: typing stays in the field, and Esc leaves it.
  window.addEventListener(
    "keydown",
    (e) => {
      if (e.fromThumbdeck) return;
      const t = e.target;
      if (t instanceof Element && t.closest("input, textarea, select, [contenteditable]")) {
        if (e.key === "Escape") {
          t.blur();
          e.preventDefault();
        }
        return;
      }
      e.preventDefault();
      e.stopImmediatePropagation();
      const { key, code, ctrlKey, metaKey, altKey, shiftKey } = e;
      post({ kind: "key", key: { key, code, ctrlKey, metaKey, altKey, shiftKey } });
    },
    true,
  );
  // A click in the frame asks for the keyboard
  window.addEventListener("pointerdown", () => post({ kind: "activate" }), true);

  // ------------------------------------------------------------ the API
  const escapes = { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" };

  window.thumbdeck = Object.freeze({
    context,

    projects: Object.freeze({
      selected: () => call("projects.selected"),
      list: () => call("projects.list"),
    }),

    setup: Object.freeze({
      get: () => call("setup.get"),
      edit: () => tell("setup.edit"),
      save: (value) => call("setup.save", { value }),
    }),

    settings: Object.freeze({ get: () => call("settings.get") }),

    storage: Object.freeze({
      get: async (key, o = {}) => (await call("storage.get", { key, scope: o.scope })) ?? undefined,
      set: (key, value, o = {}) => call("storage.set", { key, value, scope: o.scope }),
      remove: (key, o = {}) => call("storage.remove", { key, scope: o.scope }),
    }),

    fs: Object.freeze({
      read: (path, o = {}) => call("fs.read", { path, ...o }),
      write: (path, text) => call("fs.write", { path, text }),
      stat: (path) => call("fs.stat", { path }),
      list: (folder, o = {}) => call("fs.list", { folder, ...o }),
      watch(path, fn) {
        let id = null;
        let stopped = false;
        call("fs.watch", { path }).then((w) => {
          if (stopped) return void tell("fs.unwatch", { id: w });
          id = w;
          watches.set(id, fn);
        });
        return () => {
          stopped = true;
          if (id !== null) {
            watches.delete(id);
            tell("fs.unwatch", { id });
          }
        };
      },
    }),

    exec: (command, o = {}) => call("exec", { command, ...o }),

    async run(command, o = {}) {
      const id = await call("run", { command, label: o.label ?? command, cwd: o.cwd });
      const r = { output: new Set(), exit: new Set(), finish: () => {} };
      const done = new Promise((resolve) => (r.finish = resolve));
      runs.set(id, r);
      return Object.freeze({
        id,
        onOutput: (fn) => void r.output.add(fn),
        onExit: (fn) => void r.exit.add(fn),
        stop: () => tell("run.stop", { id }),
        done,
      });
    },

    tmux: (command, o = {}) => call("tmux", { command, window: o.window, show: !!o.show }),

    ui: Object.freeze({
      say: (text, o = {}) => tell("ui.say", { text: String(text), error: !!o.error }),
      notify: (title, body) => tell("ui.notify", { title: String(title), body: body == null ? "" : String(body) }),
      confirm: (question, o = {}) => call("ui.confirm", { question: String(question), yes: o.yes, no: o.no }),
      openUrl: (url) => tell("ui.openUrl", { url: String(url) }),
      openPage: (id, data) => tell("ui.openPage", { id, data: data ?? null }),
      close: () => tell("ui.close"),
      openReview(r) {
        const diff = typeof r.diff === "function" ? callback(r.diff) : String(r.diff ?? "");
        tell("ui.openReview", { title: r.title, subtitle: r.subtitle, viewedKey: r.viewedKey, startFile: r.startFile, diff });
      },
      badge: (value) => tell("ui.badge", { value: value == null ? null : String(value) }),
      status: (text) => tell("ui.status", { text: text == null ? null : String(text) }),
      mood: (signal, value) => tell("ui.mood", { signal, value: value ?? null }),
    }),

    actions: Object.freeze({ refresh: () => tell("actions.refresh") }),

    keys: Object.freeze({
      use: (map) => tell("keys.use", { map }),
      take: () => tell("keys.take"),
      release: () => tell("keys.release"),
    }),

    backend: Object.freeze({
      call: (method, params) => call("backend.call", { method, params: params ?? null }),
      on: (name, fn) => on(`backend:${name}`, fn),
    }),

    on,

    escape: (text) => String(text).replace(/[&<>"']/g, (c) => escapes[c]),
  });

  // The page's height, for a panel that's as tall as its content
  let lastHeight = 0;
  const measure = () => {
    const h = Math.ceil(document.documentElement.scrollHeight);
    if (h !== lastHeight) post({ kind: "height", px: (lastHeight = h) });
  };
  addEventListener("load", () => {
    measure();
    if (typeof ResizeObserver === "function") new ResizeObserver(measure).observe(document.body ?? document.documentElement);
  });

  post({ kind: "ready" });
})();
