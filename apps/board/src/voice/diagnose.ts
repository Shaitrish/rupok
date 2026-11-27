// Why the microphone did not work, and how to fix it, in words a teacher can
// follow. `micAdvice` is pure (unit-tested in apps/board/test/voice.test.ts);
// `gatherMicFacts` asks the browser what it knows (the site permission, the
// input devices) and `micLevelProbe` measures the input level for the
// "Test microphone" row in Settings.

import type { Engine, OsName } from "./select";
import type { VoiceErrorCode } from "./types";

export type MicPermission = "granted" | "denied" | "prompt" | "unknown";

export interface MicFacts {
  os: OsName;
  engine: Engine;
  edge: boolean;
  /** Inside the Rupok desktop app. */
  desktop: boolean;
  /** The site's microphone permission (Permissions API). */
  permission: MicPermission;
  /** Audio inputs the browser can see, or null when it would not say. */
  inputs: number | null;
  /** https or localhost (getUserMedia and Web Speech need it). */
  secure: boolean;
  /** Rupok runs inside another page's frame. */
  framed: boolean;
  error: VoiceErrorCode;
  /** The engine's own error text ("Permission denied by system"). */
  detail?: string;
}

export interface MicAdvice {
  title: string;
  text: string;
  steps: string[];
  /** Trying again can help (a prompt the teacher can now answer). */
  retry: boolean;
}

const browserName = (f: MicFacts) =>
  f.desktop ? "Rupok" : f.edge ? "Edge" : f.engine === "webkit" ? "Safari" : f.engine === "gecko" ? "Firefox" : "Chrome";

/** Where the operating system's own microphone switch is, per OS. */
function osSteps(f: MicFacts): string[] {
  const app = browserName(f);
  switch (f.os) {
    case "windows":
      return [
        "Open Windows Settings › Privacy & security › Microphone.",
        "Turn on “Microphone access”.",
        f.desktop ? "Turn on “Let desktop apps access your microphone”." : `Turn on “Let desktop apps access your microphone” (${app} is a desktop app).`,
        `Close and reopen ${app}, then tap the mic again.`,
      ];
    case "mac":
      return [
        "Open System Settings › Privacy & Security › Microphone.",
        `Turn on ${app === "Rupok" ? "Rupok" : app === "Safari" ? "Safari" : `“${app === "Edge" ? "Microsoft Edge" : "Google Chrome"}”`}.`,
        `Quit ${app} (⌘Q) and open it again, then tap the mic.`,
      ];
    case "linux":
      return [
        "Open Settings › Privacy & Security › Microphone and make sure it is on.",
        "Open Settings › Sound › Input: choose your microphone and check that the level bar moves when you speak.",
        ...(f.desktop ? [] : ["If Chrome or Chromium is a Snap, run: snap connect chromium:audio-record"]),
      ];
    case "chromeos":
      return ["Open Settings › Privacy and security › Privacy controls and turn on Microphone access."];
    default:
      return ["Check the device's privacy settings allow this app to use the microphone."];
  }
}

/** Where the site's microphone permission is, per browser. */
function siteSteps(f: MicFacts): string[] {
  if (f.engine === "webkit") {
    return ["In the menu bar choose Safari › Settings for This Website…", "Set Microphone to Allow.", "Reload the page and tap the mic again."];
  }
  if (f.engine === "gecko") {
    return ["Click the microphone or padlock icon at the left of the address bar.", "Remove the “Blocked” microphone permission.", "Reload the page and tap the mic again."];
  }
  return [
    "Click the site icon at the left of the address bar (the padlock or the ⚙ sliders).",
    f.edge ? "Choose Permissions for this site › Microphone › Allow." : "Turn Microphone on (or choose Site settings › Microphone › Allow).",
    "Reload the page and tap the mic again.",
  ];
}

/** The fix for a microphone failure, from what is known. */
export function micAdvice(f: MicFacts): MicAdvice {
  const app = browserName(f);
  const linuxToo = f.os === "linux" && !f.desktop ? [`Also check Linux: Settings › Privacy & Security › Microphone is on.`] : [];
  if (!f.desktop && !f.secure) {
    return {
      title: "The microphone needs a secure page",
      text: "Browsers only allow the microphone on https pages (or on this computer's localhost). Open Rupok at https://rupok.app.",
      steps: [],
      retry: false,
    };
  }
  if (f.error === "no-api") {
    return {
      title: "This app cannot reach the microphone",
      text: f.desktop
        ? "The web engine inside this copy of Rupok has no microphone support."
        : "This browser does not give pages the microphone.",
      steps: f.desktop && f.os === "linux"
        ? ["Install Rupok from the .deb package instead of the AppImage (it uses the system's WebKitGTK), or open Rupok in Google Chrome."]
        : ["Open Rupok in Google Chrome or Microsoft Edge."],
      retry: false,
    };
  }
  if (f.error === "service") {
    return {
      title: "This browser has no speech service",
      text: "Chromium, Brave, Opera and Vivaldi have the speech API but not Google's service behind it, so nothing can be recognised.",
      steps: ["Open Rupok in Google Chrome or Microsoft Edge, or use the Rupok desktop app (offline voice)."],
      retry: false,
    };
  }
  if (f.error === "no-mic" || f.inputs === 0) {
    return {
      title: "No microphone found",
      text: `${app} cannot find a microphone on this computer.`,
      steps: [
        "Plug in a microphone or headset (or switch the board's microphone on).",
        ...(f.os === "windows" ? ["Check Windows Settings › System › Sound › Input shows it."]
          : f.os === "mac" ? ["Check System Settings › Sound › Input shows it."]
          : f.os === "linux" ? ["Check Settings › Sound › Input shows it and its level moves."] : []),
        "Tap the mic again.",
      ],
      retry: true,
    };
  }
  if (f.error === "busy") {
    return {
      title: "The microphone is busy",
      text: "The microphone exists but could not be opened. Another app (Zoom, Meet, a recorder) may be using it, or the system blocks it.",
      steps: ["Close other apps that use the microphone.", ...osSteps(f)],
      retry: true,
    };
  }
  if (f.error === "not-allowed") {
    const systemDenied = /system/i.test(f.detail ?? "");
    // The desktop app grants its own board the microphone, so a refusal
    // there comes from the operating system.
    if (f.desktop || systemDenied || f.permission === "granted") {
      return {
        title: f.os === "windows" ? "Windows is blocking the microphone" : f.os === "mac" ? "macOS is blocking the microphone" : "The system is blocking the microphone",
        text: f.desktop ? "Rupok asked for the microphone, but the operating system refused." : `${app} allows this site the microphone, but the operating system refuses it to ${app}.`,
        steps: osSteps(f),
        retry: true,
      };
    }
    if (f.framed) {
      return {
        title: "The page around Rupok blocks the microphone",
        text: "Rupok is open inside another page (a frame) that does not pass the microphone on.",
        steps: ["Open https://rupok.app in its own browser tab."],
        retry: false,
      };
    }
    if (f.permission === "denied") {
      return {
        title: "The microphone is blocked for this site",
        text: `${app} was told not to let this site use the microphone.`,
        steps: [...siteSteps(f), ...linuxToo],
        retry: false,
      };
    }
    // "prompt" (the question was closed or ignored) or unknown.
    return {
      title: `${app} did not get permission`,
      text: `The microphone question was closed or not answered. Tap the mic again and choose Allow when ${app} asks (near the address bar).`,
      steps: [`If ${app} does not ask:`, ...siteSteps(f), ...linuxToo],
      retry: true,
    };
  }
  return { title: "Voice input failed", text: f.detail || "Something went wrong with the microphone.", steps: osSteps(f), retry: true };
}

// ------------------------------------------------------------ browser facts

/** The site's microphone permission, when the browser will tell. */
export async function micPermission(): Promise<MicPermission> {
  try {
    const st = await navigator.permissions.query({ name: "microphone" as PermissionName });
    return st.state as MicPermission;
  } catch {
    return "unknown"; // Firefox before 2024, older Safari, WebKitGTK
  }
}

/** How many audio inputs the browser sees (before permission Chrome still
 * lists one entry per kind when any exists; Safari may list none). */
export async function audioInputCount(): Promise<number | null> {
  try {
    if (!navigator.mediaDevices?.enumerateDevices) return null;
    const list = await navigator.mediaDevices.enumerateDevices();
    return list.filter((d) => d.kind === "audioinput").length;
  } catch {
    return null;
  }
}

export interface MicEnv {
  os: OsName;
  engine: Engine;
  edge: boolean;
  desktop: boolean;
}

/** Ask the browser what it knows about the microphone. */
export async function gatherMicFacts(env: MicEnv, error: VoiceErrorCode, detail?: string): Promise<MicFacts> {
  const [permission, inputs] = await Promise.all([micPermission(), audioInputCount()]);
  let framed = false;
  try { framed = window.top !== window.self; } catch { framed = true; }
  return {
    ...env,
    permission,
    // Safari hides devices until the page has the permission; 0 then means
    // nothing.
    inputs: env.engine === "webkit" && permission !== "granted" ? null : inputs,
    secure: window.isSecureContext !== false,
    framed,
    error,
    detail,
  };
}

/** A getUserMedia failure as a voice error code. */
export function mediaErrorCode(e: unknown): VoiceErrorCode {
  const name = (e as { name?: string })?.name ?? "";
  if (name === "NotAllowedError" || name === "SecurityError" || name === "PermissionDeniedError") return "not-allowed";
  if (name === "NotFoundError" || name === "OverconstrainedError" || name === "DevicesNotFoundError") return "no-mic";
  if (name === "NotReadableError" || name === "AbortError" || name === "TrackStartError") return "busy";
  return "other";
}

// ------------------------------------------------------------- level probe

export interface LevelProbe {
  /** Peak level so far, 0–1. */
  peak(): number;
  stop(): void;
}

/** Open the microphone and report its level (Settings › Test microphone).
 * Call from the tap itself: some engines only start audio in a gesture. */
export async function micLevelProbe(onLevel: (v: number) => void): Promise<LevelProbe> {
  const AC: typeof AudioContext = (window as any).AudioContext || (window as any).webkitAudioContext;
  const ctx = new AC();
  void ctx.resume().catch(() => {});
  let stream: MediaStream;
  try {
    if (!navigator.mediaDevices?.getUserMedia) throw Object.assign(new Error("no getUserMedia"), { name: "NoApi" });
    stream = await navigator.mediaDevices.getUserMedia({ audio: { echoCancellation: false, noiseSuppression: false, autoGainControl: true } });
  } catch (e) {
    void ctx.close().catch(() => {});
    throw e;
  }
  const src = ctx.createMediaStreamSource(stream);
  const an = ctx.createAnalyser();
  an.fftSize = 1024;
  src.connect(an);
  const buf = new Float32Array(an.fftSize);
  let peak = 0;
  let raf = 0;
  let stopped = false;
  const tick = () => {
    if (stopped) return;
    an.getFloatTimeDomainData(buf);
    let p = 0;
    for (let i = 0; i < buf.length; i++) p = Math.max(p, Math.abs(buf[i]));
    peak = Math.max(peak, p);
    onLevel(Math.min(1, p * 2.5));
    raf = requestAnimationFrame(tick);
  };
  raf = requestAnimationFrame(tick);
  return {
    peak: () => peak,
    stop() {
      stopped = true;
      cancelAnimationFrame(raf);
      stream.getTracks().forEach((t) => t.stop());
      void ctx.close().catch(() => {});
      onLevel(0);
    },
  };
}
