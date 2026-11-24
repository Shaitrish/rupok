// Settings → About: version, licences and links.

import { APP_NAME, SITE_URL } from "../../brand";
import type { SettingProps } from "../../settings/registry";
import { Mark } from "../Icon";
import { controller } from "../../controller";
import { graphicsAdapter } from "../../inkLatency";

export const VERSION = "0.2.2";
const REPO = "https://github.com/shaitrish/rupok";

export function AboutSection(props: SettingProps) {
  return (
    <div class="st-about">
      <div class="st-about-head">
        <Mark size={40} />
        <div>
          <div class="st-about-name" id={props.labelId}>{APP_NAME}</div>
          <div class="st-help" id={props.descId}>Version {VERSION} · pre-alpha · the open-source whiteboard for teachers</div>
        </div>
      </div>
      <dl class="st-facts">
        <dt>Licence</dt><dd>AGPL-3.0-or-later. The file format, Smart Object SDK and agent skills are Apache-2.0.</dd>
        <dt>Made by</dt><dd>Shaitrish</dd>
        <dt>Icons</dt><dd>Solar by 480 Design (CC BY 4.0)</dd>
        <dt>Typefaces</dt><dd>Inter and Hind Siliguri (SIL Open Font License)</dd>
      </dl>
      <div class="st-inline">
        <a class="st-btn" href={SITE_URL} target="_blank" rel="noreferrer">Website</a>
        <a class="st-btn" href={REPO} target="_blank" rel="noreferrer">Source code</a>
        <a class="st-btn" href={`${REPO}/issues`} target="_blank" rel="noreferrer">Report a problem</a>
      </div>
    </div>
  );
}

/** Settings → About → Troubleshooting → Graphics: the adapter and backend. */
export function GraphicsInfo(props: SettingProps) {
  const a = graphicsAdapter();
  let backend = "";
  try {
    backend = controller.board.backend().replace("BrowserWebGpu", "WebGPU").replace(/^Gl\b/, "WebGL2");
  } catch {
    backend = "not started";
  }
  return (
    <dl class="st-facts" id={props.descId}>
      <dt>Renderer</dt><dd>{backend}</dd>
      <dt>Adapter</dt><dd>{a.software ? `Software, no GPU (${a.name})` : a.name}</dd>
    </dl>
  );
}
