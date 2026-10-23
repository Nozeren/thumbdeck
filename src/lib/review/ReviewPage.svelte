<script lang="ts">
  // The review page: a diff over the whole window. The changed files on the left (a tree; tick
  // them off as viewed), the shown file's changes in the middle with a line cursor, side by side
  // or in one column, syntax colours, the words that changed marked, and a map of where the
  // changes and comments are. For a pull request (when the plugin gives `actions`): comments
  // on a line or on selected lines (V), suggested changes, the threads under their lines (reply,
  // resolve), and the review sent as Comment, Approve or Request changes. Every button shows its
  // key. Keys come through handleKey() (the page sends them all here while it's open).
  import { tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import type { ReviewComment, ReviewEvent, ReviewRequest, ReviewThread } from "./types.ts";
  import { parseDiff, splitRows, wordDiff, type DiffFile, type Line, type Segment } from "./diff.ts";
  import { languageFor, lineHtml } from "./highlight.ts";
  import { anchorKey, commentable, commentRange, fileTree, findMatches, isChange, keptText, matchRanges, parseQuery, rangeLabel, renderBody, rowsFor, suggestionText, textLines, type Anchor, type VRow } from "./review.ts";
  import { keyName, KeyReader, times, type Fed, type Keymap, type Press } from "../keys/keys.ts";
  import { MOVED, REVIEW, REVIEW_COMMENT, REVIEW_FILES, REVIEW_POST, REVIEW_VISUAL } from "../keys/maps.ts";
  import KeyHelp from "../keys/KeyHelp.svelte";
  import { copyText, linesWord } from "../clipboard.ts";

  // Links go through thumbdeck, so a plugin that claimed them (Chrome) picks the profile
  const openUrl = (url: string) => invoke("open_url", { url }).catch(() => {});

  let { request, onClose, say = () => {} }: { request: ReviewRequest; onClose: () => void; say?: (text: string, error?: boolean) => void } = $props();

  const actions = $derived(request.actions);
  const canComment = $derived(!!actions?.submit);

  let files = $state<DiffFile[]>([]);
  let error = $state("");
  let loading = $state(true);
  let current = $state(0);
  let threads = $state<ReviewThread[]>([]);

  async function load() {
    loading = true;
    try {
      files = parseDiff(await request.load());
      error = "";
      const start = files.findIndex((f) => f.path === request.startFile);
      // Files that changed since you viewed them come first
      const again = files.filter(changedSince).length;
      if (start >= 0) current = start;
      else if (again) current = files.findIndex(changedSince);
      current = Math.min(current, Math.max(0, files.length - 1));
      texts = {};
      if (again) say(`${again} file${again === 1 ? "" : "s"} changed since you viewed ${again === 1 ? "it" : "them"} (marked ●)`);
    } catch (err) {
      error = String(err);
    } finally {
      loading = false;
    }
    loadThreads();
  }
  async function loadThreads() {
    if (!actions?.threads) return;
    try {
      threads = await actions.threads();
    } catch (err) {
      say(`Couldn't read the comments: ${err}`, true);
    }
  }
  $effect(() => {
    void request;
    load();
  });

  // ------------------------------------------------------------ remembered per viewer
  const store = {
    get(key: string) {
      try {
        return localStorage.getItem(key);
      } catch {
        return null;
      }
    },
    set(key: string, value: string) {
      try {
        localStorage.setItem(key, value);
      } catch {
        // Not remembered: fine
      }
    },
  };

  let split = $state(store.get("thumbdeck:review:mode") !== "unified");
  $effect(() => store.set("thumbdeck:review:mode", split ? "split" : "unified"));
  let aboutOpen = $state(store.get("thumbdeck:review:about") !== "closed");
  $effect(() => store.set("thumbdeck:review:about", aboutOpen ? "open" : "closed"));

  /** path -> the fingerprint of its changes when you marked it viewed */
  let viewed = $state<Record<string, string>>({});
  $effect(() => {
    try {
      viewed = JSON.parse(store.get(`thumbdeck:viewed:${request.viewedKey}`) ?? "{}");
    } catch {
      viewed = {};
    }
  });
  const isViewed = (f: DiffFile) => viewed[f.path] === f.hash;
  const viewedCount = $derived(files.filter(isViewed).length);
  /** Marked viewed, but its changes are different now (new commits, more edits) */
  const changedSince = (f: DiffFile) => !!viewed[f.path] && viewed[f.path] !== f.hash;
  const changedCount = $derived(files.filter(changedSince).length);

  function toggleViewed(i: number, next = true) {
    const f = files[i];
    if (!f) return;
    const v = { ...viewed };
    if (isViewed(f)) delete v[f.path];
    else v[f.path] = f.hash;
    viewed = v;
    store.set(`thumbdeck:viewed:${request.viewedKey}`, JSON.stringify(v));
    // Marked viewed: on to the next one still to look at
    if (next && isViewed(f)) {
      const after = files.findIndex((x, k) => k > i && !isViewed(x));
      const any = after >= 0 ? after : files.findIndex((x) => !isViewed(x));
      if (any >= 0) showFile(any);
      else say(canComment ? "Every file viewed: P posts your review" : "Every file viewed");
    }
  }

  // ------------------------------------------------------------ the shown file's rows
  const file = $derived<DiffFile | undefined>(files[current]);
  const lang = $derived(file ? languageFor(file.path) : null);

  /** Files' whole text (new version), read when a hidden part is opened */
  let texts = $state<Record<string, string[] | "reading" | "failed">>({});
  let whole = $state(false);
  let opened = $state<Record<string, true>>({});
  async function readText(path: string) {
    if (!request.file || texts[path]) return;
    texts[path] = "reading";
    try {
      texts[path] = textLines(await request.file(path));
    } catch (err) {
      texts[path] = "failed";
      say(`Couldn't read ${path}: ${err}`, true);
    }
  }
  const fileText = $derived(file && Array.isArray(texts[file.path]) ? (texts[file.path] as string[]) : null);
  // Read a file's text as soon as it's shown (cheap, and the hidden parts can then open)
  $effect(() => {
    if (file && request.file) readText(file.path);
  });

  const rows = $derived<VRow[]>(file ? rowsFor(file, split, fileText, (key) => whole || !!opened[`${file.path}#${key}`]) : []);

  /** Word changes of each removed line and the added line that replaced it */
  const words = $derived.by(() => {
    const map = new Map<Line, Segment[]>();
    if (!file) return map;
    for (const r of splitRows(file)) {
      if (r.left?.kind === "del" && r.right?.kind === "add") {
        const d = wordDiff(r.left.text, r.right.text);
        if (d) map.set(r.left, d.before), map.set(r.right, d.after);
      }
    }
    return map;
  });
  const html = (l: Line | null) => (l ? lineHtml(l.text, lang, words.get(l) ?? null, query && hitLines.has(l) ? matchRanges(l.text, query) : []) : "");

  // ------------------------------------------------------------ the cursor and the selection
  let cursor = $state(0);
  /** Where V was pressed (the selection runs from there to the cursor), or null */
  let anchorRow = $state<number | null>(null);
  const movable = (r: VRow | undefined) => !!r && r.kind !== "hunk";
  const selected = $derived.by(() => {
    if (anchorRow === null) return [] as number[];
    const [a, b] = [Math.min(anchorRow, cursor), Math.max(anchorRow, cursor)];
    return rows.map((_, i) => i).filter((i) => i >= a && i <= b && commentable(rows[i]));
  });

  // y: the selected lines (or the cursor's), as they read: the new side, or the old for a removed line
  async function yankRows() {
    const [a, b] = anchorRow !== null ? [Math.min(anchorRow, cursor), Math.max(anchorRow, cursor)] : [cursor, cursor];
    const picked = rows.slice(a, b + 1).filter((r): r is Extract<VRow, { kind: "line" }> => r?.kind === "line");
    const text = picked.map((r) => (r.right ?? r.left)?.text ?? "");
    if (!text.length) return say("No line here to copy");
    anchorRow = null;
    try {
      await copyText(text.join("\n"));
      say(`Copied ${linesWord(text.length)}`);
    } catch {
      say("Couldn't copy");
    }
  }

  let diffEl = $state<HTMLElement | null>(null);
  async function scrollToCursor() {
    await tick();
    document.getElementById(`rv-row-${cursor}`)?.scrollIntoView({ block: "nearest" });
  }
  function moveCursor(by: number) {
    const step = by > 0 ? 1 : -1;
    let i = cursor;
    for (let left = Math.abs(by); left > 0; left--) {
      let j = i + step;
      while (rows[j] && !movable(rows[j])) j += step;
      if (!rows[j]) break;
      i = j;
    }
    cursor = i;
    scrollToCursor();
  }
  function cursorTo(i: number) {
    cursor = Math.max(0, Math.min(rows.length - 1, i));
    if (!movable(rows[cursor])) moveCursor(1);
    scrollToCursor();
  }

  async function showFile(i: number, at: "first" | "change" | "last" = "first") {
    current = Math.max(0, Math.min(files.length - 1, i));
    anchorRow = null;
    box = null;
    await tick();
    diffEl?.scrollTo({ top: 0 });
    document.getElementById(`rv-file-${current}`)?.scrollIntoView({ block: "nearest" });
    if (at === "change") cursorTo(Math.max(0, rows.findIndex(isChange)));
    else if (at === "last") cursorTo(rows.length - 1);
    else cursorTo(Math.max(0, rows.findIndex((r) => r.kind === "line")));
  }

  /** The next (or previous) block of changes; past the last one, the next file */
  function jumpChange(dir: 1 | -1) {
    if (dir > 0) {
      let i = cursor;
      while (isChange(rows[i])) i++;
      const next = rows.findIndex((r, k) => k > i && isChange(r));
      if (next >= 0) return cursorTo(next);
      if (current < files.length - 1) showFile(current + 1, "change");
      return;
    }
    let i = cursor;
    while (i > 0 && isChange(rows[i - 1]) && isChange(rows[i])) i--;
    for (let k = i - 1; k >= 0; k--) if (isChange(rows[k]) && !isChange(rows[k - 1])) return cursorTo(k);
    if (current > 0) showFile(current - 1, "last");
  }

  // ------------------------------------------------------------ comments
  /** A comment waiting in your review (kept until you post it, even if the page closes) */
  interface Pending { id: number; path: string; start: Anchor; end: Anchor; label: string; body: string; original: string[] }
  let pending = $state<Pending[]>([]);
  $effect(() => {
    try {
      pending = JSON.parse(store.get(`thumbdeck:pending:${request.viewedKey}`) ?? "[]");
    } catch {
      pending = [];
    }
  });
  const savePending = () => store.set(`thumbdeck:pending:${request.viewedKey}`, JSON.stringify(pending));

  /** The box being written: a new comment on rows, or a reply to a thread */
  let box = $state<{ kind: "comment"; path: string; start: Anchor; end: Anchor; label: string; original: string[]; rows: number[] } | { kind: "reply"; thread: ReviewThread; row: number } | null>(null);
  let draft = $state("");
  let sending = $state(false);

  const threadsHere = $derived(file ? threads.filter((t) => t.path === file.path) : []);
  const threadsAt = $derived.by(() => {
    const m = new Map<string, ReviewThread[]>();
    for (const t of threadsHere) if (t.line !== null) m.set(`${t.side}:${t.line}`, [...(m.get(`${t.side}:${t.line}`) ?? []), t]);
    return m;
  });
  const outdated = $derived(threadsHere.filter((t) => t.line === null));
  const pendingAt = (r: VRow) => (r.kind === "line" && r.anchor && file ? pending.filter((p) => p.path === file.path && anchorKey(p.end) === anchorKey(r.anchor!)) : []);
  const threadAt = (r: VRow | undefined) => (r?.kind === "line" && r.anchor ? (threadsAt.get(anchorKey(r.anchor)) ?? [])[0] : undefined);
  let showResolved = $state<Record<string, true>>({});
  /** The text of the lines a thread sits on (what its suggestion would replace) */
  const threadLines = (t: ReviewThread) => {
    if (t.line === null) return [];
    const from = t.startLine ?? t.line;
    return rows.flatMap((r) => (r.kind === "line" && r.anchor && r.anchor.side === t.side && r.anchor.line >= from && r.anchor.line <= t.line! ? [(r.right ?? r.left)!.text] : []));
  };

  /** The rows a comment covers light up: the selection, the box's lines, the pending comment under the cursor */
  const lit = $derived.by(() => {
    const s = new Set<number>();
    if (anchorRow !== null) for (let i = Math.min(anchorRow, cursor); i <= Math.max(anchorRow, cursor); i++) if (rows[i]?.kind === "line") s.add(i);
    if (box?.kind === "comment") box.rows.forEach((i) => s.add(i));
    const r = rows[cursor];
    for (const p of r ? pendingAt(r) : []) rows.forEach((x, i) => x.kind === "line" && x.anchor && x.anchor.line >= p.start.line && x.anchor.line <= p.end.line && (x.anchor.side === p.end.side || x.anchor.side === p.start.side) && s.add(i));
    return s;
  });

  function startComment() {
    if (!canComment || !file) return say("This review is read-only");
    const list = anchorRow !== null ? selected : commentable(rows[cursor]) ? [cursor] : [];
    if (!list.length) return say(rows[cursor]?.kind === "line" ? "Only the changed parts can take a comment on GitHub" : "Pick a line first (j / k)");
    const first = rows[list[0]], last = rows[list[list.length - 1]];
    if (!commentable(first) || !commentable(last)) return;
    const lineRows = list.map((i) => rows[i]).filter(commentable);
    const anchors = lineRows.map((r) => r.anchor);
    box = { kind: "comment", path: file.path, start: first.anchor, end: last.anchor, label: rangeLabel(anchors), rows: list,
            original: keptText(lineRows) };
    draft = "";
    anchorRow = null;
    cursor = list[list.length - 1];
  }
  function startReply() {
    const t = threadAt(rows[cursor]);
    if (!t) return;
    if (!actions?.reply) return say("Replies can't be sent from here");
    box = { kind: "reply", thread: t, row: cursor };
    draft = "";
  }
  function suggest() {
    if (box?.kind !== "comment") return;
    const lineRows = box.rows.map((i) => rows[i]).filter((r): r is Extract<VRow, { kind: "line" }> => r?.kind === "line");
    draft = (draft.trim() ? draft.trimEnd() + "\n\n" : "") + suggestionText(lineRows);
  }
  async function send() {
    if (!box || !draft.trim()) return say("Write something first");
    if (box.kind === "comment") {
      pending = [...pending, { id: Date.now(), path: box.path, start: box.start, end: box.end, label: box.label, body: draft.trim(), original: box.original }];
      savePending();
      say(`Added to your review (${pending.length} waiting): P posts it`);
      box = null;
      draft = "";
      return;
    }
    sending = true;
    try {
      await actions!.reply!(box.thread.id, draft.trim());
      say("Reply sent");
      box = null;
      draft = "";
      await loadThreads();
    } catch (err) {
      say(`Couldn't send the reply: ${err}`, true);
    } finally {
      sending = false;
    }
  }
  async function resolve() {
    const t = threadAt(rows[cursor]);
    if (!t) return say("No thread on this line");
    if (t.resolved && !showResolved[t.id]) return void (showResolved[t.id] = true);
    if (t.resolved && showResolved[t.id] && !actions?.resolve) return void delete showResolved[t.id];
    if (!actions?.resolve) return say("Threads can't be resolved from here");
    try {
      await actions.resolve(t.id, !t.resolved);
      say(t.resolved ? "Thread open again" : "Thread resolved");
      delete showResolved[t.id];
      await loadThreads();
    } catch (err) {
      say(`Couldn't change the thread: ${err}`, true);
    }
  }
  function nextThread(dir: 1 | -1) {
    const has = (r: VRow) => r.kind === "line" && !!r.anchor && (threadsAt.has(anchorKey(r.anchor)) || pendingAt(r).length > 0);
    for (let i = cursor + dir; i >= 0 && i < rows.length; i += dir) if (has(rows[i])) return cursorTo(i);
    // On in the other files: the first file (in that direction) with a thread or a pending comment
    for (let k = 1; k < files.length; k++) {
      const fi = (current + dir * k + files.length) % files.length;
      const p = files[fi].path;
      if (threads.some((t) => t.path === p && t.line !== null) || pending.some((x) => x.path === p)) {
        showFile(fi).then(() => {
          const list = rows.map((r, i) => (has(r) ? i : -1)).filter((i) => i >= 0);
          if (list.length) cursorTo(dir > 0 ? list[0] : list[list.length - 1]);
        });
        return;
      }
    }
    say("No more threads");
  }

  // ------------------------------------------------------------ posting the review
  let post = $state<ReviewEvent | null>(null);
  let summary = $state("");
  let posting = $state(false);
  let verdict = $state<ReviewEvent | null>(null);
  const CHOICES: { id: ReviewEvent; name: string; does: string }[] = [
    { id: "COMMENT", name: "Comment", does: "Feedback, no verdict" },
    { id: "APPROVE", name: "Approve", does: "Good to merge" },
    { id: "REQUEST_CHANGES", name: "Request changes", does: "Needs another round before merging" },
  ];
  function openPost(e: ReviewEvent | null) {
    if (!canComment) return say("This review is read-only");
    post = e ?? (pending.length ? "COMMENT" : "APPROVE");
  }
  async function sendReview() {
    if (!post || !actions?.submit) return;
    if (post === "COMMENT" && !summary.trim() && !pending.length) return say("A comment review needs a summary or a line comment");
    if (post === "REQUEST_CHANGES" && !summary.trim()) return say("Say what to change in the summary (GitHub asks for one)");
    posting = true;
    const comments: ReviewComment[] = pending.map((p) => ({ path: p.path, ...commentRange(p.start, p.end), body: p.body }));
    try {
      await actions.submit({ event: post, body: summary.trim(), comments });
      say(`Review posted: ${post === "APPROVE" ? "approved" : post === "REQUEST_CHANGES" ? "changes requested" : "commented"}${comments.length ? `, with ${comments.length} comment${comments.length === 1 ? "" : "s"}` : ""}`);
      verdict = post;
      pending = [];
      savePending();
      summary = "";
      post = null;
      await loadThreads();
    } catch (err) {
      say(`Couldn't post the review: ${err}`, true);
    } finally {
      posting = false;
    }
  }

  // ------------------------------------------------------------ searching the changes (/, n, N)
  let searchText = $state("");
  let searching = $state(false);
  let searchEl = $state<HTMLInputElement | null>(null);
  const query = $derived(parseQuery(searchText));
  const matches = $derived(query ? findMatches(files, query) : []);
  const hitLines = $derived(new Set(matches.map((m) => m.line)));
  const hitsIn = $derived(matches.reduce((n, m) => n.set(m.file, (n.get(m.file) ?? 0) + 1), new Map<number, number>()));
  const isHit = (r: VRow | undefined) => !!r && r.kind === "line" && !r.extra && ((!!r.left && hitLines.has(r.left)) || (!!r.right && hitLines.has(r.right)));
  /** Which match the cursor is on ("3 of 17"), 0 when it's on none */
  const matchHere = $derived.by(() => {
    const r = rows[cursor];
    if (r?.kind !== "line") return 0;
    return matches.findIndex((m) => m.file === current && (m.line === r.left || m.line === r.right)) + 1;
  });

  async function openSearch() {
    searching = true;
    focus = "diff";
    await tick();
    searchEl?.select();
  }
  /** Enter in the box: on to the first match from here */
  function endSearch() {
    searching = false;
    searchEl?.blur();
    if (!query) return;
    if (!matches.length) return say(`Not found: ${searchText}`);
    if (!isHit(rows[cursor])) jumpMatch(1);
  }
  function clearSearch() {
    searching = false;
    searchText = "";
    searchEl?.blur();
  }
  /** The next (or previous) match: in this file past the cursor, then in the next files, round again at the end */
  function jumpMatch(dir: 1 | -1) {
    if (!query) return say("No search yet: / searches the changes");
    if (!matches.length) return say(`Not found: ${searchText}`);
    const here = rows.flatMap((r, i) => (isHit(r) ? [i] : []));
    const next = dir > 0 ? here.find((i) => i > cursor) : here.findLast((i) => i < cursor);
    if (next !== undefined) return cursorTo(next);
    for (let k = 1; k <= files.length; k++) {
      const fi = (current + dir * k + files.length * 2) % files.length;
      if (!hitsIn.get(fi)) continue;
      if (dir > 0 ? fi <= current : fi >= current) say(dir > 0 ? "Past the last match: back to the first" : "Past the first match: on to the last");
      showFile(fi).then(() => {
        const list = rows.flatMap((r, i) => (isHit(r) ? [i] : []));
        if (list.length) cursorTo(dir > 0 ? list[0] : list[list.length - 1]);
      });
      return;
    }
  }

  // ------------------------------------------------------------ the file list (after -)
  let focus = $state<"diff" | "files">("diff");
  let fileCursor = $state(0);
  let filter = $state("");
  let filtering = $state(false);
  let filterEl = $state<HTMLInputElement | null>(null);
  const tree = $derived(fileTree(files, filter));
  const order = $derived(tree.flatMap((g) => g.files));

  function toFiles() {
    focus = "files";
    fileCursor = current;
  }
  /** Moving in the list shows each file as you go */
  function moveFile(to: number) {
    if (!order.length) return;
    const at = Math.max(0, order.indexOf(fileCursor));
    fileCursor = order[Math.max(0, Math.min(order.length - 1, to === Infinity ? order.length - 1 : to === -Infinity ? 0 : at + to))];
    showFile(fileCursor);
  }
  function filesKey(p: Press) {
    switch (p.action) {
      case "down": moveFile(times(p)); break;
      case "up": moveFile(-times(p)); break;
      case "first": moveFile(-Infinity); break;
      case "last": moveFile(Infinity); break;
      case "open": focus = "diff"; showFile(fileCursor, "change"); break;
      case "tick": toggleViewed(fileCursor, false); break;
      case "search": openFilter(); break;
      case "help": help = true; break;
      case "leave": focus = "diff"; break;
    }
  }
  async function openFilter() {
    filtering = true;
    focus = "files";
    await tick();
    filterEl?.focus();
  }

  // ------------------------------------------------------------ keys
  let help = $state(false);

  export const keymap = (): Keymap =>
    post ? REVIEW_POST : box ? REVIEW_COMMENT : focus === "files" ? REVIEW_FILES : anchorRow !== null ? REVIEW_VISUAL : REVIEW;

  // The keys are read the vim way (sequences like ]c, counts like 5j) against the keymap of the
  // moment: the changes, a selection (VISUAL), the file list, a comment box, the post dialog
  const reader = new KeyReader();
  let pendingKeys = $state("");

  export function handleKey(e: KeyboardEvent): boolean {
    if (help) {
      help = false; // any key closes the help
      return true;
    }
    const inText = e.target instanceof HTMLElement && !!e.target.closest("textarea, input");
    const name = keyName(e);
    if (post) {
      if (name === "Escape") post = null;
      else if (name === "Ctrl+Enter" || name === "Ctrl+s") sendReview();
      else if (inText) return false;
      else if (["1", "2", "3"].includes(name)) post = CHOICES[Number(name) - 1].id;
      else if (name === "j" || name === "ArrowDown" || name === "k" || name === "ArrowUp") {
        const i = CHOICES.findIndex((c) => c.id === post);
        post = CHOICES[(i + (name === "j" || name === "ArrowDown" ? 1 : 2)) % 3].id;
      } else return false;
      return true;
    }
    if (box) {
      if (name === "Escape") (box = null), (draft = "");
      else if (name === "Ctrl+Enter" || name === "Ctrl+s") send();
      else if (name === "Ctrl+g") suggest();
      else return false;
      return true;
    }
    if (searching && inText) {
      if (name === "Enter") endSearch();
      else if (name === "Escape") clearSearch();
      else return false;
      return true;
    }
    if (filtering && inText) {
      if (name === "Enter") (filtering = false), filterEl?.blur();
      else if (name === "Escape") (filtering = false), (filter = ""), filterEl?.blur();
      else return false;
      return true;
    }
    if (inText || e.metaKey) return false;
    const map = keymap();
    const fed = reader.feed(map, name);
    pendingKeys = fed.kind === "wait" ? fed.pending : "";
    if (fed.kind === "wait") return true;
    if (fed.kind === "none") {
      const moved = fed.cancelled ? MOVED[fed.cancelled[0]] : MOVED[name];
      if (moved && !["]", "["].includes(name)) say(moved);
      return !!fed.cancelled || !!moved;
    }
    let f: Fed | undefined = fed;
    while (f?.kind === "action") {
      if (focus === "files") filesKey(f.press);
      else runKey(f.press);
      f = f.then;
    }
    return true;
  }

  function runKey(p: Press) {
    const page = 15;
    const n = times(p);
    const r = rows[cursor];
    switch (p.action) {
      case "down": moveCursor(n); break;
      case "up": moveCursor(-n); break;
      case "page-down": moveCursor(page * n); break;
      case "page-up": moveCursor(-page * n); break;
      case "full-down": moveCursor(2 * page * n); break;
      case "full-up": moveCursor(-2 * page * n); break;
      case "first": cursorTo(0); break;
      case "last": cursorTo(p.count ? Math.min(rows.length - 1, p.count - 1) : rows.length - 1); break;
      case "next-change": case "previous-change": for (let i = 0; i < n; i++) jumpChange(p.action === "next-change" ? 1 : -1); break;
      case "next-thread": case "previous-thread": for (let i = 0; i < n; i++) nextThread(p.action === "next-thread" ? 1 : -1); break;
      case "next-file": showFile(current + n); break;
      case "previous-file": showFile(current - n); break;
      case "change": case "comment": startComment(); break;
      case "yank": yankRows(); break;
      case "visual": case "visual-line":
        if (anchorRow !== null) anchorRow = null;
        else if (r?.kind === "line") (anchorRow = cursor), say(`Selecting: j / k extend, y copies${canComment ? ", c comments" : ""}, Esc drops it`);
        else say("Pick a line first");
        break;
      case "press":
        if (r?.kind === "gap") openGap(r.key);
        else if (threadAt(r)?.resolved && !showResolved[threadAt(r)!.id]) showResolved[threadAt(r)!.id] = true;
        else if (threadAt(r)) startReply();
        else startComment();
        break;
      case "fold-open":
        if (r?.kind === "gap") openGap(r.key);
        else say("Nothing folded here: zR shows the whole file");
        break;
      case "tick":
        // on a thread: resolve it; elsewhere: the file is viewed
        if (threadAt(r)) resolve();
        else toggleViewed(current);
        break;
      case "split": keepCursor(() => (split = !split)); break;
      case "unfold-all": case "fold-all":
        if (!request.file) say("The whole file isn't available here");
        else keepCursor(() => (whole = p.action === "unfold-all")), say(whole ? "The whole file (zM: only the changes)" : "Only the changes (zR: the whole file)");
        break;
      case "info": aboutOpen = !aboutOpen; break;
      case "approve": openPost("APPROVE"); break;
      case "request-changes": openPost("REQUEST_CHANGES"); break;
      case "post": openPost(null); break;
      case "files": toFiles(); break;
      case "search": openSearch(); break;
      case "next-match": for (let i = 0; i < n; i++) jumpMatch(1); break;
      case "previous-match": for (let i = 0; i < n; i++) jumpMatch(-1); break;
      case "outside": if (request.about?.url) openUrl(request.about.url); else say("No page to open"); break;
      case "refresh": load(); break;
      case "help": help = true; break;
      case "leave":
        if (anchorRow !== null) anchorRow = null;
        else if (searchText) clearSearch();
        else onClose();
        break;
    }
  }

  /** Keys typed so far, for the status bar */
  export const typedKeys = () => pendingKeys;

  /** The : line on the review page: :w posts, :approve, :request, :q, :e */
  export function command(text: string): boolean {
    switch (text) {
      case "w": case "post": runKey({ action: "post", key: ":w", count: null }); return true;
      case "approve": runKey({ action: "approve", key: ":approve", count: null }); return true;
      case "request": case "request-changes": runKey({ action: "request-changes", key: ":request", count: null }); return true;
      case "q": case "quit": onClose(); return true;
      case "e": case "refresh": load(); return true;
      case "wide": case "split": runKey({ action: "split", key: ":split", count: null }); return true;
    }
    return false;
  }

  function openGap(key: string) {
    if (!file) return;
    if (!request.file) return say("The hidden lines aren't available here");
    if (!fileText) return say("Reading the file…");
    opened[`${file.path}#${key}`] = true;
  }
  /** Change how rows are drawn, the cursor staying on the same line */
  async function keepCursor(change: () => void) {
    const r = rows[cursor];
    const at = r?.kind === "line" ? (r.right ?? r.left) : null;
    anchorRow = null;
    change();
    await tick();
    if (at) {
      const i = rows.findIndex((x) => x.kind === "line" && (x.right === at || x.left === at || ((x.right ?? x.left)?.new === at.new && at.new !== null)));
      if (i >= 0) return cursorTo(i);
    }
    cursorTo(cursor);
  }
  function clickRow(i: number, e: MouseEvent) {
    if (e.shiftKey && canComment && commentable(rows[i])) {
      if (anchorRow === null) anchorRow = commentable(rows[cursor]) ? cursor : i;
    } else anchorRow = null;
    focus = "diff";
    cursor = i;
  }

  // ------------------------------------------------------------ drawn pieces
  const totals = $derived(files.reduce((t, f) => ({ add: t.add + f.additions, del: t.del + f.deletions }), { add: 0, del: 0 }));
  const statusLetter = { modified: "M", added: "A", deleted: "D", renamed: "R" } as const;
  const dirOf = (p: string) => (p.includes("/") ? p.slice(0, p.lastIndexOf("/") + 1) : "");
  const nameOf = (p: string) => p.slice(p.lastIndexOf("/") + 1);
  const openThreads = (p: string) => threads.filter((t) => t.path === p && !t.resolved).length;
  const pendingIn = (p: string) => pending.filter((x) => x.path === p).length;
  const initial = (who: string) => (who.trim()[0] ?? "?").toUpperCase();
  const COLORS = ["purple", "blue", "orange", "aqua", "yellow", "green", "red"];
  const colorOf = (who: string) => `var(--${COLORS[[...who].reduce((h, c) => (h * 31 + c.charCodeAt(0)) >>> 0, 7) % COLORS.length]})`;
  const ago = (iso?: string) => {
    if (!iso) return "";
    const m = Math.round((Date.now() - Date.parse(iso)) / 60000);
    return m < 1 ? "now" : m < 60 ? `${m}m` : m < 1440 ? `${Math.round(m / 60)}h` : `${Math.round(m / 1440)}d`;
  };
  const checksSummary = $derived.by(() => {
    const c = request.about?.checks ?? [];
    if (!c.length) return null;
    if (c.some((x) => x.state === "failure")) return { text: `✗ ${c.filter((x) => x.state === "failure").length} failing`, cls: "red" };
    if (c.some((x) => x.state === "pending")) return { text: "● checks running", cls: "blue" };
    return { text: "✓ checks", cls: "green" };
  });
  /** The map on the right: where changes, threads and your comments are */
  const marks = $derived.by(() => {
    const n = Math.max(1, rows.length);
    return rows.flatMap((r, i) => {
      if (r.kind !== "line") return [];
      const out: { top: number; cls: string }[] = [];
      if (r.right?.kind === "add") out.push({ top: (i / n) * 100, cls: "add" });
      else if (r.left?.kind === "del") out.push({ top: (i / n) * 100, cls: "del" });
      if (r.anchor && threadsAt.has(anchorKey(r.anchor))) out.push({ top: (i / n) * 100, cls: "thread" });
      if (pendingAt(r).length) out.push({ top: (i / n) * 100, cls: "pending" });
      if (isHit(r)) out.push({ top: (i / n) * 100, cls: "hit" });
      return out;
    });
  });
  function clickMap(e: MouseEvent) {
    const el = e.currentTarget as HTMLElement;
    const r = el.getBoundingClientRect();
    cursorTo(Math.floor(((e.clientY - r.top) / r.height) * rows.length));
  }
  function focusOnMount(el: HTMLTextAreaElement) {
    el.focus();
    el.scrollIntoView({ block: "nearest" });
  }
  const verdictText = { APPROVE: ["✓ You approved", "green"], REQUEST_CHANGES: ["✗ You asked for changes", "red"], COMMENT: ["💬 You commented", "blue"] } as const;
</script>

{#snippet kbd(k: string)}<kbd>{k}</kbd>{/snippet}

{#snippet comments(t: ReviewThread, isCur: boolean)}
  <div class="thread" class:cur={isCur}>
    {#each t.comments as c}
      <div class="cm">
        <span class="av" style="background: {colorOf(c.author)}">{initial(c.author)}</span>
        <div class="cm-body"><div class="who">{c.author}<span class="when">{ago(c.createdAt)}</span></div>{@html renderBody(c.body, threadLines(t))}</div>
      </div>
    {/each}
    <div class="tfoot">
      {#if actions?.reply}<button onclick={() => ((cursor = rows.findIndex((r) => threadAt(r)?.id === t.id)), startReply())}>↩ Reply {@render kbd("Enter")}</button>{/if}
      {#if actions?.resolve}<button onclick={() => ((cursor = rows.findIndex((r) => threadAt(r)?.id === t.id)), resolve())}>{t.resolved ? "Open again" : "✓ Resolve"} {@render kbd("x")}</button>{/if}
      <span class="spacer"></span><span>{t.resolved ? "resolved" : "open"}</span>
    </div>
  </div>
{/snippet}

<div class="review" role="dialog" aria-label="Review: {request.title}">
  <header class="top">
    <button class="back" title="Back (q)" onclick={onClose}>‹ back {@render kbd("q")}</button>
    <div class="what">
      <strong>{request.title}</strong>
      <span class="dim line2">
        {#if request.about?.author}<span>{request.about.author}</span>{/if}
        {#if request.about?.head}<span class="branch">{request.about.head}</span> → <span class="branch">{request.about.base}</span>{/if}
        {#if request.subtitle && !request.about}<span>{request.subtitle}</span>{/if}
        <span class="stat"><span class="plus">+{totals.add}</span> <span class="minus">−{totals.del}</span></span>
        {#if checksSummary}<span class="pill {checksSummary.cls}">{checksSummary.text}</span>{/if}
        {#if verdict}<span class="verdict {verdictText[verdict][1]}">{verdictText[verdict][0]}</span>{/if}
      </span>
    </div>
    <span class="spacer"></span>
    {#if request.about}<button class="btn" class:on={aboutOpen} title="About the pull request (Space i)" onclick={() => (aboutOpen = !aboutOpen)}>About {@render kbd("Space i")}</button>{/if}
    {#if canComment}
      <button class="btn" title="Comment on the line (cc), or the selected lines (v, then c)" onclick={startComment}>Comment {@render kbd("cc")}</button>
      <button class="btn changes" title="Finish your review asking for changes (Space R, :request)" onclick={() => openPost("REQUEST_CHANGES")}>Request changes {@render kbd("Space R")}</button>
      <button class="btn approve" title="Finish your review approving it (Space A, :approve)" onclick={() => openPost("APPROVE")}>Approve {@render kbd("Space A")}</button>
      <button class="btn post" title="Post your review (Space P, :w)" onclick={() => openPost(null)}>Post review{#if pending.length}<span class="count">{pending.length}</span>{/if} {@render kbd(":w")}</button>
    {:else}
      <span class="dim">read-only</span>
    {/if}
    <button class="btn" title="Keys (?)" onclick={() => (help = true)}>{@render kbd("?")}</button>
  </header>

  <div class="body" class:with-about={aboutOpen && !!request.about}>
    <nav class="files" class:focused={focus === "files"}>
      <div class="files-head">
        <div class="row"><strong>Files</strong><span class="dim">{viewedCount} / {files.length} viewed</span><span class="spacer"></span><button title="Move in the files (-)" onclick={toFiles}>{@render kbd("-")}</button></div>
        <div class="progress"><span style="width: {files.length ? (viewedCount / files.length) * 100 : 0}%"></span></div>
        {#if changedCount}<div class="changed-note" title="You marked them viewed, and their changes are different now">● {changedCount} changed since you viewed {changedCount === 1 ? "it" : "them"}</div>{/if}
        <label class="filter">⌕
          <input bind:this={filterEl} bind:value={filter} placeholder="Filter files" onfocus={() => (filtering = true)} onblur={() => (filtering = false)} />
          {@render kbd("- /")}
        </label>
      </div>
      <div class="list">
        {#each tree as g (g.dir)}
          {#if g.dir}<div class="dir-row">▾ {g.dir}/</div>{/if}
          {#each g.files as i (i)}
            {@const f = files[i]}
            <button id="rv-file-{i}" class="file" class:nested={!!g.dir} class:on={i === current} class:cursor={focus === "files" && i === fileCursor}
                    class:seen={isViewed(f)} title={f.path} onclick={() => ((focus = "diff"), showFile(i))}>
              <span class="st {f.status}">{statusLetter[f.status]}</span>
              <span class="name">{nameOf(f.path)}</span>
              {#if pendingIn(f.path)}<span class="bubble pending" title="your comments waiting">{pendingIn(f.path)}</span>
              {:else if openThreads(f.path)}<span class="bubble" title="open threads">{openThreads(f.path)}</span>{/if}
              {#if hitsIn.get(i)}<span class="bubble hits" title="matches of the search">{hitsIn.get(i)}</span>{/if}
              <span class="counts"><span class="plus">{f.additions ? `+${f.additions}` : ""}</span> <span class="minus">{f.deletions ? `−${f.deletions}` : ""}</span></span>
              <span class="tick" class:changed={changedSince(f)} role="checkbox" aria-checked={isViewed(f)} tabindex="-1"
                    title={changedSince(f) ? "Changed since you viewed it: x marks it viewed again" : "Viewed (x)"}
                    onclick={(e) => (e.stopPropagation(), toggleViewed(i, i === current))} onkeydown={() => {}}>{isViewed(f) ? "✓" : changedSince(f) ? "●" : ""}</span>
            </button>
          {/each}
        {/each}
        {#if !tree.length && files.length}<p class="dim msg">No file matches.</p>{/if}
      </div>
    </nav>

    <section class="diff">
      {#if loading && !files.length}
        <p class="dim msg">Reading the changes…</p>
      {:else if error}
        <p class="msg err">{error}</p>
      {:else if !file}
        <p class="dim msg">No changes.</p>
      {:else}
        <header class="file-head">
          <span class="st {file.status}">{statusLetter[file.status]}</span>
          <span class="path" title={file.path}><span class="dir">{dirOf(file.path) ? `\u200e${dirOf(file.path)}\u200e` : ""}</span><strong>{nameOf(file.path)}</strong></span>
          {#if file.oldPath}<span class="dim">← {file.oldPath}</span>{/if}
          <span class="stat"><span class="plus">+{file.additions}</span> <span class="minus">−{file.deletions}</span></span>
          <span class="spacer"></span>
          <span class="dim">{current + 1} of {files.length}</span>
          <button title="Previous file ([f)" onclick={() => showFile(current - 1)}>{@render kbd("[f")}</button>
          <button title="Next file (]f)" onclick={() => showFile(current + 1)}>{@render kbd("]f")}</button>
          {#if request.file}<button class="toggle" class:on={whole} title="The whole file (zR) ↔ only the changes (zM)" onclick={() => keepCursor(() => (whole = !whole))}>{whole ? "Whole file" : "Changes only"} {@render kbd(whole ? "zM" : "zR")}</button>{/if}
          <button class="toggle" class:on={split} title="Side by side ↔ one column (Ctrl+w v)" onclick={() => keepCursor(() => (split = !split))}>{split ? "Side by side" : "One column"} {@render kbd("Ctrl+w v")}</button>
          <button class="toggle" class:on={isViewed(file)} class:changed={changedSince(file)} title={changedSince(file) ? "Changed since you viewed it: x marks it viewed again" : "Viewed: on to the next (x)"}
                  onclick={() => toggleViewed(current)}>{isViewed(file) ? "✓ Viewed" : changedSince(file) ? "● Changed since viewed" : "Viewed"} {@render kbd("x")}</button>
        </header>
        <label class="search" class:active={!!query}>⌕
          <input bind:this={searchEl} bind:value={searchText} placeholder="Search the changes (+word: added lines, -word: removed)"
                 onfocus={() => (searching = true)} onblur={() => (searching = false)} />
          {#if query}
            <span class="dim">{matches.length ? `${matchHere ? `${matchHere} of ` : ""}${matches.length} match${matches.length === 1 ? "" : "es"}` : "not found"}</span>
            <button title="The previous match (N)" onclick={() => jumpMatch(-1)}>{@render kbd("N")}</button>
            <button title="The next match (n)" onclick={() => jumpMatch(1)}>{@render kbd("n")}</button>
            <button title="Clear the search (Esc)" onclick={clearSearch}>{@render kbd("Esc")}</button>
          {:else}
            {@render kbd("/")}
          {/if}
        </label>
        <div class="diff-body">
          <div class="scroll" bind:this={diffEl}>
            {#if outdated.length}
              <div class="outdated">
                <div class="dim">Outdated threads (their lines changed since)</div>
                {#each outdated as t (t.id)}{@render comments(t, false)}{/each}
              </div>
            {/if}
            {#if file.binary}
              <p class="dim msg">A binary file: no text to compare.</p>
            {:else if !rows.length}
              <p class="dim msg">{file.status === "renamed" ? "Renamed, with no changes inside." : "No changes inside (e.g. only its mode changed)."}</p>
            {:else}
              <div class="rows" class:split>
                {#each rows as r, i}
                  {@const isCur = i === cursor && focus === "diff"}
                  {#if r.kind === "hunk"}
                    <div class="hunk"><span>{r.header.replace(/ @@.*$/, " @@")}</span>{#if r.fn}<span class="fn">{r.fn}</span>{/if}</div>
                  {:else if r.kind === "gap"}
                    <button id="rv-row-{i}" class="gap" class:cur={isCur} onclick={() => ((cursor = i), openGap(r.key))} disabled={!r.canOpen}>
                      ⋯ {r.count} unchanged line{r.count === 1 ? "" : "s"}{#if r.canOpen} · {@render kbd("zo")} show them · {@render kbd("zR")} the whole file{/if}
                    </button>
                  {:else}
                    {@const same = r.left === r.right}
                    <div id="rv-row-{i}" class="row" class:cur={isCur} class:lit={lit.has(i)} class:extra={r.extra} role="presentation" onclick={(e) => clickRow(i, e)}>
                      {#if split}
                        {@const lk = r.left ? (same ? "same" : r.left.kind) : "empty"}
                        {@const rk = r.right ? (same ? "same" : r.right.kind) : "empty"}
                        <span class="n {lk}">{r.left?.old ?? ""}</span><code class="{lk} l">{@html html(r.left)}</code>
                        <span class="n {rk}">{r.right?.new ?? ""}</span><code class={rk}>{@html html(r.right)}</code>
                      {:else}
                        {@const l = (r.left ?? r.right)!}
                        <span class="n {l.kind}">{l.old ?? ""}</span><span class="n {l.kind}">{l.new ?? ""}</span>
                        <code class={l.kind}><span class="sign">{l.kind === "add" ? "+" : l.kind === "del" ? "−" : " "}</span>{@html html(l)}</code>
                      {/if}
                      {#if canComment && commentable(r)}<button class="plus-btn" title="Comment (c)" onclick={(e) => (e.stopPropagation(), (cursor = i), startComment())}>+</button>{/if}
                    </div>
                    {#each r.anchor ? (threadsAt.get(anchorKey(r.anchor)) ?? []) : [] as t (t.id)}
                      {#if t.resolved && !showResolved[t.id]}
                        <button class="resolved" class:cur={isCur} onclick={() => (showResolved[t.id] = true)}>✓ Resolved: {t.comments.length} comment{t.comments.length === 1 ? "" : "s"} by {[...new Set(t.comments.map((c) => c.author))].join(", ")} · {@render kbd("x")} show</button>
                      {:else}
                        {@render comments(t, isCur)}
                      {/if}
                      {#if box?.kind === "reply" && box.thread.id === t.id}
                        <div class="box">
                          <div class="bh">Reply to the thread <span class="spacer"></span> sent at once</div>
                          <textarea bind:value={draft} use:focusOnMount placeholder="Write a reply… (Ctrl+Enter sends it)" disabled={sending}></textarea>
                          <div class="bf"><span class="spacer"></span><button class="btn" onclick={() => (box = null)}>Cancel {@render kbd("Esc")}</button><button class="btn post" onclick={send} disabled={sending}>{sending ? "Sending…" : "Reply"} {@render kbd("Ctrl+Enter")}</button></div>
                        </div>
                      {/if}
                    {/each}
                    {#each pendingAt(r) as p (p.id)}
                      <div class="thread mine" class:cur={isCur}>
                        <div class="cm">
                          <span class="av" style="background: var(--green)">Y</span>
                          <div class="cm-body"><div class="who">you<span class="badge pend">Pending</span>{#if p.label.startsWith("lines")}<span class="badge range">{p.label}</span>{/if}<span class="when">waits for Post review</span></div>{@html renderBody(p.body, p.original)}</div>
                        </div>
                        <div class="tfoot"><button onclick={() => ((pending = pending.filter((x) => x.id !== p.id)), savePending())}>Delete</button><span class="spacer"></span><span>sent with your review {@render kbd(":w")}</span></div>
                      </div>
                    {/each}
                    {#if box?.kind === "comment" && i === box.rows[box.rows.length - 1]}
                      <div class="box">
                        <div class="bh">Comment on {box.label} <span class="spacer"></span> Markdown</div>
                        <textarea bind:value={draft} use:focusOnMount placeholder="Write a comment… (Ctrl+Enter adds it to your review)"></textarea>
                        <div class="bf">
                          <button class="btn" title="Suggest a change to the lines (Ctrl+g)" onclick={suggest}>± Suggest a change {@render kbd("Ctrl+g")}</button>
                          <span class="spacer"></span>
                          <button class="btn" onclick={() => (box = null)}>Cancel {@render kbd("Esc")}</button>
                          <button class="btn post" onclick={send}>Add to review {@render kbd("Ctrl+Enter")}</button>
                        </div>
                      </div>
                    {/if}
                  {/if}
                {/each}
              </div>
            {/if}
          </div>
          <div class="map" role="presentation" title="Where the changes and comments are (click to go there)" onclick={clickMap}>
            {#each marks as m}<i class={m.cls} style="top: {m.top}%"></i>{/each}
            <i class="here" style="top: {(cursor / Math.max(1, rows.length)) * 100}%"></i>
          </div>
        </div>
      {/if}
    </section>

    {#if aboutOpen && request.about}
      {@const a = request.about}
      <aside class="about">
        {#if a.description}<section><h4>Description</h4><div class="desc">{@html renderBody(a.description)}</div></section>{/if}
        {#if a.checks?.length}
          <section><h4>Checks</h4>
            {#each a.checks as c}<div class="it"><span class={{ success: "green", failure: "red", pending: "blue", neutral: "dim" }[c.state]}>{{ success: "✓", failure: "✗", pending: "●", neutral: "○" }[c.state]}</span>{c.name}<span class="r dim">{c.detail ?? ""}</span></div>{/each}
          </section>
        {/if}
        {#if a.reviewers?.length}
          <section><h4>Reviewers</h4>
            {#each a.reviewers as rv}<div class="it"><span class="av small" style="background: {colorOf(rv.name)}">{initial(rv.name)}</span>{rv.name}<span class="r dim">{rv.state}</span></div>{/each}
          </section>
        {/if}
        {#if a.labels?.length}<section><h4>Labels</h4><div class="labels">{#each a.labels as l}<span class="label">{l}</span>{/each}</div></section>{/if}
        {#if canComment}
          <section><h4>Your review</h4>
            <div class="it">{pending.length} comment{pending.length === 1 ? "" : "s"} waiting<span class="r">{@render kbd(":w")}</span></div>
            <div class="it">{viewedCount} of {files.length} files viewed</div>
          </section>
        {/if}
        {#if a.url}<section><button class="btn" onclick={() => openUrl(a.url!)}>Open on GitHub {@render kbd("gx")}</button></section>{/if}
      </aside>
    {/if}
  </div>

  {#if post}
    <div class="scrim" role="presentation" onclick={() => (post = null)}></div>
    <div class="dialog" role="dialog" aria-label="Post your review">
      <div class="dh">Post your review <span class="dim">· {request.title}</span></div>
      <div class="db">
        <textarea bind:value={summary} placeholder="A summary (optional to approve; needed to request changes)" disabled={posting}></textarea>
        {#each CHOICES as c, i}
          <button class="choice {c.id}" class:on={post === c.id} onclick={() => (post = c.id)}>
            <span class="radio"></span><span class="cname"><b>{c.name}</b><span class="dim">{c.does}</span></span>{@render kbd(String(i + 1))}
          </button>
        {/each}
        <div class="dim">{pending.length ? `With your ${pending.length} line comment${pending.length === 1 ? "" : "s"} (${[...new Set(pending.map((p) => nameOf(p.path)))].join(", ")})` : "No line comments in this review."}</div>
      </div>
      <div class="df"><span class="dim">Sent with your GitHub login</span><span class="spacer"></span>
        <button class="btn" onclick={() => (post = null)}>Not yet {@render kbd("Esc")}</button>
        <button class="btn post" onclick={sendReview} disabled={posting}>{posting ? "Posting…" : "Post review"} {@render kbd("Ctrl+Enter")}</button>
      </div>
    </div>
  {/if}

  {#if help}<KeyHelp map={keymap()} note={canComment ? "" : "read-only"} />{/if}
</div>

<style>
  .review { position: fixed; inset: 0 0 22px 0; /* the status bar stays visible below */ z-index: 50; display: flex; flex-direction: column; background: var(--bg-dim); font: 12.5px var(--mono);
            --add-bg: color-mix(in srgb, var(--green) 12%, transparent); --del-bg: color-mix(in srgb, var(--red) 12%, transparent);
            --add-mark: color-mix(in srgb, var(--green) 36%, transparent); --del-mark: color-mix(in srgb, var(--red) 36%, transparent); }
  kbd { display: inline-block; min-width: 9px; padding: 0 4px; border: 1px solid var(--bg3); border-bottom-width: 2px; border-radius: 4px;
        font: 10.5px/1.45 var(--mono); color: var(--grey); text-align: center; vertical-align: 1px; background: var(--bg1); }
  button kbd { margin-left: 4px; }
  .spacer { flex: 1; }
  .dim { color: var(--grey); }
  .err, .red { color: var(--red); } .green { color: var(--green); } .blue { color: var(--blue); }
  .plus { color: var(--green); } .minus { color: var(--red); }

  /* the pull request */
  .top { display: flex; align-items: center; gap: 10px; padding: 9px 14px; background: var(--bg0); border-bottom: 1px solid var(--bg2); white-space: nowrap; }
  .what { display: flex; flex-direction: column; min-width: 0; flex: 1 1 auto; overflow: hidden; }
  .what strong { overflow: hidden; text-overflow: ellipsis; font-size: 14px; }
  .line2 { display: flex; gap: 10px; align-items: center; font-size: 11.5px; overflow: hidden; }
  .branch { color: var(--aqua); background: color-mix(in srgb, var(--aqua) 10%, transparent); border-radius: 4px; padding: 0 5px; }
  .pill { border: 1px solid currentColor; border-radius: 10px; padding: 0 8px; font-size: 11px; }
  .verdict { font-weight: 700; }
  .back { padding: 3px 8px; border-radius: 6px; color: var(--grey); }
  .back:hover { color: var(--fg); background: var(--bg1); }
  .btn { flex: none; display: inline-flex; align-items: center; padding: 4px 10px; border-radius: 7px; border: 1px solid var(--bg3); background: var(--bg1); white-space: nowrap; font: 12.5px var(--mono); }
  .btn:hover { border-color: var(--grey); }
  .btn.on { background: var(--bg2); border-color: var(--grey); }
  .btn.approve { color: var(--green); border-color: color-mix(in srgb, var(--green) 45%, var(--bg3)); }
  .btn.changes { color: var(--red); border-color: color-mix(in srgb, var(--red) 40%, var(--bg3)); }
  .btn.post { background: var(--green); color: var(--bg-dim); border-color: var(--green); font-weight: 700; }
  .btn.post kbd { background: color-mix(in srgb, var(--bg-dim) 20%, transparent); border-color: color-mix(in srgb, var(--bg-dim) 35%, transparent); color: var(--bg-dim); }
  .btn.post .count { background: var(--bg-dim); color: var(--green); border-radius: 8px; padding: 0 6px; margin-left: 6px; font-size: 11px; }
  .btn:disabled { opacity: 0.6; }

  .body { flex: 1; min-height: 0; display: grid; grid-template-columns: minmax(210px, 18%) 1fr; }
  .body.with-about { grid-template-columns: minmax(210px, 18%) 1fr minmax(240px, 22%); }

  /* the files */
  .files { display: flex; flex-direction: column; min-height: 0; background: var(--bg0); border-right: 1px solid var(--bg2); }
  .files.focused { box-shadow: inset 0 0 0 1px var(--orange); }
  .files-head { padding: 10px 12px 8px; display: flex; flex-direction: column; gap: 7px; border-bottom: 1px solid var(--bg2); }
  .files-head .row { display: flex; align-items: center; gap: 8px; }
  .progress { height: 5px; background: var(--bg2); border-radius: 3px; overflow: hidden; }
  .progress span { display: block; height: 100%; background: var(--green); border-radius: 3px; transition: width 0.2s; }
  .filter { display: flex; align-items: center; gap: 6px; border: 1px solid var(--bg3); border-radius: 6px; padding: 3px 8px; color: var(--grey); }
  .filter input { flex: 1; min-width: 0; background: none; border: none; outline: none; color: var(--fg); font: inherit; }
  .filter:focus-within { border-color: var(--blue); }
  .list { flex: 1; overflow: auto; padding: 6px; }
  .dir-row { color: var(--grey); padding: 5px 6px 2px; font-size: 11.5px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .file { width: 100%; display: flex; align-items: center; gap: 7px; padding: 3px 6px; border-radius: 6px; text-align: left; font: 12.5px var(--mono); }
  .file.nested { padding-left: 16px; }
  .file:hover { background: var(--bg1); }
  .file.on { background: var(--bg2); }
  .files.focused .file.cursor { box-shadow: inset 2px 0 var(--orange); }
  .file.seen .name { color: var(--grey); text-decoration: line-through; text-decoration-color: var(--bg3); }
  .name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .counts { flex: none; font-size: 10.5px; }
  .bubble { flex: none; font-size: 10px; color: var(--bg-dim); background: var(--yellow); border-radius: 7px; padding: 0 5px; }
  .bubble.pending { background: var(--orange); }
  .bubble.hits { background: var(--purple); }
  .tick { flex: none; width: 13px; height: 13px; border: 1px solid var(--bg3); border-radius: 3px; display: grid; place-items: center; font-size: 9px; color: var(--bg-dim); }
  .file.seen .tick { background: var(--green); border-color: var(--green); }
  .tick.changed { border-color: var(--orange); color: var(--orange); }
  .changed-note { color: var(--orange); font-size: 11.5px; }
  .st { flex: none; width: 12px; text-align: center; font-weight: 700; }
  .st.modified { color: var(--yellow); } .st.added { color: var(--green); } .st.deleted { color: var(--red); } .st.renamed { color: var(--purple); }

  /* the changes */
  .diff { min-width: 0; min-height: 0; display: flex; flex-direction: column; }
  .file-head { display: flex; align-items: center; gap: 9px; padding: 7px 12px; border-bottom: 1px solid var(--bg2); background: var(--bg0); white-space: nowrap; }
  .file-head .path { display: flex; min-width: 60px; flex: 0 1 auto; overflow: hidden; }
  .file-head .path .dir { color: var(--grey); overflow: hidden; text-overflow: ellipsis; min-width: 0; direction: rtl; }
  .file-head .path strong { flex: none; }
  .file-head .stat { font-size: 11px; }
  .toggle { display: inline-flex; align-items: center; padding: 2px 8px; border-radius: 6px; border: 1px solid var(--bg3); font: 12px var(--mono); }
  .toggle.on { color: var(--green); border-color: color-mix(in srgb, var(--green) 45%, var(--bg3)); }
  .toggle.changed { color: var(--orange); border-color: color-mix(in srgb, var(--orange) 45%, var(--bg3)); }
  /* The search: a line under the file's header */
  .search { display: flex; align-items: center; gap: 8px; padding: 4px 12px; border-bottom: 1px solid var(--bg2); color: var(--grey); white-space: nowrap; }
  .search input { flex: 1; min-width: 0; background: none; border: none; outline: none; color: var(--fg); font: inherit; }
  .search:focus-within, .search.active { background: var(--bg0); }
  .search:focus-within { box-shadow: inset 2px 0 var(--blue); }
  .diff-body { flex: 1; min-height: 0; display: flex; }
  .scroll { position: relative; flex: 1; min-height: 0; overflow: auto; }
  .msg { padding: 16px; }

  .rows { min-width: min-content; }
  .row { position: relative; display: grid; grid-template-columns: 46px 46px 1fr; min-height: 19px; }
  .rows.split .row { grid-template-columns: 46px minmax(0, 1fr) 46px minmax(0, 1fr); }
  .row.cur::before { content: ""; position: absolute; inset: 0; background: color-mix(in srgb, var(--yellow) 8%, transparent); box-shadow: inset 3px 0 var(--orange); pointer-events: none; z-index: 1; }
  .row.lit::after { content: ""; position: absolute; inset: 0; background: color-mix(in srgb, var(--blue) 13%, transparent); box-shadow: inset 3px 0 var(--blue); pointer-events: none; }
  .row.cur.lit::after { box-shadow: inset 3px 0 var(--orange); }
  .row.extra code, .row.extra .n { opacity: 0.85; }
  .n { padding: 0 8px; text-align: right; color: var(--grey-dim); font-size: 11.5px; user-select: none; line-height: 19px; }
  code { padding: 0 10px; white-space: pre-wrap; word-break: break-all; font: 12.5px var(--mono); line-height: 19px; tab-size: 4; }
  .rows.split code.l { border-right: 1px solid var(--bg1); }
  .add { background: var(--add-bg); } .del { background: var(--del-bg); }
  .n.add { color: var(--green); } .n.del { color: var(--red); }
  .empty { background: repeating-linear-gradient(135deg, transparent 0 6px, var(--bg1) 6px 7px); }
  .sign { display: inline-block; width: 14px; color: var(--grey); user-select: none; }
  code.add .sign { color: var(--green); } code.del .sign { color: var(--red); }
  code.add :global(mark) { background: var(--add-mark); color: inherit; border-radius: 2px; }
  .rows code :global(mark.hit) { background: color-mix(in srgb, var(--purple) 45%, transparent); color: var(--fg); border-radius: 2px; box-shadow: 0 0 0 1px color-mix(in srgb, var(--purple) 70%, transparent); }
  code.del :global(mark) { background: var(--del-mark); color: inherit; border-radius: 2px; }
  .plus-btn { position: absolute; left: 70px; top: 1px; width: 17px; height: 17px; border-radius: 4px; background: var(--blue); color: var(--bg-dim); font-weight: 700; display: none; place-items: center; z-index: 2; }
  .rows.split .plus-btn { left: calc(50% + 24px); }
  .row:hover .plus-btn { display: grid; }
  .hunk { display: flex; gap: 10px; padding: 3px 12px; margin-top: 2px; color: var(--blue); background: color-mix(in srgb, var(--blue) 8%, transparent); font-size: 11.5px; }
  .hunk .fn { color: var(--grey); }
  .gap { position: relative; display: flex; gap: 6px; align-items: center; width: 100%; padding: 3px 12px; color: var(--grey); background: var(--bg0); font: 11.5px var(--mono); border-block: 1px dashed var(--bg2); text-align: left; }
  .gap:not(:disabled):hover { color: var(--fg); }
  .gap.cur { box-shadow: inset 3px 0 var(--orange); color: var(--fg); }

  /* threads, your comments, the box */
  .thread, .box, .resolved { margin: 6px 14px 8px 100px; max-width: 780px; }
  .rows.split .thread, .rows.split .box, .rows.split .resolved { margin-left: calc(50% + 50px); max-width: 560px; }
  .outdated { padding: 8px 0 0 100px; }
  .outdated .thread { margin-left: 0; }
  .thread { border: 1px solid var(--bg3); border-radius: 10px; background: var(--bg0); font: 13px var(--sans); }
  .thread.cur { border-color: var(--orange); box-shadow: 0 0 0 1px var(--orange); }
  .cm { display: grid; grid-template-columns: 26px 1fr; gap: 10px; padding: 10px 12px; }
  .cm + .cm { border-top: 1px solid var(--bg2); }
  .av { width: 26px; height: 26px; border-radius: 50%; display: grid; place-items: center; font: 700 11px var(--mono); color: var(--bg-dim); }
  .av.small { width: 18px; height: 18px; font-size: 9px; }
  .who { font: 600 12px var(--mono); display: flex; gap: 8px; align-items: baseline; }
  .when { color: var(--grey); font-weight: 400; font-size: 11px; }
  .badge { font: 600 10px var(--mono); border-radius: 4px; padding: 0 5px; }
  .badge.pend { background: color-mix(in srgb, var(--orange) 18%, transparent); color: var(--orange); }
  .badge.range { background: color-mix(in srgb, var(--blue) 18%, transparent); color: var(--blue); }
  .cm-body { min-width: 0; color: var(--fg); line-height: 1.5; }
  .cm-body :global(p) { margin: 3px 0 0; }
  .cm-body :global(code), .desc :global(code) { font: 12px var(--mono); background: var(--bg2); padding: 0 4px; border-radius: 3px; }
  .cm-body :global(pre.code), .desc :global(pre.code) { font: 12px var(--mono); background: var(--bg1); padding: 6px 8px; border-radius: 6px; white-space: pre-wrap; }
  .cm-body :global(.suggestion) { margin-top: 6px; border: 1px solid var(--bg3); border-radius: 6px; overflow: hidden; font: 12px/1.5 var(--mono); }
  .cm-body :global(.sg-head) { background: var(--bg1); color: var(--grey); padding: 2px 8px; font-size: 11px; }
  .cm-body :global(.sg-old) { background: var(--del-bg); padding: 1px 8px; white-space: pre-wrap; }
  .cm-body :global(.sg-new) { background: var(--add-bg); padding: 1px 8px; white-space: pre-wrap; }
  .tfoot { display: flex; gap: 14px; align-items: center; padding: 6px 12px; border-top: 1px solid var(--bg2); color: var(--grey); font: 11.5px var(--mono); }
  .tfoot button:hover { color: var(--fg); }
  .resolved { display: flex; gap: 8px; align-items: center; color: var(--grey); font: 11.5px var(--mono); text-align: left; }
  .resolved.cur, .resolved:hover { color: var(--fg); }
  .box { border: 1px solid var(--blue); border-radius: 10px; background: var(--bg0); box-shadow: 0 0 0 1px color-mix(in srgb, var(--blue) 40%, transparent); }
  .bh { display: flex; gap: 8px; padding: 6px 12px; color: var(--grey); font-size: 11.5px; border-bottom: 1px solid var(--bg2); }
  .box textarea, .dialog textarea { width: 100%; min-height: 76px; resize: vertical; background: var(--bg-dim); color: var(--fg); border: none; outline: none; padding: 8px 12px; font: 13px/1.5 var(--sans); display: block; box-sizing: border-box; }
  .bf { display: flex; gap: 8px; align-items: center; padding: 6px 10px; border-top: 1px solid var(--bg2); }
  .bf .btn { padding: 2px 8px; }

  /* the map of changes and comments */
  .map { position: relative; width: 12px; flex: none; background: var(--bg0); border-left: 1px solid var(--bg2); cursor: pointer; }
  .map i { position: absolute; left: 3px; right: 3px; height: 3px; border-radius: 1px; }
  .map i.add { background: var(--green); } .map i.del { background: var(--red); }
  .map i.thread { background: var(--yellow); left: 0; right: 0; }
  .map i.pending { background: var(--orange); left: 0; right: 0; }
  .map i.hit { background: var(--purple); left: 0; right: 0; }
  .map i.here { left: 0; right: 0; height: 2px; background: var(--fg); opacity: 0.7; }

  /* about the pull request */
  .about { background: var(--bg0); border-left: 1px solid var(--bg2); overflow: auto; padding: 12px 14px; display: flex; flex-direction: column; gap: 14px; font-size: 12px; }
  .about h4 { margin: 0 0 6px; font: 600 11px var(--mono); color: var(--grey); letter-spacing: 0.06em; text-transform: uppercase; }
  .desc { font: 13px/1.55 var(--sans); }
  .desc :global(p) { margin: 0 0 8px; }
  .it { display: flex; gap: 8px; align-items: center; padding: 2px 0; white-space: nowrap; }
  .it .r { margin-left: auto; }
  .labels { display: flex; flex-wrap: wrap; gap: 4px; }
  .label { border-radius: 10px; padding: 0 8px; font-size: 11px; background: color-mix(in srgb, var(--blue) 18%, transparent); color: var(--blue); }

  /* posting the review */
  .scrim { position: absolute; inset: 0; background: color-mix(in srgb, var(--bg-dim) 60%, transparent); z-index: 10; }
  .dialog { position: absolute; z-index: 11; left: 50%; top: 70px; transform: translateX(-50%); width: min(560px, 90vw); background: var(--bg0); border: 1px solid var(--bg3); border-radius: 12px; box-shadow: 0 20px 60px #0009; }
  .dh { padding: 12px 16px; font: 600 14px var(--mono); border-bottom: 1px solid var(--bg2); }
  .db { padding: 12px 16px; display: flex; flex-direction: column; gap: 10px; }
  .dialog textarea { border: 1px solid var(--bg3); border-radius: 8px; }
  .dialog textarea:focus { border-color: var(--blue); }
  .choice { display: grid; grid-template-columns: 22px 1fr auto; gap: 10px; align-items: center; padding: 8px 10px; border: 1px solid var(--bg3); border-radius: 8px; text-align: left; }
  .choice:hover { border-color: var(--grey); }
  .choice .cname { display: flex; flex-direction: column; font-size: 12px; } .choice .cname .dim { font: 12px var(--sans); }
  .radio { width: 14px; height: 14px; border-radius: 50%; border: 2px solid var(--bg3); }
  .choice.COMMENT { color: var(--blue); } .choice.APPROVE { color: var(--green); } .choice.REQUEST_CHANGES { color: var(--red); }
  .choice.on { border-color: currentColor; } .choice.on .radio { border-color: currentColor; background: currentColor; box-shadow: inset 0 0 0 3px var(--bg0); }
  .df { display: flex; gap: 8px; align-items: center; padding: 10px 16px; border-top: 1px solid var(--bg2); font-size: 11.5px; }

  /* Syntax colours from the theme */
  code :global(.hljs-keyword), code :global(.hljs-selector-tag), code :global(.hljs-literal) { color: var(--red); }
  code :global(.hljs-string), code :global(.hljs-regexp), code :global(.hljs-addition) { color: var(--green); }
  code :global(.hljs-number), code :global(.hljs-symbol) { color: var(--purple); }
  code :global(.hljs-comment), code :global(.hljs-quote) { color: var(--grey); font-style: italic; }
  code :global(.hljs-title), code :global(.hljs-section), code :global(.hljs-name) { color: var(--green); font-weight: 600; }
  code :global(.hljs-built_in), code :global(.hljs-type), code :global(.hljs-class) { color: var(--yellow); }
  code :global(.hljs-attr), code :global(.hljs-attribute), code :global(.hljs-property), code :global(.hljs-variable) { color: var(--blue); }
  code :global(.hljs-meta), code :global(.hljs-tag), code :global(.hljs-params) { color: var(--aqua); }
  @media (prefers-reduced-motion: reduce) { .progress span { transition: none; } }
</style>
