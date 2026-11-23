// Settings → General → Rupok Cloud: the account, space used, and where new
// lessons are saved. Everything cloud is optional; Rupok works without it.

import { registerSettings } from "../registry";
import type { SettingProps } from "../registry";
import { CloudAccount, CloudUsage } from "../../components/CloudAccount";
import { cloudUser } from "../../cloud/account";
import { libPrefs, setLibPrefs } from "../../library/prefs";

function Account(props: SettingProps) {
  return <div class="st-embed" role="group" aria-labelledby={props.labelId}><CloudAccount /></div>;
}
function Usage(props: SettingProps) {
  return <div class="st-embed" role="group" aria-labelledby={props.labelId}><CloudUsage /></div>;
}

registerSettings([
  {
    id: "general.cloudAccount", category: "general", group: "Rupok Cloud", order: 200, type: "custom", layout: "block",
    label: "Account", description: "Sign in to sync lessons between devices and share them with a link. 500 MB free.",
    keywords: ["cloud", "sign in", "sign out", "login", "logout", "google", "account", "sync", "ক্লাউড", "সাইন ইন"],
    component: Account,
  },
  {
    id: "general.cloudUsage", category: "general", group: "Rupok Cloud", order: 210, type: "custom", layout: "block",
    label: "Space used", description: "Only lessons count. The trash counts until it is emptied (30 days).",
    keywords: ["quota", "storage", "space", "usage", "500 MB"],
    when: () => !!cloudUser(),
    component: Usage,
  },
  {
    id: "general.saveLocation", category: "general", group: "Rupok Cloud", order: 220, type: "segmented",
    label: "Save new lessons to", description: "Where a lesson goes when you start drawing on a fresh board.",
    keywords: ["default save location", "cloud", "local", "device"],
    options: [{ value: "local", label: "This device" }, { value: "cloud", label: "Rupok Cloud" }],
    enabled: () => !!cloudUser(),
    get: () => libPrefs().saveTo,
    set: (v) => setLibPrefs({ saveTo: v === "cloud" ? "cloud" : "local" }),
    default: "local",
  },
]);
