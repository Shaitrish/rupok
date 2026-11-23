# Rupok Cloud

`Status: 📋 planned · Phase 2–3 · Difficulty: advanced`

The paid, hosted part of Rupok's open core: Google sign-in, 500 MB of free
cloud storage per teacher, share links students open with no account, a
Claude connector, Google Drive integration and, later, class spaces and live
classes. It runs on Cloudflare Workers, D1 and R2.

## Goal

Rupok, the app, stays free and open source forever. Rupok Cloud is the
convenience layer teachers pay for when they want more. It has to:

- **Feel free on the free plan.** 500 MB per teacher is enough for a term of
  lessons, so most teachers never pay.
- **Cost almost nothing to run.** The founder's budget is the $5/month
  Workers Paid plan. The free tier must stay inside it, or close to it, for
  the first thousands of teachers (see [Cost model](#cost-model)).
- **Pay for itself through Pro and School,** sold with Paddle first, bKash
  later.
- **Respect the open-core promise.** Lessons are open `.rpk` files that
  teachers can always download. The app works fully without the cloud, and
  the share API is documented.

## Decisions already made (founder)

| Topic | Decision |
|---|---|
| Model | Open core. App AGPL-3.0; spec, SDK and skills Apache-2.0; CLA for contributors. Rupok Cloud is the paid, hosted part. |
| Accounts | Google sign-in only, at first. |
| Free plan | 500 MB per teacher. |
| Paid plans | Pro (teachers: more storage, AI credits, live class) and School (whole schools: seats, admin, classes). |
| Billing | Paddle first; bKash later. |
| Sharing | Teacher uploads, shares `rupok.app/s/<id>`; students see the live lesson (replay, working Smart Objects, Download PDF) with no account. Class spaces later. |
| Claude | A remote MCP server on Workers with OAuth (`workers-oauth-provider`), so teachers manage lessons from their own Claude. |
| Google Drive | Files can stay in the teacher's Drive (`drive.file`). A Drive "Open with → Rupok" integration through the Google Workspace Marketplace. |
| Android | A Web Share Target, so a `.rpk` received in WhatsApp can be shared straight into Rupok. |

## User stories

- *As a teacher*, I sign in with Google, press **Upload** on tonight's lesson,
  and paste the link into the class WhatsApp group. Students tap it and the
  lesson opens in their browser: replay, working simulations, Download PDF.
  Nobody makes an account.
- *As a teacher*, I fix a mistake in the lesson after class and upload again.
  The same link now shows the corrected version.
- *As a teacher*, I ask Claude: "Add a titration simulation to page 4 of my
  acid–base lesson and send me the share link." Claude does it through the
  Rupok connector.
- *As a teacher*, I keep my lessons in my own Google Drive. Rupok only sees
  files it created or that I opened with it, and they don't count against my
  500 MB.
- *As a student*, my teacher shared a `.rpk` in Google Drive. I tap it, choose
  **Open with → Rupok**, and it opens in the viewer.
- *As a student on Android*, I long-press the `.rpk` my teacher sent on
  WhatsApp, tap **Share → Rupok**, and the lesson opens.
- *As a head teacher*, I buy the School plan: every teacher gets Pro, I see
  who is using it, and we share one course library.
- *As a teacher in Bangladesh*, I pay for Pro with bKash.

## Scope

**In (this plan):** Google sign-in, sessions; lesson upload, storage, quotas;
share links and the share viewer; plans, quotas and Paddle billing; the remote
MCP connector; Google Drive "Open with"; Android Web Share
Target; abuse handling; privacy; the open-core boundary; the cost model.

**Later (own plans):** class spaces and rosters, the School admin, bKash, live
classes on Durable Objects (the hosted twin of
[live-broadcast](./live-broadcast.md)), Rupok AI credits.

## Non-goals

- Replacing local files. The app keeps working offline with local `.rpk`
  files and the browser autosave; the cloud is optional.
- Real-time co-editing between teachers (Loro makes it possible later).
- Student accounts in the first phases. Students never sign in to view a
  share link.
- Hosting arbitrary files. Only valid Rupok lessons are accepted.

## Design

### Architecture

```text
             rupok.app (one Worker, static assets + API)
 ┌───────────────────────────────────────────────────────────────┐
 │ static assets (free, unlimited): /, /open/, /board/, /compare/ │
 │ Worker routes (run_worker_first):                               │
 │   /api/*            JSON API for the app (session cookie)       │
 │   /s/<id>           share page: viewer shell + OG tags          │
 │   /mcp              remote MCP (createMcpHandler)               │
 │   /oauth/*, /authorize, /token   OAuth 2.1 (workers-oauth-provider) │
 │   /.well-known/oauth-*           MCP discovery (when /mcp ships) │
 └──────┬──────────────┬───────────────┬──────────────┬───────────┘
        │              │               │              │
     D1 (SQLite)    R2 bucket       KV OAUTH_KV    Durable Objects (later)
     users, lessons  lesson .rpk,   OAuth tokens   LiveRoom per class
     shares, plans   thumbnails     (library       session (WebSocket
     usage, audit    (APAC hint)    requirement)   hibernation)
        │
     Google (sign-in, Drive API) · Paddle (webhooks) · bKash (later)
```

- **One Worker.** Today `apps/board/wrangler.jsonc` serves static assets only.
  Add a `main` script and `assets.run_worker_first` for the routes above.
  Everything else stays a free static-asset request. Static asset requests
  are free and unlimited, as long as Workers Caching isn't turned on for them
  ([pricing](https://developers.cloudflare.com/workers/platform/pricing/)).
- **D1** holds metadata only (small rows). One database to start; D1's 10 GB
  per-database limit is far away (about 50 KB of metadata per teacher).
- **R2** holds the `.rpk` bytes and page thumbnails. Egress is free, which
  matters most: a viral share link costs Class B operations, not bandwidth.
- **KV** only for `OAUTH_KV`, which `workers-oauth-provider` requires.
  Don't use it for lesson data (eventual consistency, $5/M writes).
- **Durable Objects** come with live classes: one `LiveRoom` per session, using
  the WebSocket Hibernation API so idle rooms cost nothing.
- **Wasm in the Worker.** `rupok-format` and `rupok-core` already build for
  `wasm32-unknown-unknown` with no clock (see AGENTS.md). The Worker reuses
  them to **validate every upload** (`Package::read` with its reader limits)
  and to **edit lessons for the MCP connector** (the same `ops` as
  `rupok-cli`). One code path, the same safety rules.

### Sign-in and sessions

- Google Identity Services: the "Sign in with Google" button and One Tap (FedCM)
  return an ID token. The Worker verifies it (Google JWKS, `aud`, `iss`,
  `exp`) and upserts the user by `sub`
  ([GIS overview](https://developers.google.com/identity/gsi/web/guides/overview)).
- Session: a random 256-bit token in an `HttpOnly; Secure; SameSite=Lax`
  cookie; D1 stores its SHA-256 with an expiry (30 days, sliding). Logout
  deletes the row.
- Scopes: sign-in needs only `openid email profile`. The Drive "Open with"
  integration asks for `drive.file` / `drive.install` separately, only when used.
- Desktop app (Tauri): the system browser with PKCE and a loopback redirect,
  as in [google-drive-sync](./google-drive-sync.md).

### Upload, versions and quotas

1. App: `POST /api/lessons` (or `PUT /api/lessons/:id`) with the `.rpk` bytes.
   Request bodies up to 100 MB are fine on our zone plan; we cap lessons at
   50 MB.
2. The Worker checks the quota (`SUM(bytes)` from D1 plus the new size; 500 MB
   free), then validates the package in wasm. Invalid, oversized or
   not-a-lesson uploads are rejected (this is also the main abuse filter).
3. It writes to R2 at `u/<user>/l/<lesson>/<version>.rpk`, keeping the last 3
   versions (older ones deleted; deletes are free on R2). It stores
   `sha256`, so re-uploading the same bytes is a no-op.
4. The page-1 thumbnail (WebP) is rendered by the app and uploaded alongside,
   for link previews and the lesson list.

Quotas count the latest version plus kept versions. Drive-mode lessons don't
count.

### Share links: `rupok.app/s/<id>`

- `<id>` is 10 characters of base62 (about 8×10¹⁷ values): unguessable
  enough for "anyone with the link", short enough to read aloud.
- `GET /s/<id>` (Worker): looks up the share in D1. Revoked, expired or
  missing shares return 404. Otherwise it returns the viewer shell (the
  board's HTML in Student mode) with Open Graph tags (lesson title, page-1
  thumbnail), so WhatsApp shows a proper preview card.
- The viewer fetches `GET /api/s/<id>/lesson`, which streams the R2 object
  with `Cache-Control: private, max-age=300` and an `ETag`. A share points
  to the lesson, not a version, so re-uploading updates the link.
- Everything students can do works today in the local viewer: replay, Smart
  Objects (sandboxed exactly as locally), Download PDF. Nothing is written
  back.
- Options per share: expiry, "allow download of the `.rpk`" (default on, since
  the format is open), revoke.
- View counts: incremented in D1 at most once per IP hash per share per day
  (no cookies, no personal data).

### Claude connector (remote MCP)

- `createMcpHandler` from the Agents SDK over Streamable HTTP at `/mcp`.
  Cloudflare now marks `McpAgent` as deprecated and feature-frozen
  ([docs](https://developers.cloudflare.com/agents/model-context-protocol/mcp-agent-api/)).
- Auth: `@cloudflare/workers-oauth-provider` (OAuth 2.1 with PKCE; 1.2.4 as of
  Oct 2026). Our `/authorize` page is the normal Google sign-in, then a consent
  screen ("Claude wants to read and edit your Rupok lessons").
- Client registration: support **Client ID Metadata Documents** (Claude's
  published identity, which Anthropic recommends) and keep Dynamic Client
  Registration for older clients. The MCP spec (2026-07-28) deprecates DCR
  ([spec](https://modelcontextprotocol.io/specification/2026-07-28/basic/authorization)).
  Claude's callback is `https://claude.ai/api/mcp/auth_callback`
  ([Claude docs](https://claude.com/docs/connectors/custom/add-unlisted)).
- Scopes: `lessons:read`, `lessons:write`, `shares:write`. The library
  advertises scopes but doesn't enforce them; each tool checks
  `ctx.auth.scope`.
- Tools: the 11 local tools (same names and arguments as `rupok mcp`, so the
  skills in `skills/` work unchanged), plus `share_lesson` (returns the link),
  `list_shares`, `revoke_share` and `get_usage`. Edits load the `.rpk` from
  R2, apply the shared `ops` in wasm, validate, and save a new version.
- **Discovery endpoints, published only when `/mcp` ships** (not before):
  - `https://rupok.app/.well-known/oauth-protected-resource/mcp` (RFC 9728)
  - `https://rupok.app/.well-known/oauth-authorization-server` (RFC 8414)
  - and `401` responses carry `WWW-Authenticate: Bearer resource_metadata="…"`
  ([library docs](https://github.com/cloudflare/workers-oauth-provider/blob/main/docs/mcp-discovery.md)).
- Claude's free plan allows one custom connector, so this works for free
  teachers too ([help](https://support.claude.com/en/articles/11175166-get-started-with-custom-connectors-using-remote-mcp)).
  Later, apply for the Claude connectors directory.

### Google Drive

**No Drive storage mode (decision, Nov 2026).** Rupok Cloud stores lessons
itself; it does not sync into the teacher's Google Drive, and Rupok does not
build a Drive sync engine (see [google-drive-sync](./google-drive-sync.md),
now not planned). Lessons on a teacher's computer are plain `.rpk` files, so a
Drive/OneDrive/Dropbox desktop client can still back them up with no Rupok
code. The paid value of Rupok Cloud is the classroom layer Drive can't
provide: no-account student links with live Smart Objects and replay, class
"make a copy", conflict-free multi-device merge, the Claude connector,
version history, live classes, quizzes and school admin.

**"Open with → Rupok" (Drive UI integration).**

- Needs the `drive.install` scope (also non-sensitive) and the Drive UI
  integration in the Google Workspace Marketplace SDK: Open URL
  `https://rupok.app/drive/open?state={state}`, default extension `rpk`,
  MIME `application/vnd.rupok.lesson+zip`.
- Drive calls the Open URL with a JSON `state`
  (`{"ids":[…],"action":"open","userId":"…","resourceKeys":{…}}`). The page
  gets a `drive.file` token for that user, downloads the file in the browser,
  and hands it to the viewer through the same IndexedDB hand-off as `/open/`
  (`apps/board/src/pending.ts`).
- Publishing: a **public Marketplace listing**, reviewed by Google. The
  visibility can't be changed later, and @gmail.com developer accounts can
  only publish publicly
  ([Marketplace SDK](https://developers.google.com/workspace/marketplace/sdk)).
- Caveat: each student has to install the Drive app once and grant
  `drive.file`, which means signing in to Google. That's normal on school
  Google accounts, but it isn't "no account" like a share link. For students
  without Google, share links stay the main path.

### Android: Web Share Target

- Add to `public/manifest.webmanifest`:

  ```json
  "share_target": {
    "action": "/board/share", "method": "POST", "enctype": "multipart/form-data",
    "params": { "files": [{ "name": "lesson", "accept": [".rpk", "application/vnd.rupok.lesson+zip", "application/zip", "application/octet-stream"] }] }
  }
  ```

- A service worker (the same one the [web viewer](./web-viewer.md) PWA work
  adds) handles `POST /board/share`. It reads `request.formData()`, stores the
  file with `stashPendingLesson()` (already in `src/pending.ts`), and answers
  `303 → /board/?view&pending=1`. The board's existing startup hook opens it.
- It works only for an **installed** PWA on Android (WebAPK) and ChromeOS. It
  doesn't work on iOS, Firefox or in Android WebView
  ([web.dev](https://web.dev/learn/pwa/os-integration),
  [MDN](https://developer.mozilla.org/en-US/docs/Web/Progressive_web_apps/Manifest/Reference/share_target)).
  iPhone users use `/open/`.
- `application/octet-stream` is in `accept` because Android often labels an
  unknown `.rpk` that way. Rupok will then show up for other files too; the
  viewer explains politely when a file isn't a lesson.
- Whether WhatsApp's share sheet offers PWAs for documents is **unverified**:
  test on a real device (task 1 below).
- Server fallback: if the service worker isn't active, the Worker answers
  `POST /board/share` with `303 → /open/` (the file can't be kept server-side
  without an account).

The same manifest's `file_handlers` entry (`.rpk` → `/board/`) already works.
The board consumes `launchQueue` (`onLaunchFiles` in `src/pending.ts`).

### Data model (D1)

```sql
CREATE TABLE users (
  id TEXT PRIMARY KEY,                 -- uuid
  google_sub TEXT UNIQUE NOT NULL,
  email TEXT NOT NULL, name TEXT, avatar_url TEXT,
  plan TEXT NOT NULL DEFAULT 'free',   -- free | pro | school
  org_id TEXT REFERENCES orgs(id),
  created_at INTEGER NOT NULL, deleted_at INTEGER
);
CREATE TABLE sessions (token_hash TEXT PRIMARY KEY, user_id TEXT NOT NULL, expires_at INTEGER NOT NULL);
CREATE TABLE orgs (                    -- School plan
  id TEXT PRIMARY KEY, name TEXT NOT NULL, seats INTEGER NOT NULL,
  owner_id TEXT NOT NULL, created_at INTEGER NOT NULL
);
CREATE TABLE lessons (
  id TEXT PRIMARY KEY, owner_id TEXT NOT NULL REFERENCES users(id),
  title TEXT NOT NULL, pages INTEGER, bytes INTEGER NOT NULL, sha256 TEXT NOT NULL,
  storage TEXT NOT NULL DEFAULT 'r2',  -- r2 | drive
  r2_key TEXT, drive_file_id TEXT, version INTEGER NOT NULL DEFAULT 1,
  created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, deleted_at INTEGER
);
CREATE INDEX lessons_owner ON lessons(owner_id, updated_at DESC);
CREATE TABLE lesson_versions (lesson_id TEXT, version INTEGER, r2_key TEXT, bytes INTEGER, created_at INTEGER, PRIMARY KEY (lesson_id, version));
CREATE TABLE shares (
  id TEXT PRIMARY KEY,                 -- 10-char base62, the /s/<id>
  lesson_id TEXT NOT NULL REFERENCES lessons(id), created_by TEXT NOT NULL,
  allow_download INTEGER NOT NULL DEFAULT 1,
  expires_at INTEGER, revoked_at INTEGER, views INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL
);
CREATE TABLE subscriptions (
  id TEXT PRIMARY KEY, user_id TEXT, org_id TEXT,
  provider TEXT NOT NULL,              -- paddle | bkash
  provider_ref TEXT NOT NULL, plan TEXT NOT NULL, status TEXT NOT NULL,
  current_period_end INTEGER, updated_at INTEGER NOT NULL
);
CREATE TABLE usage_monthly (user_id TEXT, month TEXT, uploads INTEGER, share_views INTEGER, mcp_calls INTEGER, ai_credits INTEGER, PRIMARY KEY (user_id, month));
CREATE TABLE google_tokens (user_id TEXT PRIMARY KEY, refresh_token_enc BLOB, scopes TEXT, updated_at INTEGER);
CREATE TABLE abuse_reports (id TEXT PRIMARY KEY, share_id TEXT, reason TEXT, details TEXT, ip_hash TEXT, status TEXT, created_at INTEGER);
CREATE TABLE audit_log (id INTEGER PRIMARY KEY, user_id TEXT, action TEXT, target TEXT, at INTEGER);
```

- **R2 keys:** `u/<user>/l/<lesson>/<version>.rpk`,
  `u/<user>/l/<lesson>/thumb.webp`, `cache/drive/<file_id>/<etag>.rpk`
  (lifecycle rule: delete after 1 day).
- **KV:** `OAUTH_KV` (grants and tokens, hashed by the library).
- **Durable Objects (later):** `LiveRoom` keyed by a session id. SQLite-backed
  storage holds the session's update log until it's folded into a lesson
  version.
- Class spaces (later) add `classes`, `class_members` (pseudonymous student
  entries; see [Privacy](#privacy-students-and-minors)) and `class_lessons`.

### Plans (proposal; prices are open questions)

| | Free | Pro | School |
|---|---|---|---|
| App, every feature | ✓ | ✓ | ✓ |
| Cloud storage | 500 MB | 20 GB | 20 GB per teacher, pooled |
| Share links | ✓ (100 active) | unlimited | unlimited |
| Claude connector | ✓ | ✓ | ✓ |
| Google Drive "Open with → Rupok" | ✓ | ✓ | ✓ |
| Live class | none | ✓ (up to 60 students) | ✓ |
| Rupok AI credits | none | monthly allowance | per seat |
| Admin, classes, shared library | none | none | ✓ |

**Billing notes.**

- Paddle charges **5% + $0.50 per transaction**, and products under $10 need
  "bespoke pricing" ([Paddle pricing](https://www.paddle.com/pricing)). A
  $3/month plan would lose about 22% to fees, so sell Pro **yearly**
  (for example $24/year, fee about 7%), and School yearly per seat.
- Paddle sells to Bangladesh in USD; BDT and bKash aren't supported
  ([currencies](https://developer.paddle.com/concepts/sell/supported-currencies),
  [payment methods](https://developer.paddle.com/concepts/payment-methods/overview)).
- Payouts go by wire or Payoneer. Whether Paddle accepts a Bangladesh-based
  seller isn't stated either way (open question).
- bKash (later): Tokenized Checkout for one-off payments. Recurring billing
  uses bKash's **Direct Charge** subscription API, which isn't public: you get
  it through a bKash account manager ([developer docs](https://developer.bka.sh/)).
  The merchant fee depends on the agreement; third-party figures put it around
  1.5–1.85%, but get a written rate card.
- Webhooks (`/api/billing/paddle`, signature-verified) set `users.plan` and
  `subscriptions`. A missed webhook is reconciled daily by a Cron Trigger.

## Cost model

Cloudflare prices as of 23 Nov 2026, from the official pricing pages
([Workers](https://developers.cloudflare.com/workers/platform/pricing/),
[D1](https://developers.cloudflare.com/d1/platform/pricing/),
[R2](https://developers.cloudflare.com/r2/pricing/),
[KV](https://developers.cloudflare.com/kv/platform/pricing/),
[Durable Objects](https://developers.cloudflare.com/durable-objects/platform/pricing/),
[Workers AI](https://developers.cloudflare.com/workers-ai/platform/pricing/)):

| Service | Included in Workers Paid ($5/month) | Beyond that |
|---|---|---|
| Workers requests | 10 M / month | $0.30 per M |
| Workers CPU | 30 M CPU-ms / month | $0.02 per M CPU-ms |
| Static asset requests | free, unlimited | — |
| D1 | 25 B rows read, 50 M rows written, 5 GB | $0.001/M read, $1.00/M written, $0.75/GB-month |
| R2 (free tier, every account) | 10 GB-month, 1 M Class A, 10 M Class B | $0.015/GB-month, $4.50/M Class A, $0.36/M Class B; **egress free** |
| KV | 10 M reads, 1 M writes, 1 GB | $0.50/M reads, $5.00/M writes |
| Durable Objects | 1 M requests, 400,000 GB-s | $0.15/M requests, $12.50/M GB-s; WebSocket messages in are billed 20:1 |
| Workers AI | 10,000 Neurons/day | $0.011 per 1,000 Neurons |

### A typical free teacher, per month (assumptions)

| Item | Assumption | Monthly use |
|---|---|---|
| Storage | 60 MB stored (12% of the 500 MB quota) | 0.06 GB-month |
| Uploads | 20 lessons, some re-uploaded | 40 Class A ops, 200 D1 rows written |
| Student opens | 30 students × 20 lessons | 600 opens = 1,200 Worker requests + 600 Class B ops |
| App API calls | list, upload, share | 300 Worker requests |
| CPU | ~2 ms per request, ~50 ms per upload validation in wasm | ~5,000 CPU-ms |
| D1 reads | ~10 rows per request | ~15,000 rows read |

### How far $5/month goes

Teachers covered by each included allowance:

| Allowance | Covers typical teachers |
|---|---|
| Workers requests (10 M ÷ 1,500) | **~6,600** |
| Workers CPU (30 M ÷ 5,000) | **~6,000** |
| D1 rows written (50 M ÷ 200) | 250,000 |
| D1 rows read (25 B ÷ 15,000) | 1.6 million |
| R2 Class A (1 M ÷ 40) / Class B (10 M ÷ 600) | 25,000 / 16,600 |
| **R2 storage (10 GB ÷ 60 MB)** | **~170** (20 if everyone fills 500 MB) |

So:

- **Compute isn't the limit.** The $5 plan's compute covers about **6,000
  typical free teachers**.
- **Storage is the first real cost**, and it's small: $0.015/GB-month is
  **about $0.90 per 1,000 typical teachers per month**, or $7.50 per 1,000 if
  every teacher fills the full 500 MB.
- **All-in marginal cost** (storage, requests, CPU and operations) beyond the
  included amounts is about **$0.002 per typical teacher per month, roughly $2
  per 1,000 teachers.**

Estimated monthly bill:

| Active free teachers | Storage | Overage | **Total / month** |
|---|---|---|---|
| 100 | 6 GB (inside the free 10 GB) | — | **$5.00** |
| 1,000 | 60 GB → 50 GB × $0.015 = $0.75 | — | **$5.75** |
| 6,000 | 360 GB → $5.25 | requests and CPU just inside | **$10.25** |
| 10,000 | 600 GB → $8.85 | requests 5 M × $0.30 = $1.50; CPU 20 M × $0.02 = $0.40 | **$15.75** |
| 50,000 | 3 TB → $44.85 | requests $19.50; CPU $4.40; Class A $4.50; Class B $7.20 | **≈ $85** |
| 1,000 at full quota (worst case) | 500 GB → $7.35 | — | **$12.35** |

**Where costs start:** past ~170 typical teachers (R2's free 10 GB), at about a
cent per teacher-year. Past ~6,000 teachers, Workers requests and CPU follow.
**A viral share link is cheap:** 100,000 student opens is about 200,000
requests ($0.06) plus 100,000 Class B operations ($0.04), with no bandwidth
bill because R2 egress is free.

**Live classes (Pro, later):** a 45-minute class with 40 students on a
hibernating Durable Object is about 337 GB-s (128 MB × 2,700 s) plus about
2,700 billed requests (teacher strokes in at 20:1; messages out to students
are free). The included 400,000 GB-s covers about 1,100 class sessions a month;
after that it's about $0.004 per class.

**Billing fees dominate small payments,** not hosting: on a $24/year Pro plan,
Paddle takes about $1.70. Even a Pro teacher who fills all 20 GB costs $3.60 a
year in R2 storage; a typical one (about 1 GB) costs about $0.18.

**Other notes:**

- **Watch:** Workers Logs moves to Observability pricing on 1 Dec 2026, with
  50 GB of ingestion included on Paid
  ([pricing](https://developers.cloudflare.com/observability/pricing/)). Keep
  log volume low: no per-request logging of share views.
- **Not covered here:** AI credits. Price them from the chosen model's
  per-token rates at launch, then cap them per plan. Workers AI ($0.011 per
  1,000 Neurons) suits small helper tasks. Lesson-quality Smart Objects need
  a frontier model, and Pro pays for that.

## Abuse and limits

| Risk | Limit / control |
|---|---|
| Free file hosting disguised as lessons | Every upload is validated in wasm (`Package::read`: ZIP, `mimetype` first, `doc.loro` loads, reader limits). Assets must be images or Smart Object bundles. 50 MB per lesson, 500 MB per free account. |
| Upload floods | 30 uploads/hour and 200/day per account (Workers rate limiting binding); new accounts start at 100 MB until 24 hours old. |
| Scraping share links | 10-character random ids; no listing endpoint; per-IP rate limit on `/s/*` and `/api/s/*` (60/min). |
| Phishing or malware inside Smart Objects | Objects already run in `sandbox="allow-scripts"` frames with a CSP that blocks the network. On share links, **objects that declare `permissions.network` are disabled for free accounts** and shown with a warning for paid ones. Never relax the sandbox (AGENTS.md rule 5). |
| Illegal or harmful content | "Report" link on every share page → `abuse_reports`; revoke-and-review within 24 hours; account suspension. Images in lessons can be scanned with Cloudflare's CSAM Scanning Tool when served through the zone. Terms of service and a takedown contact before launch. |
| Account farming for storage | One Google account = one free plan. Flag many accounts from one IP or device fingerprint for review; no automatic bans. |
| Cost runaway | A daily Cron Trigger sums usage; alerts at 2× the expected bill; a kill switch (an environment variable) that serves share views from cached copies only. |
| MCP misuse | Per-token rate limit (120 calls/hour), scope checks per tool, an audit log of write tools. |

## Privacy: students and minors

- **Students don't sign in, and we collect nothing about them.** Share pages
  set no cookies and run no analytics. View counts use a daily salted IP hash
  that is never stored raw and is rotated daily. That keeps share viewing
  outside "collecting personal information from children".
- **Applicable law:**
  - **COPPA (US, under 13):** the amended rule's compliance date was
    22 April 2026. The FTC didn't codify a school-authorization exception;
    schools may consent for parents only for school use with no other
    commercial purpose
    ([Federal Register](https://www.govinfo.gov/content/pkg/FR-2025-04-22/html/2025-05904.htm),
    [FTC FAQ](https://www.ftc.gov/business-guidance/resources/complying-coppa-frequently-asked-questions)).
  - **GDPR Art. 8:** consent age 16 by default; member states may lower it
    to 13.
  - **Bangladesh, Personal Data Protection Act 2026 (Act 63 of 2026):** a
    "child" is **under 18**; processing a child's data needs parent or
    guardian consent as regulations prescribe; it applies extraterritorially
    ([official text](https://bdlaws.minlaw.gov.bd/act-1692.html)).
- **Design rules that follow:**
  1. No student accounts until class spaces. When they come, they're created
     by a school (School plan, with a data processing agreement and
     school-given consent), hold a display name only (no email or birthday),
     and are deletable by the teacher and the school.
  2. Teachers are adults with Google accounts. We store the email, name and
     avatar only.
  3. Lessons belong to the teacher. Export is always available (download all
     as `.rpk` files), and deletion is real: R2 objects and D1 rows go within
     30 days, backups within 60.
  4. Data location: R2 bucket with the APAC location hint (closest to
     Bangladesh). EU schools later get an EU-jurisdiction bucket.
  5. Google tokens (Drive "Open with") are encrypted at rest with a key in a Workers
     secret; scopes are minimal (`drive.file`, `drive.install`).
  6. A plain-language privacy policy in Bangla and English before launch.

## The open-core boundary

| Open (in this repo) | License | Closed (private `rupok-cloud` repo) |
|---|---|---|
| The app, the viewer, the share-page client, upload/share UI | AGPL-3.0 | The Worker: accounts, quotas, billing (Paddle, bKash), abuse tooling, admin dashboard |
| `.rpk` format spec, `rupok-format` (the validator the Worker reuses), Smart Object SDK, skills | Apache-2.0 | Plan logic and pricing, fraud and abuse heuristics |
| **Rupok Cloud API spec** (`spec/cloud-api.md`, new): endpoints, share-link format, MCP tool list | Apache-2.0 | Operational config, secrets, monitoring |
| The local MCP server (`rupok mcp`) and the self-hostable live server ([live-broadcast](./live-broadcast.md)) | AGPL-3.0 | The hosted `LiveRoom` Durable Object |

Why this split works:

- Shaitrish holds the copyright in the app (contributors sign the CLA), so
  the closed Worker can reuse the AGPL engine code.
- The open API spec keeps the promise in COPYING.md that teachers are never
  locked in: anyone can build a compatible share server or client.
- The app must keep working with **no cloud at all**, so cloud features
  appear only after sign-in.

## Phases (rough sizes)

Sizes for one experienced developer: S ≈ under a week, M ≈ 2–3 weeks, L ≈ 4–6
weeks.

| Phase | Contents | Size | Depends on |
|---|---|---|---|
| **1. Share target and PWA** | Service worker, `share_target`, real-device WhatsApp test, offline shell (with [web-viewer](./web-viewer.md)) | S | — |
| **2. Accounts and share links** | Worker routes, Google sign-in, D1 schema, R2 upload with wasm validation, 500 MB quota, `/s/<id>` with OG previews, report link, privacy policy, terms | M–L | 1 (optional) |
| **3. Claude connector** | `/mcp` with `createMcpHandler`, `workers-oauth-provider` (CIMD + DCR), the 11 tools plus share tools, discovery endpoints, connector docs | M | 2 |
| **4. Pro and Paddle** | Plans, Paddle checkout and webhooks, usage page, quota enforcement, the yearly Pro plan | M | 2 |
| **5. Google Drive "Open with"** | "Open with → Rupok" (`drive.install` + `drive.file`), Marketplace listing and Google review | M (+ review time) | 2 |
| **6. School and class spaces** | Orgs, seats, admin, classes, pseudonymous rosters, DPA template | L | 4 |
| **7. bKash** | Tokenized Checkout, Direct Charge subscriptions (needs a bKash merchant agreement) | M | 4 |
| **8. Live classes** | `LiveRoom` Durable Object with hibernation; students follow the board from a link; the session becomes a lesson version | L | 2, [live-broadcast](./live-broadcast.md) |

## Implementation tasks (phase 2, PR-sized)

1. **Spike: Worker + static assets + `run_worker_first`**, confirm static
   requests stay unbilled. *(good-first-issue)*
2. **D1 schema and migrations** (`wrangler d1 migrations`). *(intermediate)*
3. **Google sign-in and sessions** (GIS button, JWT verification, cookie).
   *(intermediate)*
4. **`rupok-format` validation in the Worker** (wasm build, limits, error
   messages). *(advanced)*
5. **Upload, versions, quota** endpoints, plus the app's Upload button.
   *(intermediate)*
6. **Share links**: `/s/<id>` shell with OG tags, `/api/s/<id>/lesson`
   streaming, revoke and expiry. *(intermediate)*
7. **Viewer integration**: the board loads a share URL in Student mode.
   *(intermediate)*
8. **Abuse basics**: rate limits, report flow, admin revoke. *(intermediate)*
9. **`spec/cloud-api.md`** (an RFC, since it's a new spec). *(intermediate)*

## Acceptance criteria

- A share link opened on a 2 GB Android phone over 4G shows page 1 within
  3 seconds for a 5 MB lesson, with replay and Smart Objects working.
- Uploading a non-lesson ZIP, a ZIP with a path-traversal entry, or a 60 MB
  file is rejected with a clear message.
- Revoking a share makes `/s/<id>` return 404 within a minute (cache max-age
  300 s).
- The app with no network and no sign-in behaves exactly as today.
- A monthly cost report for the first 1,000 teachers stays under $7.

## Open questions

- **Prices:** Pro at $24/year (Paddle) and about ৳300/month (bKash)? School
  per-seat price and minimum seats?
- Does Paddle accept a Bangladesh-registered seller, or is a foreign entity
  needed? Confirm before building phase 4.
- Free-plan share limits: is "100 active links" needed, or just the storage
  quota?
- **Drive "Open with" for students without Google accounts:** is a public-link
  fetch (API key, files shared "anyone with the link") acceptable as a
  fallback?
- Should free-plan share links show a small "Made with Rupok" footer (growth)
  or nothing (trust)?
- Do share pages need a QR code for projecting in class? (Cheap: render in the
  client.)
- Where do AI credits come from (model and provider), and what is a credit
  worth?

## References

- `apps/board/src/pending.ts`: the IndexedDB hand-off used by `/open/`, reused
  by Share Target and Drive "Open with"
- `apps/board/wrangler.jsonc`, `apps/board/public/manifest.webmanifest`
- `crates/rupok-format` (validation), `crates/rupok-cli/src/ops.rs` and
  `tools.rs` (the MCP tools to mirror)
- Related plans: [google-drive-sync](./google-drive-sync.md),
  [web-viewer](./web-viewer.md), [live-broadcast](./live-broadcast.md),
  [course-library](./course-library.md), [student-layers](./student-layers.md)
- Research sources: all prices and policies above link to the vendor's page,
  as checked on 23 Nov 2026.
