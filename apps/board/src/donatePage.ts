// The donate page (/donate/, /bn/donate/; site/pages/donate.mjs):
// - the impact counter: real public numbers from GitHub's API (release
//   downloads, stars, releases), cached for six hours, with a link instead of
//   numbers when GitHub can't be reached;
// - suggested amounts that fill in "Send ৳500 to …";
// - copy buttons for numbers.
// No analytics and nothing sent anywhere except the two GitHub API reads.

const REPO_API = "https://api.github.com/repos/Shaitrish/rupok";
const CACHE = "rupok.impact";
const TTL = 6 * 60 * 60 * 1000;

interface Impact { at: number; downloads: number; stars: number; releases: number }

function readCache(): Impact | null {
  try {
    const raw = localStorage.getItem(CACHE);
    const v = raw ? (JSON.parse(raw) as Impact) : null;
    return v && typeof v.downloads === "number" ? v : null;
  } catch {
    return null;
  }
}

async function fetchImpact(): Promise<Impact | null> {
  const get = async (url: string) => {
    const r = await fetch(url, { headers: { Accept: "application/vnd.github+json" } });
    if (!r.ok) throw new Error(String(r.status));
    return r.json();
  };
  try {
    const repo = await get(REPO_API);
    let downloads = 0;
    let releases = 0;
    // Every release's assets; five pages of 100 is far more than Rupok has.
    for (let page = 1; page <= 5; page++) {
      const list: { draft?: boolean; assets?: { download_count?: number }[] }[] = await get(`${REPO_API}/releases?per_page=100&page=${page}`);
      for (const rel of list) {
        if (rel.draft) continue;
        releases++;
        for (const a of rel.assets ?? []) downloads += a.download_count ?? 0;
      }
      if (list.length < 100) break;
    }
    const v: Impact = { at: Date.now(), downloads, stars: Number(repo.stargazers_count) || 0, releases };
    try { localStorage.setItem(CACHE, JSON.stringify(v)); } catch { /* storage unavailable */ }
    return v;
  } catch {
    return null; // offline or rate-limited
  }
}

function showImpact(box: HTMLElement, v: Impact | null) {
  const locale = box.dataset.lang === "bn" ? "bn-BD" : "en-US";
  box.dataset.state = v ? "ok" : "error";
  box.querySelectorAll<HTMLElement>("[data-impact-n]").forEach((el) => {
    const n = v?.[el.dataset.impactN as keyof Omit<Impact, "at">];
    el.textContent = typeof n === "number" ? n.toLocaleString(locale) : "–";
  });
  box.querySelector<HTMLElement>("[data-impact-ok]")!.hidden = !v;
  box.querySelector<HTMLElement>("[data-impact-err]")!.hidden = !!v;
}

const impact = document.querySelector<HTMLElement>("[data-impact]");
if (impact) {
  const cached = readCache();
  if (cached) showImpact(impact, cached);
  if (!cached || Date.now() - cached.at > TTL) {
    void fetchImpact().then((v) => {
      if (v) showImpact(impact, v);
      else if (!cached) showImpact(impact, null); // a stale count beats none
    });
  }
}

// ------------------------------------------------------ suggested amounts
document.querySelectorAll<HTMLElement>("[data-amts]").forEach((group) => {
  group.hidden = false;
  const card = group.closest<HTMLElement>(".method");
  const out = card?.querySelector<HTMLElement>("[data-amt-out]");
  const any = out?.textContent ?? "";
  const buttons = Array.from(group.querySelectorAll<HTMLButtonElement>("[data-amt]"));
  const pick = (b: HTMLButtonElement | null) => {
    for (const x of buttons) x.setAttribute("aria-checked", String(x === b));
    if (out) {
      out.textContent = b ? b.dataset.amt! : any;
      out.classList.toggle("set", !!b);
    }
  };
  // Roving focus: the group is one tab stop; arrows move and pick.
  buttons.forEach((b, i) => {
    b.tabIndex = i === 0 ? 0 : -1;
    b.addEventListener("click", () => pick(b.getAttribute("aria-checked") === "true" ? null : b));
    b.addEventListener("keydown", (e) => {
      const d = e.key === "ArrowRight" || e.key === "ArrowDown" ? 1 : e.key === "ArrowLeft" || e.key === "ArrowUp" ? -1 : 0;
      if (!d) return;
      e.preventDefault();
      const next = buttons[(i + d + buttons.length) % buttons.length];
      for (const x of buttons) x.tabIndex = x === next ? 0 : -1;
      next.focus();
      pick(next);
    });
  });
});

// ------------------------------------------------------------ copy buttons
document.querySelectorAll<HTMLButtonElement>("button[data-copy]").forEach((b) => {
  if (!navigator.clipboard?.writeText) return; // stays hidden: select the text instead
  b.hidden = false;
  const label = b.textContent;
  let timer: number | undefined;
  b.addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText(b.dataset.copy!);
      b.textContent = b.dataset.done ?? label;
      b.classList.add("done");
      clearTimeout(timer);
      timer = window.setTimeout(() => {
        b.textContent = label;
        b.classList.remove("done");
      }, 1600);
    } catch { /* permission denied: nothing to do */ }
  });
});
