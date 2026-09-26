<script lang="ts">
  // The line at the bottom of the window, like a status line in nvim: who has the keyboard,
  // the project and its branch, and thumbdeck's short messages.
  let {
    mode,
    project,
    branch,
    message,
  }: {
    /** Who has the keyboard: THUMBDECK, GIT, REVIEW… */
    mode: string;
    project: string | null;
    /** The project's branch against its upstream, and its uncommitted files (null: not a repo) */
    branch: { name: string | null; ahead: number; behind: number; gone: boolean; changed: number } | null;
    message: { text: string; error: boolean } | null;
  } = $props();
</script>

<footer class="status">
  <span class="mode">{mode}</span>
  {#if project}<span class="project">{project}</span>{/if}
  {#if branch}
    <span class="branch">{branch.name ?? "detached HEAD"}</span>
    {#if branch.gone}<span class="red">upstream gone</span>{/if}
    {#if branch.ahead}<span class="yellow">↑{branch.ahead}</span>{/if}
    {#if branch.behind}<span class="yellow">↓{branch.behind}</span>{/if}
    <span class="dim">{branch.changed ? `${branch.changed} changed` : "clean"}</span>
  {/if}
  <span class="spacer"></span>
  {#if message}<span class="message" class:red={message.error}>{message.text}</span>{/if}
</footer>

<style>
  .status { position: fixed; left: 0; right: 0; bottom: 0; z-index: 60; display: flex; align-items: center; gap: 10px; height: 22px;
            padding: 0 10px; overflow: hidden; white-space: nowrap; font: 11.5px var(--mono); color: var(--fg);
            border-top: 1px solid var(--bg2); background: var(--bg-dim); }
  .mode { padding: 1px 8px; border-radius: 4px; background: var(--orange); color: var(--bg-dim); font-weight: 700; letter-spacing: 0.5px; }
  .project { font-weight: 600; }
  .branch { color: var(--green); }
  .dim { color: var(--grey); }
  .yellow { color: var(--yellow); }
  .red { color: var(--red); }
  .spacer { flex: 1; }
  .message { min-width: 0; overflow: hidden; text-overflow: ellipsis; color: var(--green); }
  .message.red { color: var(--red); }
</style>
