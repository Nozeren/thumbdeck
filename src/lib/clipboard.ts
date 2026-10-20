// Text onto the clipboard, from a key press (y). The async clipboard API first; the old
// execCommand("copy") when the webview refuses it (WebKitGTK sometimes does).
export async function copyText(text: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(text);
    return;
  } catch {
    // below
  }
  const area = document.createElement("textarea");
  area.value = text;
  area.setAttribute("readonly", "");
  area.style.cssText = "position:fixed;left:-9999px;top:0;opacity:0";
  const focused = document.activeElement as HTMLElement | null;
  document.body.append(area);
  area.select();
  const ok = document.execCommand("copy");
  area.remove();
  focused?.focus({ preventScroll: true });
  if (!ok) throw new Error("couldn't copy");
}

/** "3 lines", "1 line" */
export const linesWord = (n: number) => `${n} line${n === 1 ? "" : "s"}`;
