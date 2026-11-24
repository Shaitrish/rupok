// Privacy policy (/privacy/) and terms of service (/terms/). Google's OAuth
// consent screen links to both, so they must exist before Rupok Cloud's
// Google sign-in is published. Keep them in plain language and in step with
// what the app and Rupok Cloud actually do (see the rupok-cloud README,
// "Privacy").

import { GITHUB } from "../lib.mjs";

export const CONTACT = "privacy@rupok.app";
const UPDATED_ON = "24 November 2026";

const privacy = {
  path: "/privacy/",
  lang: "en",
  title: "Privacy Policy | Rupok",
  ogTitle: "Rupok privacy policy",
  description:
    "How Rupok handles your data: the app works without an account, has no ads, analytics or tracking cookies, and Rupok Cloud stores only what it needs to sync and share your lessons.",
  crumbs: [["Privacy"]],
  tag: "Privacy",
  h1: "Privacy policy",
  lede: `Rupok is a free, open-source whiteboard for teachers, made by Shaitrish in Bangladesh. You can use it without an account. We don't show ads, we don't use analytics, and we don't use tracking cookies. Last updated ${UPDATED_ON}.`,
  sections: [
    {
      id: "summary",
      h2: "The short version",
      blocks: [
        {
          ul: [
            "**No account needed.** The board, the desktop app and the lesson library work without signing in. Your lessons stay on your device.",
            "**No ads, no analytics, no tracking cookies.** We don't sell or share personal data.",
            "**Rupok Cloud is optional.** If you sign in with Google, we store your name, email, profile picture link and Google account ID, plus the lessons you choose to sync or share.",
            "**You stay in control.** You can export everything, or delete your account and lessons at any time.",
          ],
        },
      ],
    },
    {
      id: "device",
      h2: "Data on your device",
      blocks: [
        "Lessons you create are saved on your computer or in your browser's own storage. Settings (theme, pen, gestures and so on) are kept in the browser or app. None of this is sent to us unless you use Rupok Cloud.",
      ],
    },
    {
      id: "cloud",
      h2: "Rupok Cloud (when you sign in)",
      blocks: [
        "Rupok Cloud syncs your lessons between devices and creates share links for students. When you sign in with Google we receive and store:",
        {
          ul: [
            "your **email address**, **name**, **profile picture link** and **Google account ID**, to identify your account;",
            "the **lessons, folders and files** you upload or sync, and their edit history, so they can sync between your devices;",
            "the **share links** you create and who has access to your shared folders;",
            "a **session**, kept in a strictly necessary cookie (or a token in the desktop app), so you stay signed in.",
          ],
        },
        "We ask Google only for basic profile information (`openid`, email and profile). We can't see your Gmail, Drive or anything else in your Google account.",
        "We don't store IP addresses or browser details in our database. Our hosting provider, Cloudflare, processes requests to deliver the service and keeps short-lived operational logs under its own privacy policy.",
      ],
    },
    {
      id: "students",
      h2: "Students",
      blocks: [
        "Students can open a lesson their teacher shared without an account. Viewing a share link sets no cookie and creates no record about the student.",
        "Rupok Cloud accounts are meant for teachers and for students who are old enough to sign in under the rules where they live, or who have permission from their school or a parent.",
      ],
    },
    {
      id: "ai",
      h2: "AI, voice and videos",
      blocks: [
        {
          ul: [
            "**Your own AI.** Rupok's AI features connect to an AI service you choose (for example your own Claude, Codex or OpenCode). What you send is handled by that provider under its terms. Rupok doesn't receive it.",
            "**Handwriting to text with your own Claude API key** sends the selected ink as a picture directly from your device to Anthropic. The key stays on your device.",
            "**Voice input.** In Chrome and Edge, speech is sent to Google or Microsoft for recognition. The optional offline voice model in the desktop app keeps audio on your computer. Rupok never records or stores audio.",
            "**YouTube and web embeds.** A YouTube video uses YouTube's privacy-enhanced player and connects to YouTube only when you press play. Embedded sites (such as PhET or GeoGebra) are loaded from those sites.",
          ],
        },
      ],
    },
    {
      id: "rights",
      h2: "Your choices and rights",
      blocks: [
        {
          ul: [
            "**Export:** download all your Rupok Cloud data, including every lesson as a `.rpk` file.",
            "**Delete:** deleting your account removes your profile, lessons, folders, history and share links straight away. Files nobody else uses are erased within about a day, and encrypted backups age out within 30 days.",
            "**Correct:** your name and picture come from your Google account and update when you sign in again.",
          ],
        },
        `To ask about your data, write to ${CONTACT}.`,
      ],
    },
    {
      id: "security",
      h2: "Security",
      blocks: [
        "All connections use HTTPS. Session tokens are stored hashed. Lessons are stored with Cloudflare (D1 and R2). Smart Objects in lessons run in a locked-down sandbox with no network access.",
        `Rupok is open source, so anyone can check how it works: [${GITHUB.replace("https://", "")}](${GITHUB}). To report a security problem, see the project's security policy on GitHub.`,
      ],
    },
    {
      id: "changes",
      h2: "Changes and contact",
      blocks: [
        `If we change this policy we'll update this page and the date at the top. Questions: ${CONTACT}.`,
      ],
    },
  ],
  faq: [],
};

const terms = {
  path: "/terms/",
  lang: "en",
  title: "Terms of Service | Rupok",
  ogTitle: "Rupok terms of service",
  description: "The terms for using the Rupok app and the optional Rupok Cloud service.",
  crumbs: [["Terms"]],
  tag: "Terms",
  h1: "Terms of service",
  lede: `These terms cover the Rupok app and the optional Rupok Cloud service, provided by Shaitrish in Bangladesh. Last updated ${UPDATED_ON}.`,
  sections: [
    {
      id: "software",
      h2: "The Rupok app",
      blocks: [
        `The Rupok app is free, open-source software under the [GNU AGPL-3.0 licence](${GITHUB}/blob/main/LICENSE). The \`.rpk\` file format, the Smart Object SDK and the AI skills are Apache-2.0. Those licences, not these terms, govern your use of the source code.`,
      ],
    },
    {
      id: "cloud",
      h2: "Rupok Cloud",
      blocks: [
        "Rupok Cloud is an optional online service for syncing and sharing lessons. You sign in with a Google account. The free plan includes 500 MB of storage. Rupok is in early alpha, so features and limits may change.",
      ],
    },
    {
      id: "content",
      h2: "Your lessons",
      blocks: [
        "You own the lessons and files you create. By using Rupok Cloud you let us store, copy and serve them only as needed to run the service, for example to sync your devices and to show a lesson to the people you share it with.",
        "When you share a link, anyone with that link can open the lesson with the access you chose. You can turn a link off at any time.",
      ],
    },
    {
      id: "use",
      h2: "Acceptable use",
      blocks: [
        "Don't use Rupok Cloud to:",
        {
          ul: [
            "store or share content that is illegal, or that infringes someone else's rights;",
            "harass or harm others, or share personal data about students without a proper reason and permission;",
            "upload malware, or try to break, overload or get around the limits or security of the service;",
            "use the service to send spam or run another business's file hosting.",
          ],
        },
        "We may remove content or suspend an account that breaks these rules, and anyone can report a shared lesson.",
      ],
    },
    {
      id: "warranty",
      h2: "No warranty",
      blocks: [
        "Rupok and Rupok Cloud are provided \"as is\", without warranties of any kind. Keep your own copies of important lessons: you can export them as `.rpk` files at any time. To the extent the law allows, Shaitrish is not liable for lost data or indirect damages arising from using Rupok.",
      ],
    },
    {
      id: "end",
      h2: "Ending, changes and law",
      blocks: [
        "You can stop using Rupok Cloud and delete your account at any time. We may change or stop the service; if we do, we'll give reasonable notice where we can so you can export your lessons.",
        "We may update these terms and will change the date at the top when we do. These terms are governed by the laws of Bangladesh.",
        `Questions: ${CONTACT}.`,
      ],
    },
  ],
  faq: [],
};

export const LEGAL = [privacy, terms];
