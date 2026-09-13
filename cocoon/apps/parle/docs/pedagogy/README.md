# What the evidence says a top-quality language programme looks like

A literature survey for Parle, September 2026. Pedagogy only; nothing here is
about code. It synthesises second-language-acquisition research, the cognitive
science of memory, the efficacy record of existing apps and methods, and the
design decisions each of those implies.

Roughly 150 sources across six domain surveys. Effect sizes are reported as
found in the cited meta-analyses (Cohen's *d* or Hedges' *g* unless stated).

## Files

| File | Covers |
|---|---|
| [01-acquisition-theory.md](01-acquisition-theory.md) | Krashen and what survived, interaction and output, Nation's four strands, skill acquisition theory, usage-based accounts, explicit vs implicit instruction, task-based teaching, extensive reading, input-only movements, time on task |
| [02-vocabulary-and-memory.md](02-vocabulary-and-memory.md) | Spacing, retrieval practice, schedulers (SM-2, FSRS, half-life regression), desirable difficulties, Dunlosky's technique ratings, vocabulary size, card design, sentence mining, sleep, cognitive load, multimedia |
| [03-listening-pronunciation-speaking.md](03-listening-pronunciation-speaking.md) | Intelligibility, high variability phonetic training, shadowing, listening decoding and metacognition, captions, fluency and task repetition, corrective feedback, anxiety and willingness to communicate, tutors and chatbots, speech recognition |
| [04-motivation-persistence-curriculum.md](04-motivation-persistence-curriculum.md) | Motivation theory, attrition, gamification, deliberate practice, habits and goals, session structure, self-regulation, CEFR and ACTFL, curriculum design, individual differences |
| [05-grammar-literacy-assessment.md](05-grammar-literacy-assessment.md) | Developmental sequences, input enhancement, processing instruction, metalanguage, writing systems, reading and glossing, written feedback, adaptive testing, knowledge tracing, self-assessment, cross-linguistic influence |
| [06-existing-programmes-and-apps.md](06-existing-programmes-and-apps.md) | Efficacy studies of Duolingo, Babbel, busuu, Pimsleur; systematic reviews of mobile learning; community methods (Dreaming Spanish, Refold, LingQ, Assimil, Michel Thomas, Anki); LLM tutors; recurring failure modes |

Evidence strength is flagged throughout as **Strong** (meta-analysis or large
replication), **Moderate** (several studies, mixed or proximal measures) or
**Thin** (single studies, vendor data, or practitioner consensus).

## The short answer

The single clearest finding across every domain is that the popular products
optimise for the easy end of the problem. They teach isolated vocabulary by
recognition, reward time-on-app, and stop at the end of A2. A programme built
from the research would spend most of the learner's time on comprehensible
input, schedule deliberate study by a fitted memory model, teach grammar
briefly and explicitly at the moment of need, make speaking a repeated,
time-pressured act with corrective feedback, and measure progress in things
that cause learning rather than things that reward opening the app.

### Twelve design commitments, in rough order of how much they matter

1. **Design backwards from tasks, and budget honestly in hours.** Define each
   level by what the learner can do (the CEFR's action-oriented approach), then
   tell them the cumulative hours that level typically costs for their language
   pair. The numbers are large and learners deserve them up front.
2. **Balance four strands, and audit the split.** Nation's rule of thumb:
   roughly a quarter each on meaning-focused input, meaning-focused output,
   deliberate language study, and fluency work with already-known language. Log
   where minutes go and check the ratios. Most apps put most of the time in the
   third strand and none in the fourth.
3. **Make comprehensible input the largest component.** Extensive reading and
   listening have some of the most reliable effects in the field. Gate texts by
   lexical coverage, default to reading while listening, offer multiple-choice
   first-language glosses on tap, and stay narrow within a series or topic so
   vocabulary recycles.
4. **Schedule review with a fitted model and require retrieval.** Spacing and
   retrieval practice are the two techniques rated highest for utility across
   educational psychology. Use an FSRS-class scheduler, let the learner set
   retention as a workload dial, and never let an item graduate on a passive
   re-read.
5. **Teach grammar explicitly, briefly, and tied to input and use.** Explicit
   instruction beats implicit in every meta-analysis, including for complex
   forms. Keep explanations short, follow them immediately with structured
   input and meaningful production, and sequence by salience, regularity and
   frequency rather than by a canonical textbook order.
6. **Treat speaking as its own skill to be trained, not a by-product.**
   Practice is skill-specific: comprehension does not buy production. Use task
   repetition under falling time limits, planning time, and a conversation
   partner (an AI partner is defensible for volume, not as a tutor substitute).
   Provide both prompts and recasts as feedback.
7. **Aim pronunciation at intelligibility, and train perception and
   articulation together.** Multi-talker minimal-pair training works for
   perception but transfers weakly to production on its own. Pair it with
   explicit phonetic explanation, pick contrasts by functional load for the
   learner's first language, and score speech only at the granularity where
   recognition is reliable.
8. **Build for interruption, not for streaks.** Two-thirds of app learners
   drop out. Dormant learners commonly return within three months. A forgiving
   return path, weekly rather than daily targets, and competence-signalling
   feedback beat reward currencies, which the longitudinal evidence links to
   lower intrinsic motivation and gaming of the system.
9. **Report progress in causal quantities.** Estimated corpus coverage,
   delayed-retention estimates, hours of input, and can-do tasks passed. Never
   in-session accuracy, points, or streak length, all of which are either
   uninformative or inversely related to durable learning.
10. **Adapt from behaviour, at about 85 percent success.** An adaptive yes/no
    vocabulary test with non-word anchors is the cheapest good placement.
    Online ability and difficulty estimation (Elo or item response theory)
    then keeps generated practice near the sweet spot. Do not gate learners on
    aptitude tests.
11. **Use the first language as an asset.** The evidence for banning the
    mother tongue does not exist. First-language glosses outperform
    second-language ones, cognates are a fast early win, false friends deserve
    their own drill class, and the language pair should set difficulty priors
    and time expectations.
12. **Build a bridge to authentic media before the intermediate plateau.**
    Every commercial app's evidence stops at A2 because its content stops
    there. The methods that produce advanced learners all route them into real
    books, shows and conversations with tooling that keeps the material
    comprehensible.

## Balance and time budgets

Nation's four strands is the most directly implementable finding in the survey
because it can be audited. A well-balanced course gives roughly equal time to:

| Strand | What it is | Nation's rule |
|---|---|---|
| Meaning-focused input | Listening and reading for the message, with 95–98% of running words already known | ~25% |
| Meaning-focused output | Speaking and writing to convey a message | ~25% |
| Language-focused learning | Deliberate study of vocabulary, grammar, pronunciation, strategies | no more than 25% |
| Fluency development | Using only already-known language under time pressure to get faster | at least 25% |

Heil and colleagues' 2016 coding of the 50 most popular apps found 84% taught
vocabulary in isolation, open production was rare and one word long, and
fluency activities were absent. Inferred, a typical app spends roughly 80% of
learner minutes in the third strand.

Time budgets come from institutional sources rather than experiments, and both
describe well-supported learners (**Thin**):

| Level or category | Cumulative hours | Example languages for an English speaker |
|---|---:|---|
| CEFR A1 | 90–100 | |
| CEFR A2 | 180–200 | |
| CEFR B1 | 350–400 | |
| CEFR B2 | 500–600 | |
| CEFR C1 | 700–800 | |
| CEFR C2 | 1,000–1,200 | |
| FSI Category I | 600–750 | Spanish, French, Italian, Portuguese, Dutch, Swedish |
| FSI Category II | 900 | German, Indonesian, Swahili |
| FSI Category III | 1,100 | Russian, Hindi, Greek, Polish, Thai, Turkish, Vietnamese |
| FSI Category IV | 2,200 | Arabic, Mandarin, Cantonese, Japanese, Korean |

Cambridge figures are cumulative guided-learning hours for motivated adults
learning English. FSI figures are full-time classroom hours for selected US
diplomats reaching roughly B2–C1, excluding self-study.

**Design implication.** A B2 goal in a Category I language is about 550 hours,
so roughly 140 hours each of input, output, deliberate study and fluency work.
A programme can show a learner this arithmetic on day one, then show their
actual split every week. Ten minutes a day does not reach these targets, and
the programme should say so.

## Open disputes

Where the field genuinely disagrees, and the design stance this survey takes.

- **The interface question.** Whether explicit knowledge becomes implicit
  competence (strong interface), merely helps noticing (weak), or never
  converts (Krashen). Stance: weak-to-strong, so teach explicitly and
  proceduralise through practice.
- **Delayed speaking.** Input-only movements versus the mainstream view that
  early output is harmless and probably helpful. Almost no controlled evidence
  either way beyond one 1974 study supporting a four-week delay. Stance:
  tolerate a personal silent period, never enforce one, and schedule speaking
  as its own skill.
- **Massed versus spaced for grammar.** Long lags win for retention; massed
  wins for within-session proceduralisation in some studies. Stance: space for
  knowledge, cluster for fluency.
- **Words in context versus isolation.** Almost certainly stage-dependent but
  not cleanly tested. Stance: isolated first, context after graduation.
- **Recasts versus prompts, and feedback timing.** Stance: offer both types,
  vary them, and let the learner set intrusiveness.
- **Gamification.** Small positive short-run effects versus longitudinal harm
  to intrinsic motivation. Stance: competence signals over currencies;
  forgiving weekly structure if any.
- **Perception-to-production transfer in pronunciation, and segmentals versus
  prosody.** Stance: train both, measure in spontaneous speech.

## How much to trust this

- **Most instructed-language research is short and proximal.** The large
  effects (0.9 to 1.4) mean "this reliably changes performance on a test soon
  after," not "this doubles fluency." Generalisation to months of solo desktop
  study is an assumption.
- **The strongest and weakest evidence sit side by side.** Spacing, retrieval,
  glossing, explicit instruction and extensive reading rest on meta-analyses
  with thousands of participants. Sentence mining, Heisig, delayed speaking,
  journaling, kana mnemonics, most polyglot roadmaps and every
  large-language-model tutor rest on community consensus, vendor reports or
  nothing.
- **Several engineering-relevant results are vendor research.** Half-life
  regression, the Duolingo English Test design papers and the platform's
  proficiency reports are transparent and competent but not independent, and
  their samples are unusual slices of real users.

Some primary papers were paywalled during the survey; where an effect size came
from an abstract, a repository copy or citing literature rather than the full
text, the domain files note it. Gaps worth a dedicated follow-up: controlled
evidence on Anki-style sentence cards, peer-reviewed evaluations of the newer
conversation apps, and longitudinal data on self-taught learners who reached C1.
