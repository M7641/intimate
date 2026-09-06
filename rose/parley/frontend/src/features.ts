// The feature map, as data — an executable mirror of docs/theory/skill-map.md.
//
// Every page in parley traces to one skill or system and one SLA principle. Keeping
// that mapping here (not scattered in components) means the nav, the "planned"
// pages, and the docs all tell the same story. When the theory changes, this
// changes with it.

export type Status = "live" | "in-progress" | "planned";

export interface Feature {
  /** URL slug and stable id. */
  id: string;
  /** Short nav label. */
  title: string;
  /** The language skill or system this page trains. */
  skill: string;
  /** The SLA principle it serves (see the README's six principles). */
  principle: string;
  /** What actually drives it — see docs/theory/what-powers-it.md. */
  poweredBy: string;
  /** One sentence on why this page exists. */
  rationale: string;
  status: Status;
}

export const FEATURES: Feature[] = [
  {
    id: "converse",
    title: "Converser",
    skill: "Parler — production orale",
    principle: "Pushed output (Swain) + négociation du sens (Long)",
    poweredBy: "Whisper → Qwen2.5/Claude → Piper (la boucle vocale complète)",
    rationale:
      "Le cœur : produire de la parole force le passage du sens à la forme. Le tuteur corrige par recast, sans casser la conversation.",
    status: "live",
  },
  {
    id: "progress",
    title: "Progression",
    skill: "L'état du learner (le liant)",
    principle: "Fixe le « i » de i+1 ; alimente la répétition espacée",
    poweredBy: "Le store de mémoires (fichiers locaux ou MinIO) — aucun modèle",
    rationale:
      "La seule page dont le rôle est de te refléter : ton niveau, tes mots, les domaines explorés, et vers où le tuteur va te pousser.",
    status: "in-progress",
  },
  {
    id: "listen",
    title: "Écouter",
    skill: "Comprendre à l'oral — réception",
    principle: "Comprehensible input (Krashen, i+1)",
    poweredBy: "Qwen/Claude génère un passage, Piper le dit — pas d'ASR",
    rationale:
      "La case réception orale qui manque le plus. Réutilise la machinerie existante dans l'autre sens.",
    status: "planned",
  },
  {
    id: "read",
    title: "Lire",
    skill: "Comprendre à l'écrit — réception",
    principle: "Comprehensible input, à ton rythme",
    poweredBy: "Qwen/Claude seul (passage nivelé, glose au survol)",
    rationale:
      "La fonctionnalité la moins chère : une seule capacité, pas d'audio. Lire à son rythme, avec le sens à portée de clic.",
    status: "planned",
  },
  {
    id: "write",
    title: "Écrire",
    skill: "Rédiger — production écrite",
    principle: "Pushed output, sans la pression du temps réel",
    poweredBy: "Qwen/Claude réagit à ton texte par un recast",
    rationale:
      "Produire à l'écrit, avec le temps de réfléchir à la forme — la boucle Converser sans ASR ni TTS.",
    status: "planned",
  },
  {
    id: "vocabulary",
    title: "Vocabulaire",
    skill: "Système lexical — répétition espacée",
    principle: "Spaced repetition (les mots ratés, revus au bon moment)",
    poweredBy:
      "Le store de mémoires + le Wiktionnaire (définitions françaises fiables) ; SRS à venir",
    rationale:
      "Enregistre un mot, obtiens sa définition depuis un dictionnaire français de confiance. Les mots viennent de TES conversations, pas d'une liste au hasard.",
    status: "in-progress",
  },
  {
    id: "pronunciation",
    title: "Prononciation",
    skill: "Prosodie — shadowing",
    principle: "Shadowing / prosodie (répéter après un natif)",
    poweredBy: "Piper (audio de référence) + Whisper (t'écoute répéter)",
    rationale:
      "Réécoute et répète l'audio du tuteur pour entraîner l'oreille et l'accent. Réutilise les deux capacités audio.",
    status: "planned",
  },
];

export const featureById = (id: string): Feature | undefined =>
  FEATURES.find((f) => f.id === id);
