// Where the chat's answers come from. Today: the teacher's own agent on this
// computer (ACP, through `rupok agent` or the desktop app; chat.ts). Later:
// Rupok AI, a cloud provider for teachers signed in to Rupok Cloud. The
// interface is here so the picker and the panel do not care which.

import { cloudUser } from "./cloud/account";

export interface ChatModel {
  id: string;
  name: string;
  description?: string;
}

export interface ChatProvider {
  /** "agent:<id>" for local agents, "rupok-ai" for the cloud. */
  id: string;
  label: string;
  kind: "agent" | "cloud";
  /** Shown but not selectable yet. */
  comingSoon?: boolean;
  /** Can be picked now. */
  available(): boolean;
  /** One line under the label in the picker. */
  note(): string;
  /** Models this provider offers (a cloud provider offers only its own). */
  models(): ChatModel[];
}

/** Rupok AI: offered to signed-in Rupok Cloud teachers. No backend yet; the
 * models will come from the Rupok Cloud API (`/api/v1/ai/models`). */
export const rupokAi: ChatProvider = {
  id: "rupok-ai",
  label: "Rupok AI",
  kind: "cloud",
  comingSoon: true,
  available: () => false,
  note: () => "Coming soon to Rupok Cloud",
  models: () => [],
};

/** Cloud providers to list in the picker: only for signed-in teachers. */
export function cloudProviders(): ChatProvider[] {
  return cloudUser() ? [rupokAi] : [];
}
