// Requests other parts of the board answer: the Smart Object dialog lives
// in App.tsx, but the ☰ menu and the tools box open it too.
import { createSignal } from "solid-js";

export const [objectDialog, setObjectDialog] = createSignal(false);
export const openObjectDialog = () => setObjectDialog(true);
