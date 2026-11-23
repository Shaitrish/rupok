// Settings → General → Library: where lessons live and browser storage,
// reusing the Library's own settings UI (components/LibrarySettings.tsx).

import type { SettingProps } from "../../settings/registry";
import { LibrarySettings } from "../LibrarySettings";

export function LibraryLocation(props: SettingProps) {
  return (
    <div class="st-embed" role="group" aria-labelledby={props.labelId}>
      <LibrarySettings only="storage" />
    </div>
  );
}
