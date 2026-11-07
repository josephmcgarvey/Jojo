import type { Settings } from "../backend/Settings";

export const app = $state({ settings: null as Settings | null });
