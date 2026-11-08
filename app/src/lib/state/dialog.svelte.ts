// State of the confirmation dialog (ConfirmDialog.svelte). One request is
// open at a time; opening another answers the first with "cancel".

export interface ConfirmRequest {
  title: string;
  message: string;
  /** Lines listed under the message (for example the sessions a tab close ends). */
  details: string[];
  confirmLabel: string;
}

interface OpenConfirm extends ConfirmRequest {
  resolve(confirmed: boolean): void;
}

export const dialog = $state({ confirm: null as OpenConfirm | null });

export function askConfirm(request: ConfirmRequest): Promise<boolean> {
  dialog.confirm?.resolve(false);
  return new Promise((resolve) => {
    dialog.confirm = { ...request, resolve };
  });
}

export function answerConfirm(confirmed: boolean): void {
  const open = dialog.confirm;
  dialog.confirm = null;
  open?.resolve(confirmed);
}
