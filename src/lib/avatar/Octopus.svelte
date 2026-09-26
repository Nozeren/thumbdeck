<script lang="ts">
  // The octopus in the top bar (a nod to split keyboards' thumb keys): pixel art that shows
  // what's going on. Hover for the caption. Stops animating while the window is hidden.
  import type { Mood } from "./mood.ts";
  import { frame, WIDTH } from "./sprite.ts";

  let { mood, caption }: { mood: Mood; caption: string } = $props();

  const COLORS: Record<string, string> = {
    b: "var(--orange)", c: "var(--red)", k: "var(--bg-dim)", w: "var(--fg)", m: "var(--bg-dim)", o: "var(--red)",
  };
  const SIGNS: Partial<Record<Mood, [string, string]>> = {
    failed: ["✗", "red"], done: ["✓", "green"], waiting: ["?", "orange"], thinking: ["…", "fg"], reading: ["≡", "aqua"],
    review: ["!", "yellow"], update: ["★", "yellow"], sleeping: ["z", "grey"], night: ["☾", "yellow"],
  };

  let tick = $state(1);
  $effect(() => {
    const timer = setInterval(() => {
      if (!document.hidden) tick++;
    }, 200);
    return () => clearInterval(timer);
  });

  /** Runs of one colour in each row, as rectangles */
  const rects = $derived.by(() => {
    const out: { x: number; y: number; w: number; fill: string }[] = [];
    frame(mood, tick).forEach((row, y) => {
      for (let x = 0; x < WIDTH; ) {
        let end = x;
        while (end < WIDTH && row[end] === row[x]) end++;
        if (COLORS[row[x]]) out.push({ x, y, w: end - x, fill: COLORS[row[x]] });
        x = end;
      }
    });
    return out;
  });
  const sign = $derived(SIGNS[mood]);
</script>

<span class="octopus {mood}" title={caption} role="img" aria-label={caption}>
  <svg viewBox="0 0 {WIDTH} 13" shape-rendering="crispEdges">
    {#each rects as r}<rect x={r.x} y={r.y} width={r.w} height="1" fill={r.fill} />{/each}
  </svg>
  {#if sign}<span class="sign" style="color: var(--{sign[1]})">{sign[0]}</span>{/if}
</span>

<style>
  .octopus { position: relative; display: inline-flex; align-items: flex-end; height: 42px; vertical-align: middle; }
  /* 3 screen pixels per art pixel: whole numbers keep the edges sharp */
  svg { height: 39px; width: 48px; }
  .sign { position: absolute; left: 100%; top: -2px; font: 700 15px var(--mono); margin-left: 2px; }
  .sleeping .sign { animation: float 2.4s ease-in-out infinite; }
  .octopus.idle svg, .octopus.review svg, .octopus.update svg { animation: bob 2.4s ease-in-out infinite; }
  .octopus.running svg { animation: bob 0.5s ease-in-out infinite; }
  .octopus.done svg { animation: jump 0.6s ease-out 3; }
  .octopus.failed svg { animation: shake 0.4s linear 2; }
  .octopus.thinking svg { animation: sway 3s ease-in-out infinite; }
  .octopus.sleeping svg, .octopus.night svg { animation: breathe 4s ease-in-out infinite; }
  @keyframes bob { 50% { transform: translateY(-2px); } }
  @keyframes jump { 40% { transform: translateY(-8px); } }
  @keyframes shake { 25% { transform: translateX(-3px); } 75% { transform: translateX(3px); } }
  @keyframes sway { 25% { transform: rotate(-4deg); } 75% { transform: rotate(4deg); } }
  @keyframes breathe { 50% { transform: scaleY(0.94); } }
  @keyframes float { 0% { opacity: 0; transform: translateY(3px); } 50% { opacity: 1; } 100% { opacity: 0; transform: translateY(-8px); } }
  svg { transform-origin: 50% 100%; }
  @media (prefers-reduced-motion: reduce) { .octopus svg, .sign { animation: none !important; } }
</style>
