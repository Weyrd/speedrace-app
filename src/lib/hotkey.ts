const isMac =
  typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);

function codeToKey(code: string): string | null {
  if (/^Key[A-Z]$/.test(code)) return code.slice(3); // KeyF -> F
  if (/^Digit[0-9]$/.test(code)) return code.slice(5); // Digit1 -> 1
  if (/^F([1-9]|1[0-9]|2[0-4])$/.test(code)) return code; // F1..F24
  const map: Record<string, string> = {
    Enter: "Enter",
    NumpadEnter: "Enter",
    Space: "Space",
    Tab: "Tab",
    Backspace: "Backspace",
    Delete: "Delete",
    Insert: "Insert",
    Home: "Home",
    End: "End",
    PageUp: "PageUp",
    PageDown: "PageDown",
    ArrowUp: "Up",
    ArrowDown: "Down",
    ArrowLeft: "Left",
    ArrowRight: "Right",
    Minus: "Minus",
    Equal: "Equal",
    BracketLeft: "BracketLeft",
    BracketRight: "BracketRight",
    Semicolon: "Semicolon",
    Quote: "Quote",
    Backquote: "Backquote",
    Comma: "Comma",
    Period: "Period",
    Slash: "Slash",
    Backslash: "Backslash",
  };
  return map[code] ?? null;
}


export function eventToAccelerator(e: KeyboardEvent): string | null {
  const key = codeToKey(e.code);
  if (!key) return null; // modifier-only or unsupported key

  const parts: string[] = [];
  if (e.ctrlKey || e.metaKey) parts.push("CmdOrCtrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  if (parts.length === 0) return null; // require at least one modifier

  parts.push(key);
  return parts.join("+");
}

export function eventToLiveAccelerator(e: KeyboardEvent): string {
  const parts: string[] = [];
  if (e.ctrlKey || e.metaKey) parts.push("CmdOrCtrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  const key = codeToKey(e.code);
  if (key) parts.push(key);
  return parts.join("+");
}

export function formatAccelerator(accel: string): string {
  return accel
    .split("+")
    .map((part) => {
      switch (part) {
        case "CmdOrCtrl":
        case "CommandOrControl":
          return isMac ? "⌘" : "Ctrl";
        case "Ctrl":
        case "Control":
          return isMac ? "⌃" : "Ctrl";
        case "Alt":
          return isMac ? "⌥" : "Alt";
        case "Shift":
          return isMac ? "⇧" : "Shift";
        case "Super":
        case "Meta":
          return isMac ? "⌘" : "Win";
        default:
          return part;
      }
    })
    .join("+");
}
