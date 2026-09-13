# Domain Names — A Mental Model

*Written from the root down. The aim is that nothing about the top-level namespace should feel arbitrary by the end — not why `.rodeo` exists, not why `.rocher` stopped existing, not why there is no two-letter alternative to `.com`, and not why a registrar's search page for the letter R is so much shorter than the internet's own list. Part I builds the model. Part II is the case study that started it. Part III is what to do with it. Part IV steps outside the root entirely, to the namespaces that resolve without ICANN's permission. Figures throughout are live from IANA's root zone file, version 2026090601, retrieved 7 September 2026.*

---

# Part I — The model, from the root down

## 0. The one-sentence model

> **The top-level namespace is not a market. It is a licensed registry of strings, allocated in discrete rounds by a single non-profit, under rules written to prevent three specific failures: collision, confusion, and capture.**

Every strange thing you notice follows from a clause of that sentence:

- **Licensed** → you don't buy a TLD, you sign a contract to operate one, with obligations that outlast your interest in it. Hence the retirements.
- **Registry of strings** → the unit of allocation is a *word*, and words were claimed in bulk by whoever could afford to. Hence the dictionary-shaped portfolios.
- **Discrete rounds** → supply is not continuous. One round in 2012, the next in 2026. Hence fourteen years of a namespace frozen at the moment of one particular land grab.
- **Collision, confusion, capture** → the three fears generate almost every rule in the guidebook, and the rules generate the house style.

Hold onto that. The rest is unpacking.

## 1. The root is a file

Start with what the top level physically *is*. It is a zone file — a text file, a few megabytes, listing every TLD and the nameservers authoritative for it. It is edited by IANA (a function of ICANN), signed with DNSSEC, and published to the thirteen root server letters, which are operated by twelve independent organisations across a thousand-plus anycast instances.

Two consequences fall straight out of this, and both matter more than they look:

**Adding a row is cheap; the *consequences* of a row are not.** The technical act of delegating a TLD costs essentially nothing. What costs is everything downstream: someone must run the registry reliably forever, resolvers worldwide will cache it, existing software will encounter a string it has never seen, and every trademark holder on earth now has a new surface to police. ICANN's entire fee structure and evaluation apparatus exists to price in that externality, not the row.

**The root is a single point of governance.** There is exactly one authoritative list, and one organisation editing it. This is the internet's most centralised component, and it is the reason the namespace has *policy* at all — rules, rounds, objections, guidebooks — rather than being a free market in strings. Every oddity you're about to see is a policy artefact.

Today the file holds **1,438 live TLDs**.

## 2. The four kinds of TLD, and the two ways they come into being

| Type | Live count | Origin |
|---|---:|---|
| Country-code (ccTLD) | 248 two-letter | **Derived** from ISO 3166-1 alpha-2 |
| Internationalised (IDN) | 151 `xn--` | Non-Latin scripts, stored as Punycode |
| Generic (gTLD) | ~1,021 ASCII | **Applied for**, in a round |
| Sponsored | 14 | Pre-2012 community charters |
| Infrastructure | 1 | `.arpa`, reverse DNS plumbing |

The important split is not the taxonomy — it's the **two allocation mechanisms**, because they explain why superficially similar strings behave completely differently.

**ccTLDs are derived, not granted.** Jon Postel's original decision was to avoid adjudicating nationhood by outsourcing it: whatever appears on the ISO 3166-1 list gets a TLD. This is why `.rw` exists (Rwanda), why `.re` exists (Réunion, a French overseas department), and why `.su` still resolves decades after the Soviet Union stopped existing. It's a mechanical mapping, and its mechanical nature is the point — it removed a political judgement from the internet's critical path.

The catch: **ICANN has almost no authority over ccTLDs.** Each is administered by whoever the local government or community designates, under local law. They set their own eligibility rules, their own prices, and their own politics, and they can change all three unilaterally. A ccTLD is not a product you're buying; it's a jurisdiction you're entering.

**gTLDs are applied for, evaluated, and contracted.** A completely different machine: an application, a fee, a technical and financial evaluation, an objection window, a contention resolution process, and then a Registry Agreement with ICANN carrying permanent obligations. Everything weird in the generic space is downstream of the fact that this machine has only been switched on twice.

> **Mental model to keep:** ccTLD = a jurisdiction with a DNS entry. gTLD = a licence with an expiry-less contract. When a TLD surprises you, ask which machine produced it — the answer is usually the whole explanation.

## 3. 2012: the one-off auction of the dictionary

The modern namespace is the fossil record of a single event.

In 2012 ICANN opened the New gTLD Program. The evaluation fee was **$185,000, non-refundable**, before registry infrastructure, legal work, ICANN's ~$25,000 annual fixed fee, and $0.18 per domain-year in transaction fees. Roughly **1,930 applications** arrived. Around 1,200 were eventually delegated.

Three structural facts about that round explain nearly everything you see today:

**It was effectively an auction of common nouns.** At $185k a string, the applicants were not enthusiasts. They were portfolio operators making a bet on a whole vertical of language, and brands defending themselves. Where two parties wanted the same string, they entered a *contention set*, resolved by private auction — money that flowed between applicants rather than to ICANN. `.web` settled at $135 million. That is the price of a row in a text file, and it tells you exactly what kind of participant the fee structure selected for.

**It was one round, not a market.** Supply stopped in 2012 and did not resume. So the namespace you browse today is not the result of fourteen years of demand meeting supply. It is a snapshot of what a few dozen well-capitalised firms guessed would sell, in 2012, frozen.

**Roughly half of what was delegated was never for sale.** `.brand` TLDs — governed by Specification 13 of the Registry Agreement — permit exactly one registrant: the brand itself. They exist for defensive and architectural reasons, most were barely used, and a steady trickle of them are being handed back. IANA's database carries **1,595 rows against 1,438 live delegations**; the difference is largely retirement.

## 4. The rules that shape the strings

A TLD string is not free-form. The constraints below are why the namespace has a *house style*:

- **LDH only** — letters, digits, hyphen. 1–63 octets. No leading or trailing hyphen; no `--` in the third and fourth positions except for genuine Punycode.
- **Minimum three characters** for a gTLD. Two-letter strings are permanently reserved for ISO 3166 country codes, and single characters are reserved outright. *This is the single biggest reason the namespace feels inelegant:* every short, clean option is structurally unavailable. The only way to get a two-letter TLD is to be a country.
- **No country or territory names**, in any language, without government support. This is what made `.amazon` a multi-year fight with the Amazon Cooperation Treaty Organization states, and why `.rio` had to come from Rio's own municipal IT company.
- **String Similarity Review** — anything visually confusable with an existing or applied-for string is rejected, or forced into contention. Confusion is one of the three fears, and it is adjudicated pre-emptively.
- **Single-script IDNs.** No mixing Latin and Cyrillic in one label, because homograph attacks are the confusion fear in its sharpest form.
- **Name-collision reservations.** `.home`, `.corp` and `.mail` were applied for and permanently blocked, because they are already in heavy use on private internal networks. Delegating them would have broken real infrastructure — the collision fear, made concrete.
- **Objections** — legal rights, community, limited public interest, plus Governmental Advisory Committee advice — killed further applications outright.
- **Post-delegation obligations** — Trademark Clearinghouse Sunrise, a Uniform Rapid Suspension path, data escrow, RDAP, emergency registry continuity. Running a TLD is a compliance posture, not a hobby.

Read that list back and the aesthetic explains itself: three characters or more, real dictionary words, no country names, nothing that looks like anything else. The style *is* the constraint set.

## 5. Why the survivors look the way they do

Filter 1,000-odd generic strings through those rules and four archetypes come out:

**Vertical nouns, bought as a set.** `.realestate`, `.realty`, `.rentals`, `.repair`, `.report`, `.restaurant`, `.recipes`. Applied for in bulk by portfolio registries working through the dictionary of commerce. They also buy *both* grammatical numbers — `.review` and `.reviews` — so neither leaks to a rival.

**Suffix hacks.** `.rocks`, `.run`, `.rest`, `.red`, `.rip`, `.rsvp`. These are not categories; they're the second half of a phrase. Priced low, marketed on cleverness, monetised on renewal.

**Brands.** `.ricoh`, `.rogers`, `.rwe`, `.reliance`, `.ril`, `.rexroth` (Bosch), `.redumbrella` (Travelers Insurance), and `.richardli` — a TLD belonging to a *person*, via Pacific Century. Amazon holds `.read` and `.room` in the same spirit: strings taken off the board.

**Geography and language.** `.ruhr`, `.rio`, `.ryukyu` — regional, nexus-gated. `.reise` and `.reisen` are German for *travel*, singular and plural, from the same operator: language arbitrage on the same vertical logic as above.

And then the fact that reframes all of it — **the generic namespace is extraordinarily concentrated.** Of ~1,021 live ASCII generic TLDs:

| Operator | TLDs |
|---|---:|
| Binky Moon, LLC *(Identity Digital)* | 192 |
| Dog Beach, LLC *(Identity Digital)* | 47 |
| Identity Digital Limited | 22 |
| Charleston Road Registry *(Google)* | 43 |
| Amazon Registry Services | 39 |
| XYZ.COM LLC | 35 |
| Registry Services, LLC | 34 |

One corporate family — Identity Digital — operates **261 top-level domains**, a quarter of the generic namespace. Google and Amazon hold another 82 between them. What looks like a diverse landscape of quirky independent TLDs is, structurally, a handful of portfolios with a lot of shelf space.

> **Mental model to keep:** you are not browsing an ecosystem. You are browsing the 2012 inventory decisions of about ten companies, most of it held by three.

## 6. The funnel: why a registrar shows you so much less

Between the root zone and a retail search box sit four filters:

1. **Brands removed** (Spec 13 — one permitted registrant, and it isn't you).
2. **Restricted and nexus-gated removed** — credential-verified TLDs where a general registrar can't sell to a general public.
3. **Commercially declined** — the registrar must be accredited with each registry individually, and will skip TLDs with punitive wholesale pricing, weak demand, or bad abuse reputations that pollute its own standing.
4. **Reserved and premium names carved out** *within* the TLDs it does carry — the registry sets these aside at delegation, permanently.

Four filters, applied to 1,438, is why the letter-R page looks thin. It isn't thin. It's *filtered*, four times, by four different parties with four different interests.

---

# Part II — The case study: everything under R

The root currently holds **46 live TLDs beginning with R**. Here they are, in full, with who actually runs them.

## Country-code — a jurisdiction, not a product

| TLD | Administered by |
|---|---|
| `.re` | AFNIC *(Réunion, French overseas department)* |
| `.ro` | National Institute for R&D in Informatics *(Romania)* |
| `.rs` | RNIDS *(Serbia)* |
| `.ru` | Coordination Center for TLD RU *(Russia)* |
| `.rw` | RICTA *(Rwanda)* |

## Brand — delegated, not for sale

| TLD | Operator |
|---|---|
| `.read` | Amazon Registry Services |
| `.redumbrella` | Travelers TLD, LLC |
| `.reliance`, `.ril` | Reliance Industries Limited |
| `.rexroth` | Robert Bosch GMBH |
| `.richardli` | Pacific Century Asset Management (HK) |
| `.ricoh` | Ricoh Company, Ltd. |
| `.rogers` | Rogers Communications Canada |
| `.room` | Amazon Registry Services |
| `.rwe` | RWE AG |

Ten strings — nearly a quarter of the R namespace — that no one can buy.

## Restricted, community, and geographic — gated

| TLD | Operator | Gate |
|---|---|---|
| `.realtor` | Real Estate Domains LLC | NAR membership |
| `.reit` | NAREIT | Must be a REIT |
| `.radio` | Digity, LLC | Broadcast/audio verification |
| `.rugby` | World Rugby Strategic Developments | Sport-community charter |
| `.rio` | IPLANRIO *(Rio de Janeiro municipal IT)* | Geographic |
| `.ruhr` | dotSaarland GmbH | Regional, German nexus |
| `.ryukyu` | BRregistry, Inc. | Regional (Okinawa) |

## Open generic — the part you can actually shop

| TLD | Operator | Archetype |
|---|---|---|
| `.racing` | Premier Registry Limited | Vertical |
| `.realestate` | dotRealEstate LLC | Vertical |
| `.realty` | Internet Naming Co. | Vertical |
| `.recipes` | Binky Moon *(Identity Digital)* | Vertical |
| `.red` | Identity Digital | Colour / suffix hack |
| `.rehab` | Dog Beach *(Identity Digital)* | Vertical |
| `.reise` | Binky Moon | German — *travel* (sing.) |
| `.reisen` | Binky Moon | German — *travel* (pl.) |
| `.ren` | ZDNS International | CJK market play |
| `.rent` | XYZ.COM LLC | Vertical |
| `.rentals` | Binky Moon | Vertical (plural hedge) |
| `.repair` | Binky Moon | Vertical |
| `.report` | Binky Moon | Vertical |
| `.republican` | Dog Beach | Affinity |
| `.rest` | Punto 2012 *(Mexico)* | Suffix hack |
| `.restaurant` | Binky Moon | Vertical |
| `.review` | dot Review Limited | Vertical (singular) |
| `.reviews` | Dog Beach | Vertical (plural) |
| `.rich` | iRegistry GmbH | Aspirational |
| `.rip` | Dog Beach | Suffix hack |
| `.rocks` | Dog Beach | Suffix hack |
| `.rodeo` | Registry Services, LLC | Niche |
| `.rsvp` | Charleston Road *(Google)* | Suffix hack |
| `.run` | Binky Moon | Suffix hack |

Twenty-four strings — of 46 — that a general registrar can plausibly sell you. Identity Digital alone operates fourteen of them.

## The graveyard

Five R strings were delegated and then handed back: **`.raid`**, **`.redstone`**, **`.rightathome`**, **`.rmit`**, **`.rocher`** (Ferrero). All brands. All voluntarily terminated. This is the quiet counter-evidence to the idea that TLDs are scarce and valuable: a brand paid $185,000, cleared evaluation, ran a registry, and then decided the string wasn't worth the compliance overhead of continuing to hold it.

---

# Part III — What to do with the model

## The 2026 round has just closed

The freeze is over. ICANN's **2026 Round** application window ran **30 April – 12 August 2026** — closed a few weeks ago. It's the first expansion in fourteen years. Two changes matter structurally:

- **Mandatory pre-evaluated Registry Service Providers.** Applicants must use an RSP that ICANN has already assessed technically. This decouples "having an idea for a string" from "being able to run DNS infrastructure," and should widen the applicant pool beyond incumbents.
- **The Applicant Support Program**, subsidising fees for applicants demonstrating financial need, aimed squarely at the underrepresentation of the Global South in the 2012 round.

Realistically, delegations land 2028 and beyond. But the namespace you're looking at now is the *last* view of the frozen one.

## Practical consequences

**Renewal is the product; registration is the advertisement.** The registry sets wholesale pricing per TLD and can raise it. A vertical noun offered at £5 for the first year may sit at £40–60 every year thereafter, forever. Always price the ten-year cost, never the first year.

**"Premium" is permanent, not promotional.** Registries carve out desirable strings at delegation and assign them elevated *renewal* tiers. A £3,000 first year can carry a £2,000 annual tail for the life of the name.

**A ccTLD is a political dependency.** `.re`, `.ro`, `.rs`, `.ru`, `.rw` answer to their administrators and their governments, not to ICANN. Eligibility rules, pricing, and continuity can change without an appeals process you can reach. The `.io` sovereignty question and the historic `.ly` suspensions are the cautionary cases. If a brand depends on a ccTLD, that's a jurisdictional risk on the balance sheet.

**Operator concentration is a real risk axis.** If your name sits under one of Identity Digital's 261 TLDs, your pricing and policy are set by one firm's portfolio strategy. Not a reason to avoid them — but a reason to know it.

**The `.com` premium is structural, not sentimental.** It isn't nostalgia. It's the interaction of the three-character minimum, the absence of two-letter alternatives, thirty years of default-typing behaviour, and the fact that every new string carries an unavoidable explanation cost. New TLDs are cheap because attention is expensive.

## Where to look, rather than at a registrar

- `data.iana.org/TLD/tlds-alpha-by-domain.txt` — the flat authoritative list, versioned and dated.
- `iana.org/domains/root/db` — type, sponsoring organisation, and retirement status for every string ever delegated.
- `newgtldprogram.icann.org` — the 2026 round, applicant guidebook, and the evaluated RSP list.
- ICANN's Registry Agreements — where Specification 13 tells you, for any given TLD, whether anyone other than the owner is ever allowed to register in it.

---

# Part IV — The namespace outside the root

Everything above describes one root zone and the rules that shape it. But the root is a *file*, and a file can be copied, extended, or ignored. So the natural next question — is that everything, or are there names out there that resolve without ICANN's permission — has a real answer: **yes, several distinct kinds.** They're worth separating carefully, because "illegal" turns out to mean four different things and they get conflated constantly.

## 0. Ownership was the wrong frame all along

Before going outside, correct the frame inside:

- **You never own a domain.** You hold a renewable licence to a row in a registry database. Stop paying and it reverts. There is no freehold in the DNS.
- **Registries don't own their TLDs.** Verisign doesn't own `.com`; it operates it under a Registry Agreement that can in principle be re-tendered. Identity Digital's 261 TLDs are 261 contracts.
- **ICANN doesn't own the root.** It coordinates it. Since the 2016 IANA Stewardship Transition the function sits with PTI under community oversight, rather than under a US Department of Commerce contract.

So the real axis was never ownership. It is **who resolves the name** — and once you ask that question, the outside becomes legible.

> **Mental model to keep:** a domain name is not property. It is a *rendezvous convention* between you, a resolver, and whoever operates the zone. Change the resolver and you change the universe of names that exist.

## 1. Ring one — reserved by the rules, resolving outside DNS

There is a formal IANA registry of **Special-Use Domain Names (RFC 6761)**: strings deliberately never delegated. It currently includes `localhost.`, `invalid.`, `example.`, `test.`, `local.`, `home.arpa.`, `onion.` and `alt.`.

These are the *legitimate* outside — names that work, but not through the root:

- **`.local`** (RFC 6762) is mDNS. Your printer, resolved by multicast on the local link. No DNS server is involved at any point.
- **`.home.arpa`** (RFC 8375) is the sanctioned name for home networks — created because everyone was squatting `.home`, which is exactly the collision problem that got `.home`, `.corp` and `.mail` permanently blocked.
- **`.alt`** (RFC 9476, 2023) is reserved explicitly *as a container for non-DNS namespaces*. An official "here be alternatives" sign, added to keep future experiments out of the root's way.
- **`.onion`** (RFC 7686) is the elegant one. Tor addresses aren't DNS names at all — a v3 onion is a base32-encoded ed25519 public key, and "resolution" is a rendezvous handshake inside the Tor client. The IETF standardised the string precisely so that ICANN could never sell it out from under the network. An anonymity system holds a reserved slot in the internet's naming rulebook, by consensus.

## 2. Ring two — alternative roots: real, resolvable, non-authoritative

The trick is trivial once you've internalised Part I: the root zone is a text file. Copy it, add rows, serve it, and anyone who points a resolver at you inhabits your namespace.

- **OpenNIC** — volunteer-run, still operating today. It mirrors the ICANN root and adds its own strings: `.geek`, `.oss`, `.libre`, `.pirate`, `.chan`, `.dyn`. Its pitch is explicitly anti-censorship and anti-ISP-hijacking.
- **The historical ones** — AlterNIC, Name.Space, ORSN, and **New.net** (2001), which sold `.shop`, `.club` and `.law` through a browser plugin roughly a decade before ICANN legitimately delegated several of the same strings to other people.
- **Blockchain naming** — Namecoin's `.bit` (2011), Ethereum's `.eth`, Handshake auctioning top-level strings outright, Unstoppable Domains' `.crypto` / `.nft` / `.wallet`. Ownership is a private key; resolution requires a plugin, a gateway, or a custom resolver.

**None of this is illegal.** There is no law against operating a root server with extra rows in it. The objection is technical, and it is stated canonically in **RFC 2826, the IAB's "Technical Comment on the Unique DNS Root"**: the entire value of the DNS rests on a name meaning *the same thing to everyone*. Two roots serving different `.pirate` zones means a URL is no longer a reference at all — it is a reference *relative to your resolver configuration*. Referential integrity is the product. Uniqueness is not a policy preference; it's the thing being sold.

Note also the New.net pattern, which is the standing risk for every alternative namespace: build value in a string, and ICANN may later delegate that same string to someone else entirely, with the full weight of the default root behind them.

## 3. Ring three — the four senses of "illegal"

**Illegal against the grammar, not the law.** Squatting undelegated strings on internal networks. This has bitten hard: `.dev` was a widespread local-development convention until Google was delegated it and preloaded the entire TLD into HSTS. Overnight, thousands of internal setups broke. The permanent reservation of `.home`, `.corp` and `.mail` exists to prevent a much larger version of that event.

**Actually criminal — once.** In 1997 Eugene Kashpureff, who ran AlterNIC, poisoned DNS caches to redirect InterNIC's traffic to his own site. He was prosecuted and convicted of wire fraud. Note precisely what he was convicted *for*: the hijack, not the alternative root. That distinction has held ever since, and it's the cleanest available evidence that running a parallel namespace is lawful while interfering with the canonical one is not.

**Illegal at the content layer, not the name layer.** Tor is legal in most Western jurisdictions and is used by journalists, researchers, and abuse-avoidance services; it is blocked or criminalised in China, Russia, Iran and elsewhere. The namespace isn't illegal. Some of what people host on it is — exactly as with `.com`.

**Jurisdictionally awkward, but entirely legitimate.** `.su` still resolves thirty-five years after the Soviet Union stopped existing, carries a long association with cybercrime, and ICANN has wanted it retired for years. `.kp` exists. Emoji domains are prohibited under IDNA2008 in gTLDs but permitted by some ccTLDs (`.ws`, `.fm`, `.to`) — accessible, functional, and outside the rules everyone else follows, because ccTLDs answer to no one.

## 4. What actually drives the outside: seizure

Here is the asymmetry that explains why alternative namespaces keep being built, and it follows directly from Part I's observation that the root is the internet's most centralised component.

**Because there is a single point of control, domains are seizable.** A US court can order Verisign to repoint any `.com` or `.net`, because Verisign is a US company — ICE's Operation In Our Sites did this to hundreds of names. Nominet suspends tens of thousands of `.uk` domains a year on police request. UDRP can transfer a name to a trademark holder without a court being involved at all.

Blockchain naming exists to remove that lever, and it succeeds completely. No seizure, no UDRP, no registrar. And therefore: no recourse. Lose the key and the name is gone permanently; get phished and there is no transfer reversal; register someone's trademark and nobody on earth can compel you to return it.

That is a clean trade — **accountability for censorship-resistance** — and the reason the debate is so tedious is that advocates on each side price only their own half. The governance apparatus in Part I is not friction bolted onto a naming system. It *is* the naming system's answer to "what happens when something goes wrong," and the alternatives are, precisely and by design, the answer "nothing happens."

## 5. The three-question test

For any name, its real status is settled by three questions:

1. **Who resolves it?** The ICANN root, a special-use protocol, an alternative root, or a blockchain gateway.
2. **Who can take it away?** A registrar, a court, a government — or nobody, which is not automatically the good answer.
3. **Does it mean the same thing to everyone?** If not, it isn't an internet name. It's a name *in your configuration*.

> **The closing model:** what looks like an oddly specific, oddly sparse list of domain endings is the visible surface of one non-profit's rulebook, one 2012 auction, ten portfolio companies' inventory bets, and four layers of commercial filtering between the root zone and the search box. Step outside it and names still resolve — through special-use protocols, volunteer roots, and blockchains — but they stop meaning the same thing to everyone, which was the only property worth having. None of it is arbitrary. All of it is contingent.
