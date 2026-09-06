
Une collection esquissée d'outils CLI autour de l'écosystème Hugging Face.
Chaque entrée est une *idée d'outil* — un binaire Rust dans le style de
[`nyx`](../blank/nyx/) (texte→texte) ou [`siren`](./siren/) (texte→image),
mais pour une autre modalité d'inférence.

## Critères de sélection

Tous les modèles HF ne donnent pas un bon outil CLI. Les retenus passent
trois filtres :

1. **Ergonomie terminal naturelle.** La sortie est soit du texte (qu'on
   lit ou pipe), soit un artefact (image, audio) qu'on ouvre via le viewer
   système. Si la sortie demande un canvas interactif (bounding boxes,
   masques de segmentation), le terminal n'est pas le bon support.
2. **Poids réalistes.** Tourne sur Mac M-series avec Metal en moins de
   ~8 GB de RAM résidente. SDXL et Flux passent ; Hunyuan-Video non.
3. **Couvert par Candle.** L'écosystème Rust HF natif (= `candle-transformers`)
   a une implémentation, donc on évite Python ou ONNX wrappers.

## Deux familles ergonomiques

Les outils retombent dans l'une de deux formes :

- **Générative (reprompt loop)** — produit un artefact, l'utilisateur juge,
  reprompt ou sauve. C'est le pattern `siren`. S'applique à : texte→image,
  texte→audio, texte→musique, image→image.
- **Extractive (pipe / batch)** — lit une entrée, écrit la sortie sur
  stdout ou dans un dossier. Pas de boucle. Pattern unix classique.
  S'applique à : STT, OCR, traduction, captioning, embeddings.

Reconnaître à laquelle des deux un outil appartient avant de coder évite de
forcer une boucle interactive sur ce qui devrait être un pipe (et vice versa).

---

### `oracle` — speech-to-text *(extractive)*

Whisper sur fichier audio ou capture micro. Sort la transcription sur stdout,
ou en SRT/VTT si demandé.

- **Modèles** : `openai/whisper-tiny` (~75 MB) jusqu'à `whisper-large-v3`
  (~3 GB). `distil-whisper/distil-large-v3` est un bon compromis (~1.5 GB,
  6× plus rapide que large-v3).
- **Pourquoi CLI** : la sortie est du texte. Pipe-friendly :
  `oracle meeting.mp3 | grep "action item"`. Combiné avec `ffmpeg` pour la
  capture micro, ça remplace 80% des usages de transcription manuelle.
- **Candle** : `candle-transformers::models::whisper` existe et est éprouvé.

### `orphée` — text-to-music *(générative)*

Reprompt loop pour clips musicaux courts (8-30 s).

- **Modèles** : `facebook/musicgen-small` (~300 MB) ou `musicgen-medium`
  (~1.5 GB). Stable Audio Open pour les SFX.
- **Pourquoi CLI** : même pattern exact que `siren` — prompt textuel →
  artefact temporel → reprompt. Plus court que la génération d'image, donc
  les itérations sont rapides.
- **Candle** : MusicGen est implémenté.

---

## Vision (génératif)

### `chimère` — image-to-image / inpainting *(générative)*

`siren` part d'un prompt seul ; `chimère` part d'une image existante et la
transforme par prompt. Variations stylistiques, ou inpainting d'une zone
masquée.

- **Modèles** : SD 1.5 img2img (déjà disponible dans le pipeline `siren`,
  petite divergence), `timbrooks/instruct-pix2pix`.
- **Pourquoi CLI** : extension naturelle de `siren`. Pourrait partager 80%
  du pipeline. Le masque pour inpainting est le seul vrai défi UX —
  probablement une commande "ouvre Preview, attendre que tu sauvegardes un
  PNG noir+blanc à ce chemin".

### `loupe` — upscaling *(extractive)*

`loupe in.png 4x > out.png`. Augmente la résolution sans détails inventés
(c'est le complément de la "résolution choisie" dans `siren`, mais sur des
images existantes).

- **Modèles** : Real-ESRGAN (`xinntao/Real-ESRGAN`), SwinIR.
- **Pourquoi CLI** : pipe pur, pas d'interaction. Une commande, un fichier
  en sortie. Idéal pour batcher un dossier.
- **Candle** : pas d'implémentation native ESRGAN à ma connaissance — soit
  ONNX, soit ré-implémenter (réseau pas énorme).

### `silhouette` — détourage / suppression d'arrière-plan *(extractive)*

PNG → PNG avec canal alpha où l'arrière-plan était.

- **Modèles** : `briaai/RMBG-1.4` (~180 MB, excellent), `ZhengPeng7/BiRefNet`
  (plus précis sur les cheveux fins).
- **Pourquoi CLI** : `silhouette photo.jpg > cutout.png`. Pure transformation,
  batchable, terminal-friendly.
- **Candle** : pas d'impl native ; passerait par ONNX (`ort` crate) ou
  conversion vers Candle de l'architecture (U²-Net pour RMBG).

### `parallaxe` — estimation de profondeur monoculaire *(extractive)*

Image RGB → carte de profondeur (grayscale PNG).

- **Modèles** : `depth-anything/Depth-Anything-V2-Small` (~100 MB),
  `Intel/dpt-large`.
- **Pourquoi CLI** : un fichier en entrée, un fichier en sortie. Utile pour
  générer des relief maps, des données pour ControlNet, ou juste pour
  visualiser.
- **Candle** : DPT est implémenté ; Depth-Anything plus récent peut nécessiter
  un effort de portage.

---

## Vision (extractif / texte)

### `héraut` — image captioning *(extractive)*

Lit un dossier d'images, écrit un JSONL `{path, caption}`. Idéal pour générer
de l'alt-text en masse ou indexer une bibliothèque photo.

- **Modèles** : `microsoft/Florence-2-base` (~230 MB, polyvalent : caption,
  OCR, détection), `Salesforce/blip-image-captioning-base` (~990 MB).
- **Pourquoi CLI** : batch + JSONL, pipe-friendly.
  `héraut ./photos/ > captions.jsonl`.
- **Candle** : BLIP est implémenté ; Florence-2 demanderait du portage.

### `glyphe` — OCR *(extractive)*

Image (ou PDF page) → texte extrait, avec structure préservée si possible.

- **Modèles** : `microsoft/trocr-base-printed` (~330 MB, manuscrit ou
  imprimé), `naver-clova-ix/donut-base-finetuned-rvlcdip` (formulaires),
  ou Surya pour les langues multiples avec bounding boxes.
- **Pourquoi CLI** : `glyphe scan.png > text.md`. Vraiment utile pour
  numériser des notes ou des PDFs scannés.
- **Candle** : TrOCR n'est pas dans candle-transformers ; nécessiterait du
  portage ou un fallback Python.

---

## Texte multilingue

### `babel` — traduction locale *(extractive)*

`echo "Bonjour" | babel --to en` → `Hello`. Pas d'API, pas de quota.

- **Modèles** : `facebook/nllb-200-distilled-600M` (~2.5 GB, 200 langues),
  `Helsinki-NLP/opus-mt-*` (paires de langues spécifiques, ~300 MB chacun).
- **Pourquoi CLI** : pipe-friendly, offline, pas de coût par token.
- **Candle** : pas d'implémentation T5/NLLB officielle ; mais Marian et BART
  ont des chemins existants.

### `nomme` — reconnaissance d'entités nommées *(extractive)*

Texte → JSONL des entités détectées (personnes, lieux, orgs, dates).

- **Modèles** : `Davlan/xlm-roberta-base-ner-hrl`, `dslim/bert-base-NER`.
- **Pourquoi CLI** : utile pour pré-traiter des transcripts (sortie d'`oracle`)
  ou des emails avant indexation.

---

## Hybride

### `boussole` — recherche sémantique d'images *(extractive)*

`boussole index ./photos/` → embedde toutes les images avec CLIP, persiste
dans un SQLite local. `boussole search "lighthouse at dusk"` → renvoie les
chemins matchants par similarité cosinus.

- **Modèles** : `openai/clip-vit-base-patch32` (~600 MB) ou
  `laion/CLIP-ViT-B-32-laion2B-s34B-b79K`.
- **Pourquoi CLI** : c'est un cas où le terminal bat n'importe quelle GUI —
  une commande indexe, une autre cherche, pipe le résultat vers `open`,
  `eog`, ou un viewer custom.
- **Candle** : CLIP est partiellement implémenté (ViT + text encoder existent
  déjà dans `siren`'s dependencies).

### `archiviste` — embeddings de texte + recherche locale *(extractive)*

Variante pour texte plutôt qu'image. Indexe Markdown / code / emails,
recherche sémantique.

- **Modèles** : `sentence-transformers/all-MiniLM-L6-v2` (~80 MB),
  `BAAI/bge-small-en-v1.5` (~130 MB).
- **Pourquoi CLI** : `archiviste search "comment gérer les retries"`
  retourne les fichiers pertinents. Concurrent avec `rg` mais sémantique.

---

## Ce qui est *écarté* volontairement

- **Détection d'objets / segmentation** (SAM, YOLO) — sortie visuelle
  difficile à juger sans canvas interactif. Mieux servi par une web UI.
- **Génération 3D** (Shap-E, Zero123++) — le viewer 3D dépasse le scope CLI.
