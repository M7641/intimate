# Cognitive science of memory and vocabulary learning

Part of the [Parle pedagogy survey](README.md).

## 1. Spacing and scheduling

**The core empirical result.** Cepeda, Pashler, Vul, Wixted & Rohrer (2006)
synthesised 839 assessments of distributed practice from 317 experiments.
Averaged across all retention intervals (RI), spaced study produced 47.3%
correct vs 36.7% for massed. The gap widens with delay: at RIs of 8–30 days,
62.2% vs 32.8%; at RIs over 31 days, 39.0% vs 17.0%. The lag effect is
**non-monotonic**: for any given RI there is an optimal inter-study interval
(ISI), and longer is not always better.

Cepeda, Vul, Rohrer, Wixted & Pashler (2008): 1,354 participants learned 32
facts to criterion, restudied after a gap, and were tested at RIs of 7, 35, 70
or 350 days. Optimal gaps for **recall** were 1, 11, 21 and 21 days
respectively. Recall improvements of optimal vs zero gap were 10%, 59%, 111%
and 77%. Pooled, the optimal gap gave a 64% increase in recall (*d* = 1.1) for
identical study time. The optimal gap/RI ratio falls as RI grows: ~20–40% at a
one-week RI, ~5–10% at a one-year RI.

*Design implications.* (a) Spacing is the single highest-leverage free
parameter: same time, ~1.6× the recall. (b) There is no universal correct
interval; schedule backwards from the learner's target retention horizon. (c)
Because the curve is a broad ridge rather than a spike, being roughly right is
nearly as good as being exact. Do not over-engineer.

**Algorithms.** Pimsleur (1967) proposed graduated interval recall with a fixed
schedule (5s, 25s, 2min, 10min, 1h, 5h, 1d, 5d, 25d, 4mo, 2yr), the same for
every item. Wozniak's SM-2 (1987) added per-item adaptivity: I(1)=1 day, I(2)=6
days, I(n)=I(n−1)×EF, with an easiness factor initialised at 2.5 and bounded
below at ~1.3. Settles & Meeder (2016) note both Pimsleur and Leitner are
special cases of an exponential half-life model with hand-picked fixed weights.

**FSRS (Ye et al., Open Spaced Repetition)** replaces heuristics with a fitted
three-component memory model (Difficulty, Stability, Retrievability) trained on
the learner's own review log. The public benchmark uses ~350M reviews from
9,999 Anki collections. FSRS-6 outperforms SM-2 on 99.6% of collections. The
benchmark authors' own caveats: proprietary modern SuperMemo algorithms cannot
be included; SM-2 requires an artificial conversion to probabilities; and an
RWKV sequence model beats FSRS-6 on log loss, so FSRS is not the ceiling. The
"20–30% fewer reviews for the same retention" figure comes from simulation.
**There is no randomised study showing FSRS produces more knowledge than SM-2
in humans.**

**Duolingo's half-life regression** (Settles & Meeder 2016) fits
h = 2^(Θ·x) over 12.9M student-word traces. HLR achieved MAE 0.128 vs Leitner
0.235 (the ~45% reduction), Pimsleur 0.445. Caveats the paper reports and
popular summaries omit: AUC was only 0.538 and Leitner had the best AUC
(0.542); in the head-to-head user experiment, HLR vs Leitner changed daily
retention by only +0.3% (non-significant) and reduced practice activity by
7.3%. The famous "+12% engagement" came from comparing two HLR variants.

*Design implications.* Implement FSRS (open source, well-benchmarked) rather
than SM-2 or a fixed Pimsleur ladder, but describe it honestly: a better
predictor, with only indirect evidence of better learning. Log grades and
intervals from day one so the model can be fitted per user.

**Retention targets.** Anki's manual calls desired retention the most important
FSRS setting; the default is 0.90 and the manual advises staying below 0.97
because workload rises sharply near the ceiling. FSRS's "optimal retention"
tooling minimises minutes studied per unit knowledge retained, and the optimum
is learner-specific, commonly below 0.90.

*Design implications.* Expose retention as a workload dial (e.g. 0.85 light /
0.90 default / 0.95 exam mode), never above ~0.97. Show the projected daily
minutes at each setting; that is the decision the learner is actually making.

**Forgetting curves.** Murre & Dros (2015) replicated Ebbinghaus's 1885 savings
curve and found a discontinuity (an upward jump) around the 24-hour point,
consistent with overnight consolidation.

## 2. Retrieval practice (the testing effect)

Roediger & Karpicke (2006): repeated-study vs repeated-testing with equated
exposures. At a 5-minute delay restudy won (81% vs 75%), but at 2 days testing
won (68% vs 54%, *d* = 0.95) and at 1 week (56% vs 42%, *d* = 0.83). The
most-studied, worst-performing group was the most confident.

Karpicke & Roediger (2008, *Science*): 40 Swahili–English pairs in four
conditions. Dropping items from **restudy** after one correct recall cost
nothing (~80% at one week); dropping items from **testing** was catastrophic
(~33–36%). Retrieval, not re-exposure, builds durable memory.

Rawson & Dunlosky's **successive relearning**: practise to a criterion of 3
correct recalls, then relearn across ~3 widely spaced sessions. One correct
recall in each of three spaced sessions yields more than twice the recall of
three correct recalls in a single session.

*Design implications.* (a) Never let the learner mark an item "known" from a
passive re-read; require a successful retrieval. (b) Present the answer only
after an attempt. (c) Because performance during learning inversely predicts
retention, **do not use in-session accuracy or subjective confidence as your
progress metric**; surface delayed-retention estimates instead. (d) Graduate
items on successful recall in ≥2–3 separate sessions, not repeated correct
answers within one session.

## 3. Desirable difficulties, and their limits

Bjork's framework groups spacing, retrieval, interleaving, generation and
variation as conditions that depress performance during learning but improve
retention and transfer. Nakata & Suzuki (2019) confirmed this for L2 grammar:
interleaved practice produced more errors during training but better 1-week
retention than blocking.

The limits are real. Dunlosky et al. rate interleaving only moderate utility.
The expertise-reversal effect means difficulties that help experts can
overwhelm novices who have not yet formed the categories being discriminated.

Most importantly for vocabulary, **generation can actively harm early form
learning**. Barcroft (2004): learners studied 24 Spanish words either with
word–picture repetition or by writing sentences with them; sentence writing
produced strong negative effects on productive word-form knowledge in both
experiments. His TOPRA model explains this as resource competition: semantic
elaboration draws capacity away from encoding the new form. This contradicts
the naive "richer processing is always better" reading of involvement load.

*Design implications.* Sequence difficulty by stage. Early exposure to a new
word: minimal semantic elaboration, high form repetition, picture or L1 gloss.
Only once form–meaning is secure introduce generation, production and
interleaved contexts. Interleave across items (which SRS does automatically)
but block within a new word's first few encounters.

## 4. Dunlosky et al. (2013) technique ratings

Rated on generalisability across learners, materials, criterion tasks and
contexts:

- **High utility:** practice testing, distributed practice.
- **Moderate:** elaborative interrogation, self-explanation, interleaved
  practice.
- **Low:** summarisation, highlighting, **keyword mnemonic**, imagery for text,
  rereading.

On the keyword mnemonic: "not highly efficient (in terms of time needed for
training and keyword generation), and it may not produce durable learning."
Cued recall of foreign vocabulary after keyword use was either no different
from, or lower than, plain practice testing at a one-week delay (Fritz et al.
2007).

*Design implications.* Make mnemonics an optional user-generated field, never a
mandatory step. Spend effort on scheduling and retrieval formats instead.

## 5. Vocabulary specifics

**Size targets.** Nation (2006): for 98% coverage, learners need **8,000–9,000
word families for written text** and **6,000–7,000 for spoken text**. 4,000
families + proper nouns gives only ~95% of newspaper text; 2,000 families gives
~88% of a novel. Schmitt & Schmitt (2014) partition this into high-frequency
(1–3,000), mid-frequency (3,001–9,000) and low-frequency (9,001+).

*Design implications.* Frequency-order the curriculum through 3,000 families
with explicit deliberate study; from 3,000–9,000 shift toward extensive input
plus mined vocabulary. Show coverage-against-corpus as the progress metric
rather than raw card counts.

**Learning rates and card design.** Nation (1980) found 30–100 new words/hour
from bilingual word pairs. A 2022 research synthesis of 32 word-card studies
found 75% reported positive effects; paper cards outperformed digital and
ready-made outperformed self-constructed. Time spent with cards correlated
only weakly with gains.

Webb, Yanagisawa & Uchihara (2020; 100 effect sizes, 22 studies) found
intentional activities produced ~60% gains on immediate meaning- and
form-recall, falling to 39% and 25% on delayed tests. Productive form knowledge
decays steeply.

Nakata & Webb (2016) tested 4- vs 10- vs 20-item sets: part learning produced
more correct retrievals during learning but no posttest advantage. **Spacing
dominates set size.**

Elgort (2011) answers the standard Krashenite objection to flashcards: 48
deliberately learned pseudowords produced prime-lexicality, masked repetition
and automatic semantic priming effects, i.e. genuine lexical representations.
Deliberate decontextualised learning does create knowledge used in real
processing.

*Design implications.* Don't cap new cards per session on cognitive-load
grounds; cap on projected future review workload. Default to ready-made,
frequency-ordered cards. Use recognition for early form–meaning, promote to
meaning-recall, and only demand form-recall for words the learner needs
productively.

**Context vs isolation.** Webb (2007c) directly compared word pairs against the
same words in a single glossed sentence: **no significant difference**.
Uchihara, Webb & Yanagisawa (2019) found *r* = .34 between encounter frequency
and incidental learning. The glossing meta-analysis (359 effect sizes) found
glossed reading gave 45.3%/33.4% (immediate/delayed) vs 26.6%/19.8% unglossed;
multiple-choice glosses best, L1 glosses better than L2, no difference between
textual, pictorial and auditory modes.

**Involvement load** (Laufer & Hulstijn 2001) predicts retention from need +
search + evaluation. But this is about incidental learning from meaning-focused
tasks, and Barcroft's finding shows it does not license adding elaboration to
early deliberate study.

**Depth and formulaic language.** Schmitt (2014) shows depth adds explanatory
power beyond size. Formulaic sequences make up a third to a half of running
discourse, yet almost no card system teaches them.

*Design implications.* Include collocations and multi-word units as first-class
items. Track receptive/productive and form/meaning as separate states per item,
and schedule them separately.

## 6. Sentence mining and cloze cards

Sentence mining (AJATT → MIA → Refold) rests on Krashen's i+1 plus SRS. It is a
community practice with essentially no direct experimental evidence. What
evidence exists is unflattering to its strong form: Webb 2007c found a single
sentence adds nothing over a word pair; Barcroft 2004 found sentence-level
production harms early form learning; Nakata & Elgort (2021) found spacing in
contextual learning facilitates explicit but not tacit knowledge. Blog claims
that cloze beats word cards cite no controlled comparison.

The defensible parts are the parts independently evidenced: retrieval, spacing,
high encounter frequency in comprehensible input, personally relevant items,
and learning words actually met.

*Design implications.* Support sentence and cloze cards (cheap, motivating,
teach collocation and grammar-in-use) but do not claim superiority and do not
make them the beginner default. Isolated form–meaning card first (with picture
and audio); promote the item to a cloze/sentence card once it graduates.
Enforce i+1 mechanically (one unknown word per mined sentence).

## 7. Sleep, consolidation, cramming

Dumay & Gaskell (2007) taught novel spoken words and found no lexical
competition with existing neighbours immediately after learning, but a clear
effect after a 12-hour period including sleep. New words are integrated into
the lexicon offline. This aligns with the 24-hour discontinuity Murre & Dros
observed.

*Design implications.* Guarantee at least one sleep-crossing review for every
new item; a next-day review is the highest-value single review in the schedule
(and exactly the Cepeda 2008 optimum for a 7-day RI). Prefer daily short
sessions to weekly long ones.

## 8. Cognitive load and session size

Sweller's own analysis: **individual vocabulary items are low in element
interactivity.** What makes vocabulary hard is the number of items, not
simultaneous-element load. Grammar (e.g. French verb agreement) is high in
element interactivity and genuinely needs rationing. Nakata & Webb (2016)
corroborate: 20-item sets were not worse than 4-item sets once spacing was
equated.

*Design implications.* There is no evidence-based "7±2 new words per session"
rule. The real constraint is the **review debt** each new card creates. Size
sessions by projected steady-state minutes/day at the chosen retention target.
Apply cognitive-load discipline where it bites: grammar explanations, UI, and
multi-element production tasks.

## 9. Multimedia, dual coding, images and audio

Paivio's dual coding and Mayer's multimedia theory both predict a benefit for
words plus relevant pictures. Ramezanali et al.'s (2021) meta-analysis of
multimodal glossing (22 studies) found *g* = 0.46 immediate and **g = 0.28
delayed**: real but modest. Mayer's constraints matter: coherence (no
decorative imagery), redundancy (don't present identical text and narration
simultaneously), contiguity, and modality (spoken narration + image beats
on-screen text + image).

Barcroft's word–picture repetition condition is essentially the evidence-based
minimal card: form + picture + audio, repeated.

*Design implications.* Include a picture and native audio on every card where a
picture is meaningful; for abstract items don't force one. Expect a modest, not
transformative, gain.

## 10. Genuine points of disagreement

1. **Expanding vs uniform intervals.** Nakata (2015) found a small expanding
   advantage on delayed receptive tests but not on strict productive scoring.
   Consensus: massed vs spaced matters enormously; expanding vs equal barely
   matters.
2. **What retention target to aim at.** The 0.90 default is a convention; no
   experimental study identifies a target that maximises long-run growth.
3. **Words in context vs isolation.** Almost certainly stage-dependent; not
   cleanly tested.
4. **Incidental vs deliberate.** Elgort contradicts Krashen; but incidental
   learning from input is still where mid-frequency vocabulary and depth come
   from. Both, not either.
5. **Interleaving.** Plausibly harmful for genuine novices.
6. **Digital vs paper cards.** Paper outperformed digital in the synthesis, a
   warning about how easily app affordances (tap-through, low retrieval effort)
   degrade the technique.

## Recommended default design

Frequency-ordered content through 3,000 word families, then input-driven
mining; FSRS scheduling with a user-visible retention dial defaulting to 0.90;
guaranteed sleep-crossing first review; graduation on successful recall in ≥2
separate sessions; recognition → meaning-recall → form-recall promotion ladder
with separate scheduling states; card front minimal (form + audio), back with
L1 gloss + picture; no forced mnemonic step; collocations and formulaic
sequences as first-class items; cloze/sentence cards as a promotion from mature
word cards with mechanical i+1 enforcement; session size governed by projected
review minutes/day; progress reported as estimated corpus coverage and
delayed-retention estimates, never as streaks or in-session accuracy.

## References

- Barcroft, J. (2004). Effects of sentence writing in L2 lexical acquisition. *Second Language Research* 20(4). https://www.researchgate.net/publication/229003412
- Cepeda, N. J., Pashler, H., Vul, E., Wixted, J. T., & Rohrer, D. (2006). Distributed practice in verbal recall tasks: A review and quantitative synthesis. *Psychological Bulletin* 132(3). https://augmentingcognition.com/assets/Cepeda2006.pdf
- Cepeda, N. J., Vul, E., Rohrer, D., Wixted, J. T., & Pashler, H. (2008). Spacing effects in learning: A temporal ridgeline of optimal retention. *Psychological Science* 19(11). https://files.eric.ed.gov/fulltext/ED505660.pdf
- Dumay, N., & Gaskell, M. G. (2007). Sleep-associated changes in the mental representation of spoken words. https://eprints.whiterose.ac.uk/id/eprint/141115/1/preprint.pdf
- Dunlosky, J., Rawson, K. A., Marsh, E. J., Nathan, M. J., & Willingham, D. T. (2013). Improving students' learning with effective learning techniques. *PSPI* 14(1). https://gwern.net/doc/psychology/spaced-repetition/2013-dunlosky.pdf
- Elgort, I. (2011). Deliberate learning and vocabulary acquisition in a second language. *Language Learning* 61(2). https://www.lextutor.ca/freq/lists_download/elgort_2011.pdf
- Expertium / Open Spaced Repetition. Benchmark of spaced repetition algorithms. https://expertium.github.io/Benchmark.html
- FSRS wiki. The optimal retention. https://github.com/open-spaced-repetition/fsrs4anki/wiki/The-optimal-retention
- Karpicke, J. D., & Roediger, H. L. (2008). The critical importance of retrieval for learning. *Science* 319. https://web.mit.edu/jbelcher/www/learner/retrieval.pdf
- Laufer, B., & Hulstijn, J. (2001). Incidental vocabulary acquisition in a second language: The construct of task-induced involvement. *Applied Linguistics* 22(1). https://onlinelibrary.wiley.com/doi/abs/10.1111/0023-8333.00164
- Murre, J. M. J., & Dros, J. (2015). Replication and analysis of Ebbinghaus' forgetting curve. *PLOS ONE* 10(7). https://journals.plos.org/plosone/article?id=10.1371/journal.pone.0120644
- Nakata, T. (2015). Effects of expanding and equal spacing on L2 vocabulary learning. *SSLA* 37. https://eric.ed.gov/?id=EJ1084789
- Nakata, T., & Webb, S. (2016). Does studying vocabulary in smaller sets increase learning? *SSLA* 38. https://www.cambridge.org/core/journals/studies-in-second-language-acquisition/article/abs/does-studying-vocabulary-in-smaller-sets-increase-learning/E17B75ABAE1300734AF014C363D59FBC
- Nakata, T., & Suzuki, Y. (2019). Mixing grammar exercises facilitates long-term retention. *MLJ* 103. https://eric.ed.gov/?id=EJ1225042
- Nakata, T., & Elgort, I. (2021). Effects of spacing on contextual vocabulary learning. *Second Language Research*. https://journals.sagepub.com/doi/10.1177/0267658320927764
- Nation, I. S. P. (2006). How large a vocabulary is needed for reading and listening? *CMLR* 63(1). https://www.lextutor.ca/cover/papers/nation_2006.pdf
- Ramezanali, N., et al. (2021). Efficacy of multimodal glossing on L2 vocabulary learning: A meta-analysis. *TESOL Quarterly*. https://onlinelibrary.wiley.com/doi/abs/10.1002/tesq.579
- Rawson, K. A., & Dunlosky, J. (2022). Successive relearning: An underexplored but potent technique. *Current Directions in Psychological Science*. https://journals.sagepub.com/doi/full/10.1177/09637214221100484
- Roediger, H. L., & Karpicke, J. D. (2006). Test-enhanced learning. *Psychological Science* 17(3). https://colinallen.dnsalias.org/Readings/2006_Roediger_Karpicke_PsychSci.pdf
- Schmitt, N. (2014). Size and depth of vocabulary knowledge: What the research shows. *Language Learning* 64(4). https://onlinelibrary.wiley.com/doi/abs/10.1111/lang.12077
- Schmitt, N., & Schmitt, D. (2014). A reassessment of frequency and vocabulary size in L2 vocabulary teaching. *Language Teaching* 47(4). https://nottingham-repository.worktribe.com/OutputFile/734839
- Settles, B., & Meeder, B. (2016). A trainable spaced repetition model for language learning. *ACL*. https://research.duolingo.com/papers/settles.acl16.pdf
- Uchihara, T., Webb, S., & Yanagisawa, A. (2019). The effects of repetition on incidental vocabulary learning: A meta-analysis. *Language Learning* 69(3). https://onlinelibrary.wiley.com/doi/abs/10.1111/lang.12343
- Webb, S. (2007). Learning word pairs and glossed sentences. *Language Teaching Research* 11(1). https://journals.sagepub.com/doi/10.1177/1362168806072463
- Webb, S., Yanagisawa, A., & Uchihara, T. (2020). How effective are intentional vocabulary-learning activities? A meta-analysis. *MLJ* 104(4). https://onlinelibrary.wiley.com/doi/abs/10.1111/modl.12671
- Wozniak, P. (1987/1990). SuperMemo 2 algorithm. https://super-memory.com/english/ol/sm2.htm
- Yanagisawa, A., Webb, S., & Uchihara, T. (2020). How do different forms of glossing contribute to L2 vocabulary learning? *SSLA*. https://takumiuchihara.weebly.com/uploads/1/2/3/7/123756989/yanagisawa-webb-uchihara-2019-glossing_meta-analysis.pdf
- (2022). Learning English vocabulary from word cards: A research synthesis. *Frontiers in Psychology*. https://www.frontiersin.org/journals/psychology/articles/10.3389/fpsyg.2022.984211/full
- Refold. Sentence mining (community practice). https://refold.la/roadmap/library/sentence-mining

**Note on evidence quality:** the strongest claims here (spacing, retrieval
practice, vocabulary-size thresholds) rest on large meta-analyses or
multi-thousand-participant experiments. The weakest (FSRS's learning benefit,
sentence mining, cloze superiority, the "N new words per session" question)
rest on simulation, community consensus, or nothing at all.
