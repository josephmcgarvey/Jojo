// Short messages in a pane's header strip: "Copied" (spec 9.5 rule 2) and
// errors such as a link that could not be opened.

/** Spec 9.5 rule 2: the header shows "Copied" for 1.2 seconds. */
export const COPIED_MS = 1200;
/** Errors stay longer so they can be read. */
export const ERROR_MS = 4000;

export interface Notice {
  text: string;
  tone: "info" | "error";
  /** Identifies this notice, so an older timer does not remove a newer notice. */
  serial: number;
}

export const notices: Record<string, Notice> = $state({});
let lastSerial = 0;

export function showNotice(sessionId: string, text: string, tone: "info" | "error" = "info"): void {
  const serial = ++lastSerial;
  notices[sessionId] = { text, tone, serial };
  setTimeout(
    () => {
      if (notices[sessionId]?.serial === serial) delete notices[sessionId];
    },
    tone === "info" ? COPIED_MS : ERROR_MS,
  );
}
