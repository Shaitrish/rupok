# আরপিকে (RPK) ফাইল ফরম্যাট

> RPK (Rich Presentation Kit) ফাইল হলো Rupok-এর একটি লেসন: স্লাইড, হাতের লেখা, লাইভ অ্যাক্টিভিটি আর ক্লাসের রিপ্লে, একটি খোলা ZIP প্যাকেজে। ভেতরে কী আছে, কীভাবে খুলবেন, PDF বানাবেন আর এআই দিয়ে পড়বেন।

Canonical: https://rupok.app/bn/rpk/ · Language: bn · Updated: 2026-11-23

**RPK ফাইল** (`.rpk`, পুরো নাম **Rich Presentation Kit**) হলো Rupok দিয়ে বানানো একটি লেসন। Rupok শিক্ষকদের জন্য ফ্রি, ওপেন সোর্স হোয়াইটবোর্ড। একটি ফাইলেই থাকে স্লাইড, বোর্ডের সব লেখা, লাইভ অ্যাক্টিভিটি আর পুরো ক্লাসের রিপ্লে। ফরম্যাটটি খোলা ও নথিভুক্ত; মানুষ, প্রোগ্রাম আর এআই, সবাই পড়তে পারে।

## RPK ফাইল কী?

RPK মানে **Rich Presentation Kit**। শিক্ষকরা [Rupok](https://rupok.app/board/)-এ .rpk লেসন বানান আর হোয়াটসঅ্যাপ, জিমেইল বা গুগল ড্রাইভে শিক্ষার্থীদের পাঠান। PDF-এর মতো স্থির নয়, লেসনটি জীবন্ত থাকে:

- **পাতা:** স্লাইড (প্রায়ই PDF থেকে) বা অসীম বোর্ড।
- **হাতের লেখা:** প্রতিটি টান, সময়সহ, তাই ক্লাসের রিপ্লে দেখা যায়।
- **স্মার্ট অবজেক্ট:** সিমুলেশন, থ্রিডি মডেল আর কুইজ, যা এখনো চলে।
- **পড়ার উপযোগী সারাংশ:** লেসনটি Markdown আর JSON-এ, সার্চ আর এআই-এর জন্য।

- **পুরো নাম**: Rich Presentation Kit (Rupok লেসন)
- **এক্সটেনশন**: `.rpk`
- **মিডিয়া টাইপ**: `application/vnd.rupok.lesson+zip`
- **ভেতরে**: একটি ZIP প্যাকেজ
- **যা দিয়ে খোলে**: Rupok, Chrome, Edge বা Safari-তে ফ্রি ([rupok.app/bn/open](https://rupok.app/bn/open/))
- **স্পেসিফিকেশন**: খোলা, Apache-2.0 ([GitHub-এ](https://github.com/Shaitrish/rupok/blob/main/spec/rupok-format.md))

## .rpk ফাইলের ভেতরে কী থাকে

.rpk আসলে একটি ZIP ফাইল। একটি কপির নাম বদলে `.zip` করলেই ভেতরটা দেখা যায়:

```text
lesson.rpk                 a ZIP archive
├── mimetype               application/vnd.rupok.lesson+zip (first entry)
├── manifest.json          title, dates, course and chapter, page count
├── README.md              "This is a Rupok lesson…" + the outline
├── content.md             the lesson as readable Markdown
├── content.json           the same outline, structured
├── doc.loro               the lesson itself: pages, ink, history (Loro CRDT)
├── assets/<hash>.png      images and media
├── objects/<id>/index.html  interactive Smart Objects (HTML)
└── thumbs/<n>.png         page previews (optional)
```

`doc.loro` হলো মূল লেসন: একটি [Loro](https://loro.dev) CRDT ডকুমেন্ট, যাতে সম্পাদনার পুরো ইতিহাস থাকে। `README.md`, `content.md` আর `content.json` হলো সহজে পড়ার মতো সারাংশ, যা Rupok প্রতিবার সেভ করার সময় নতুন করে লেখে।

## RPK ফাইল কীভাবে খুলবেন

1. **rupok.app/bn/open খুলুন,** মোবাইল বা কম্পিউটারের Chrome, Edge বা Safari ব্রাউজারে।
2. **.rpk ফাইলটি পাতায় ছেড়ে দিন,** অথবা “.rpk ফাইল বেছে নিন”-এ চাপুন। স্টুডেন্ট মোডে খুলবে।
3. **ক্লাসের রিপ্লে দেখুন,** পাতা উল্টান, লাইভ অ্যাক্টিভিটি চালান।

[আরপিকে ভিউয়ার খুলুন](https://rupok.app/bn/open/)

## RPK থেকে PDF বানাবেন কীভাবে

1. **ফাইলটি খুলুন,** [rupok.app/bn/open](https://rupok.app/bn/open/)-এ।
2. **Download PDF চাপুন।** শিক্ষকের লেখাসহ সব পাতা PDF হয়ে যাবে।
3. **প্রিন্ট বা শেয়ার করুন।** PDF আপনার ব্রাউজারেই তৈরি হয়, কিছুই আপলোড হয় না।

## এআই দিয়ে RPK ফাইল পড়া

প্রতিটি .rpk নিজের পরিচয় নিজেই দেয়। ChatGPT বা Claude-কে ফাইলটি দিন, সে বুঝে নেবে লেসনে কী আছে।

1. **.rpk ফাইলটি আপলোড করুন** আর লেসন নিয়ে প্রশ্ন করুন।
2. **এআই ফাইলটি আনজিপ করে** `README.md` আর `content.md` পড়ে: পাতাভিত্তিক শিরোনাম, লেখা, গণিত, চিত্র, ছবি আর অ্যাক্টিভিটি।
3. **ডেভেলপার আর এজেন্টদের জন্য** [read-rpk স্কিল](https://rupok.app/skills/read-rpk.md) আর [ইংরেজি পাতা](https://rupok.app/rpk/)।

## আরপিকে ফাইল নিয়ে প্রশ্নোত্তর

### RPK-এর পুরো নাম কী?

RPK মানে Rich Presentation Kit। .rpk ফাইল হলো Rupok দিয়ে বানানো লেসন; Rupok শিক্ষকদের জন্য ফ্রি, ওপেন সোর্স হোয়াইটবোর্ড।

### RPK ফাইল কীভাবে খুলবো?

Chrome, Edge বা Safari-তে [rupok.app/bn/open](https://rupok.app/bn/open/) খুলে ফাইলটি ছেড়ে দিন। ফ্রি, অ্যাকাউন্ট লাগে না, কিছু আপলোড হয় না।

### RPK ফাইলকে PDF বানাবো কীভাবে?

ফাইলটি [rupok.app/bn/open](https://rupok.app/bn/open/)-এ খুলে Download PDF চাপুন।

### ChatGPT বা Claude কি RPK ফাইল পড়তে পারে?

হ্যাঁ। Rupok-এর নতুন সংস্করণে বানানো প্রতিটি .rpk-এ `README.md`, `content.md` আর `content.json` থাকে, অর্থাৎ পুরো লেসনের সহজপাঠ্য সারাংশ। ফাইলটি আপলোড করে প্রশ্ন করুন।

### RPK ফরম্যাট কি খোলা?

হ্যাঁ। স্পেসিফিকেশন আর রেফারেন্স রিডার-রাইটার Apache-2.0 লাইসেন্সে, তাই যে কেউ .rpk পড়ার বা লেখার টুল বানাতে পারে। [GitHub-এ স্পেসিফিকেশন](https://github.com/Shaitrish/rupok/blob/main/spec/rupok-format.md)।
