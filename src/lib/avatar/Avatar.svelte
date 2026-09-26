<script lang="ts">
  // The character in the top bar: pixel art that shows what's going on (hover for why). Click
  // it, or the title (pickCharacter()), to pick another one, or none: then nothing shows.
  // Stops animating while the window is hidden.
  import type { Mood } from "./mood.ts";
  import { CHARACTERS, DEFAULT_CHARACTER, frame, HEIGHT, WIDTH, type Character } from "./sprite.ts";

  let { mood, caption, character, onPick }: { mood: Mood; caption: string; character: string; onPick: (id: string) => void } = $props();

  const SIGNS: Partial<Record<Mood, [string, string]>> = {
    failed: ["✗", "red"], done: ["✓", "green"], waiting: ["?", "orange"], thinking: ["…", "fg"], reading: ["≡", "aqua"],
    review: ["!", "yellow"], update: ["★", "yellow"], sleeping: ["z", "grey"], night: ["☾", "yellow"],
  };

  let tick = $state(1);
  $effect(() => {
    const timer = setInterval(() => {
      if (!document.hidden && !none) tick++;
    }, 200);
    return () => clearInterval(timer);
  });

  /** Runs of one colour in each row, as rectangles */
  function rects(c: Character, rows: string[]) {
    const out: { x: number; y: number; w: number; fill: string }[] = [];
    rows.forEach((row, y) => {
      for (let x = 0; x < WIDTH; ) {
        let end = x;
        while (end < WIDTH && row[end] === row[x]) end++;
        if (c.colors[row[x]]) out.push({ x, y, w: end - x, fill: c.colors[row[x]] });
        x = end;
      }
    });
    return out;
  }

  const none = $derived(character === "none");
  const id = $derived(CHARACTERS[character] ? character : DEFAULT_CHARACTER);
  const c = $derived(CHARACTERS[id]);
  const shown = $derived(none ? [] : rects(c, frame(mood, tick, c)));
  const sign = $derived(SIGNS[mood]);

  let menu = $state(false);
  let el = $state<HTMLElement | null>(null);

  /** Open the character menu (the title does, so it's there with no character shown) */
  export function pickCharacter() {
    menu = !menu;
  }
</script>

<svelte:window onpointerdown={(e) => menu && !el?.contains(e.target as Node) && (menu = false)} />

<span class="anchor" bind:this={el}>
  {#if !none}
    <button class="avatar {id} {mood}" title="{caption} (click: another character)" aria-label={caption} onclick={() => (menu = !menu)}>
      <svg viewBox="0 0 {WIDTH} {HEIGHT}" shape-rendering="crispEdges">
        {#each shown as r}<rect x={r.x} y={r.y} width={r.w} height="1" fill={r.fill} />{/each}
      </svg>
      {#if sign}<span class="sign" style="color: var(--{sign[1]})">{sign[0]}</span>{/if}
    </button>
  {/if}
  {#if menu}
    <div class="menu pick">
      {#each Object.entries(CHARACTERS) as [key, ch]}
        <button class:on={key === id} onclick={() => ((menu = false), onPick(key))}>
          <svg viewBox="0 0 {WIDTH} {HEIGHT}" shape-rendering="crispEdges">
            {#each rects(ch, frame("idle", 1, ch)) as r}<rect x={r.x} y={r.y} width={r.w} height="1" fill={r.fill} />{/each}
          </svg>
          {ch.name}
        </button>
      {/each}
      <button class:on={none} onclick={() => ((menu = false), onPick("none"))}>
        <span class="blank"></span>None <span class="dim">(click the title to pick again)</span>
      </button>
    </div>
  {/if}
</span>

<style>
  .anchor { position: relative; display: inline-flex; }
  .avatar { position: relative; display: inline-flex; align-items: flex-end; height: 42px; padding: 0; background: none; cursor: pointer; }
  /* 3 screen pixels per art pixel: whole numbers keep the edges sharp */
  .avatar svg { height: 39px; width: 48px; transform-origin: 50% 100%; }
  .sign { position: absolute; left: 100%; top: -2px; font: 700 15px var(--mono); margin-left: 2px; }
  .sleeping .sign { animation: float 2.4s ease-in-out infinite; }

  .avatar.idle svg, .avatar.review svg, .avatar.update svg { animation: bob 2.4s ease-in-out infinite; }
  .avatar.running svg { animation: bob 0.5s ease-in-out infinite; }
  .avatar.done svg { animation: jump 0.6s ease-out 3; }
  .avatar.failed svg { animation: shake 0.4s linear 2; }
  .avatar.thinking svg { animation: sway 3s ease-in-out infinite; }
  .avatar.sleeping svg, .avatar.night svg { animation: breathe 4s ease-in-out infinite; }
  /* Each character's own ways */
  .avatar.crab.running svg { animation: scuttle 0.5s steps(2) infinite; }
  .avatar.keycap.running svg { animation: press 0.4s ease-in-out infinite; }
  .avatar.ghost svg { animation: float-ghost 3s ease-in-out infinite; }
  .avatar.ghost.sleeping svg, .avatar.ghost.night svg { opacity: 0.45; }
  .avatar.ghost.failed svg { animation: flicker 0.3s steps(2) 4; }

  @keyframes bob { 50% { transform: translateY(-2px); } }
  @keyframes jump { 40% { transform: translateY(-8px); } }
  @keyframes shake { 25% { transform: translateX(-3px); } 75% { transform: translateX(3px); } }
  @keyframes sway { 25% { transform: rotate(-4deg); } 75% { transform: rotate(4deg); } }
  @keyframes breathe { 50% { transform: scaleY(0.94); } }
  @keyframes scuttle { 50% { transform: translateX(3px); } }
  @keyframes press { 50% { transform: translateY(3px) scaleY(0.9); } }
  @keyframes float-ghost { 50% { transform: translateY(-4px); } }
  @keyframes flicker { 50% { opacity: 0.2; } }
  @keyframes float { 0% { opacity: 0; transform: translateY(3px); } 50% { opacity: 1; } 100% { opacity: 0; transform: translateY(-8px); } }
  @media (prefers-reduced-motion: reduce) { .avatar svg, .sign { animation: none !important; } }

  .pick { position: absolute; top: 100%; left: 0; z-index: 20; display: grid; gap: 2px; padding: 6px; min-width: 150px;
          border-radius: 10px; background: var(--bg1); border: 1px solid var(--bg3); box-shadow: 0 8px 24px #0008; }
  .pick button { display: flex; align-items: center; gap: 10px; padding: 4px 8px; border-radius: 6px; font: 12.5px var(--mono); text-align: left; }
  .pick button:hover, .pick button.on { background: var(--bg2); }
  .pick svg, .pick .blank { flex: none; height: 26px; width: 32px; }
  .pick .dim { color: var(--grey); font-size: 11px; }
</style>
