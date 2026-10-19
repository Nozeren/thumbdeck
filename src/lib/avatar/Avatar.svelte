<script lang="ts">
  // The character, perched on the bottom edge of the Toolkit: small pixel art that shows what's
  // going on. When its mood changes a bubble says why for a few seconds (hover shows it too), a
  // soft glow takes the mood's colour, and clicking it goes to the cause (the run, the PRs…).
  // The title (pickCharacter()) picks another character, or none: then nothing shows. Stops
  // animating while the window is hidden.
  import type { Mood } from "./mood.ts";
  import { CHARACTERS, DEFAULT_CHARACTER, frame, HEIGHT, WIDTH, type Character } from "./sprite.ts";

  let { mood, caption, character, onPick, go = null, onGo = () => {} }: {
    mood: Mood; caption: string; character: string; onPick: (id: string) => void;
    /** Where a click takes you ("its output", "Pull requests"), or null: nowhere */
    go?: string | null;
    onGo?: () => void;
  } = $props();

  /** Screen pixels per art pixel: whole numbers keep the edges sharp */
  const PX = 2;

  const SIGNS: Partial<Record<Mood, [string, string]>> = {
    failed: ["✗", "red"], done: ["✓", "green"], waiting: ["?", "orange"], thinking: ["…", "fg"], reading: ["≡", "aqua"],
    review: ["!", "yellow"], update: ["★", "yellow"], night: ["☾", "yellow"],
  };
  const GLOWS: Partial<Record<Mood, string>> = {
    failed: "red", done: "green", running: "blue", waiting: "purple", thinking: "purple", reading: "aqua", review: "yellow", update: "yellow",
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
  const glow = $derived(GLOWS[mood]);
  /** How far its body hangs below the edge it sits on */
  const hang = $derived((HEIGHT - c.seat) * PX);

  // The bubble: for a few seconds when the mood changes (not on start, not for "all quiet")
  let bubble = $state(false);
  let lastMood: Mood | null = null;
  let bubbleTimer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const m = mood;
    if (lastMood !== null && m !== lastMood && m !== "idle") {
      bubble = true;
      clearTimeout(bubbleTimer);
      bubbleTimer = setTimeout(() => (bubble = false), 3500);
    }
    lastMood = m;
  });

  let menu = $state(false);
  let el = $state<HTMLElement | null>(null);

  /** Open the character menu (the title does, so it's there with no character shown) */
  export function pickCharacter() {
    menu = !menu;
  }
</script>

<svelte:window onpointerdown={(e) => menu && !el?.contains(e.target as Node) && (menu = false)} />

<span class="perch" bind:this={el} style="--px: {PX}; --hang: {hang}px; --glow: {glow ? `color-mix(in srgb, var(--${glow}) 55%, transparent)` : 'transparent'}">
  {#if !none}
    <button class="avatar {id} {mood}" title={go ? `${caption} (click: ${go})` : caption} aria-label={caption} onclick={onGo}>
      <span class="glow"></span>
      <svg viewBox="0 0 {WIDTH} {HEIGHT}" width={WIDTH * PX} height={HEIGHT * PX} shape-rendering="crispEdges">
        {#each shown as r}<rect x={r.x} y={r.y} width={r.w} height="1" fill={r.fill} />{/each}
      </svg>
      {#if sign}<span class="sign" style="color: var(--{sign[1]})">{sign[0]}</span>{/if}
      {#if mood === "sleeping"}<span class="zz"><span>z</span><span>z</span><span>z</span></span>{/if}
      <span class="bubble" class:show={bubble}>{caption}{#if go}<span class="go">{` · click: ${go}`}</span>{/if}</span>
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
  /* Sitting on the edge: the body above it, the lower rows hanging over (see --hang) */
  .perch { position: absolute; right: 14px; bottom: calc(1px - var(--hang)); z-index: 6; line-height: 0; }
  .avatar { position: relative; display: block; padding: 0; background: none; cursor: pointer; }
  .avatar svg { display: block; transform-origin: 50% 100%; }
  /* A soft shadow where it sits, and a glow in its mood's colour */
  .avatar::after { content: ""; position: absolute; left: 14%; right: 14%; bottom: calc(var(--hang) - 2px); height: 3px; border-radius: 50%;
                   background: #0007; filter: blur(1.5px); z-index: -1; }
  .glow { position: absolute; inset: -40% -30%; border-radius: 50%; background: radial-gradient(closest-side, var(--glow), transparent);
          opacity: 0.55; z-index: -2; pointer-events: none; transition: background 0.4s; }
  .sign { position: absolute; left: 100%; top: -4px; margin-left: 1px; font: 700 11px var(--mono); line-height: 1; }
  .zz { position: absolute; left: 80%; top: -6px; font: 700 9px var(--mono); color: var(--grey); line-height: 1; }
  .zz span { position: absolute; opacity: 0; animation: zz 3s ease-in-out infinite; }
  .zz span:nth-child(2) { animation-delay: 1s; } .zz span:nth-child(3) { animation-delay: 2s; }
  /* The reason, above it */
  .bubble { position: absolute; bottom: calc(100% + 8px); right: -6px; width: max-content; max-width: 220px; padding: 4px 8px; border-radius: 8px;
            background: var(--bg2); border: 1px solid var(--bg3); box-shadow: 0 6px 18px #0006; color: var(--fg); font: 11px/1.4 var(--mono);
            text-align: left; opacity: 0; transform: translateY(4px); transition: opacity 0.25s, transform 0.25s; pointer-events: none; }
  .bubble::after { content: ""; position: absolute; top: 100%; right: 14px; border: 5px solid transparent; border-top-color: var(--bg3); }
  .bubble .go { color: var(--grey); }
  .avatar:hover .bubble, .bubble.show { opacity: 1; transform: none; }

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

  @keyframes bob { 50% { transform: translateY(-1px); } }
  @keyframes jump { 40% { transform: translateY(-6px); } }
  @keyframes shake { 25% { transform: translateX(-2px); } 75% { transform: translateX(2px); } }
  @keyframes sway { 25% { transform: rotate(-5deg); } 75% { transform: rotate(5deg); } }
  @keyframes breathe { 50% { transform: scaleY(0.93); } }
  @keyframes scuttle { 50% { transform: translateX(2px); } }
  @keyframes press { 50% { transform: translateY(2px) scaleY(0.9); } }
  @keyframes float-ghost { 50% { transform: translateY(-2px); } }
  @keyframes flicker { 50% { opacity: 0.2; } }
  @keyframes zz { 0% { opacity: 0; transform: translate(0, 4px) scale(0.7); } 30% { opacity: 1; } 100% { opacity: 0; transform: translate(8px, -16px) scale(1.1); } }
  @media (prefers-reduced-motion: reduce) { .avatar svg, .zz span { animation: none !important; } .zz span { opacity: 1; } }

  /* The character menu opens up and to the left (it sits at the bottom right) */
  .pick { position: absolute; bottom: calc(100% + 6px); right: 0; z-index: 20; display: grid; gap: 2px; padding: 6px; min-width: 150px; line-height: 1.4;
          border-radius: 10px; background: var(--bg1); border: 1px solid var(--bg3); box-shadow: 0 8px 24px #0008; }
  .pick button { display: flex; align-items: center; gap: 10px; padding: 4px 8px; border-radius: 6px; font: 12.5px var(--mono); text-align: left; white-space: nowrap; }
  .pick button:hover, .pick button.on { background: var(--bg2); }
  .pick svg, .pick .blank { flex: none; height: 26px; width: 32px; }
  .pick .dim { color: var(--grey); font-size: 11px; }
</style>
