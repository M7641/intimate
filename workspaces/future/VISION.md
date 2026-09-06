# workspace_three — Vision

> **Le prochain niveau : Arch Linux + Hyprland, streamé dans le navigateur via Selkies,
> avec la meilleure performance possible.**
>
> Ce fichier est volontairement le seul du dossier. C'est une _charte_ — la cible et
> les questions ouvertes — pas une implémentation. On conçoit le plan ensemble à partir
> d'ici.

---

## D'où l'on part (`workspace_two`)

| Dimension            | Niveau actuel                                                                             |
| -------------------- | ----------------------------------------------------------------------------------------- |
| Base OS              | Ubuntu 24.04 (stable)                                                                     |
| Bureau               | **XFCE** (GTK)                                                                            |
| Serveur d'affichage  | **X11** (Xvnc)                                                                            |
| Streaming navigateur | **KasmVNC** — web-native VNC, websocket à la racine                                       |
| Plateforme           | **Nimbus** (Kubernetes), routage **Istio** sur le port 8000                                 |
| GPU                  | **Aucun** — Xvnc software ; « KDE Plasma retiré car ne démarre pas sur ce GPU-less Xvnc » |
| Home                 | Volume persistant `/home/$WORKSPACE_USER_ID` (créé par la plateforme)                     |
| Contraintes pod      | `--security-opt seccomp=unconfined`, sonde readiness = probe TCP sur le port              |
| Outillage            | code-server, JupyterLab, Zed, mise, brew, uv                                              |

C'est solide et stable, mais bridé : X11 + VNC + rendu logiciel plafonnent la fluidité,
et XFCE est un choix de prudence, pas de performance ni d'esthétique.

---

## La cible (`workspace_three`)

| Dimension            | Niveau visé                                        | Pourquoi                                                                                    |
| -------------------- | -------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| Base OS              | **Arch Linux** (rolling)                           | Hyprland/wlroots/Mesa toujours à jour — crucial car le capture Wayland headless évolue vite |
| Bureau               | **Hyprland** (compositeur Wayland wlroots, tiling) | Performance, esthétique moderne (style Omarchy déjà visé dans v2), animations GPU           |
| Serveur d'affichage  | **Wayland** (headless)                             | Pipeline de rendu moderne, pas de legacy X                                                  |
| Streaming navigateur | **Selkies** (WebRTC, GStreamer)                    | Latence et fluidité bien supérieures au VNC ; codec vidéo réel plutôt que diff d'images     |
| Plateforme           | Idem Nimbus / Istio / port unique                    | On hérite du contrat plateforme — à ne pas casser                                           |
| Performance          | **Maximale**                                       | Objectif explicite — voir le triangle ci-dessous                                            |

**Le thème central : deux migrations couplées.** Passer à Hyprland _oblige_ à quitter
KasmVNC. KasmVNC **est** un serveur X11 ; il ne peut pas capturer un compositeur Wayland.
Le changement de bureau entraîne donc le changement de couche de streaming. Selkies
(WebRTC) est le candidat naturel — et c'est aussi un gain de performance en soi.

---

## ⚠️ Le triangle de performance GPU-less

C'est la question qui décide de tout. Sans GPU, on accumule **deux** charges CPU que
`workspace_two` n'avait pas (XFCE en X11 software reste léger) :

```
            ┌──────────────────────────┐
            │   Compositing Hyprland   │  Hyprland rend en OpenGL/EGL.
            │   (rendu GL)             │  Sans GPU → llvmpipe (Mesa software).
            └────────────┬─────────────┘  Coût CPU + risque de saccades.
                         │
            ┌────────────┴─────────────┐
            │   Encodage vidéo WebRTC  │  Selkies encode le flux.
            │   (GStreamer)            │  Sans GPU → x264 logiciel.
            └──────────────────────────┘  CPU-bound : plafonne résolution + fps.
```

> **La décision n°1 à trancher ensemble : nœud avec GPU, ou GPU-less ?**
>
> - **Avec GPU** : encodage matériel (NVENC / VA-API) + compositing GL natif → Hyprland
>   brille, Selkies stream en haute résolution/60fps à faible CPU. C'est le scénario où
>   « best possible performance » a un sens plein.
> - **GPU-less** : faisable, mais il faut accepter llvmpipe + x264 logiciel, et tuner
>   agressivement (résolution modérée, x264 `ultrafast`/`zerolatency`, fps plafonné).
>   Hyprland reste plus exigeant qu'XFCE ici — à valider tôt par un prototype.

---

## Selkies — ce qui pourrait l'empêcher de marcher (à clarifier ensemble)

Tu as flagué le doute sur Selkies. Voici les points concrets à examiner, du plus
risqué au moins risqué :

1. **WebRTC à travers Istio/Kubernetes — le risque majeur.**
   KasmVNC marche à la racine via un simple websocket (TCP, HTTP-friendly) qu'Istio route
   sans souci. Selkies, lui, fait du **WebRTC** : signaling websocket _plus_ un transport
   média (UDP idéalement). Derrière un pod k8s + Istio, le média WebRTC ne passe souvent
   pas sans un **serveur TURN** (coturn) pour relayer, voire du TURN-over-TCP. **C'est
   probablement « la chose qui empêcherait Selkies de marcher ».** À concevoir explicitement :
   où vit le TURN, comment Istio l'expose.

2. **Capture Wayland.** Selkies a historiquement capturé du **X11** (`ximagesrc`). Pour
   Hyprland, il faut un chemin Wayland : soit le protocole **wlr-screencopy**, soit
   **PipeWire** via `xdg-desktop-portal-hyprland` (`pipewiresrc`). À vérifier : la version
   de Selkies retenue supporte-t-elle nativement ce chemin, ou faut-il l'assembler ?

3. **Hyprland headless.** Aucun écran physique dans un pod → backend **headless** de
   wlroots (sortie virtuelle). À valider : Hyprland démarre-t-il en headless **et** sans
   GPU (EGL software) ? C'est l'écho direct du « KDE Plasma ne démarrait pas sur le
   GPU-less Xvnc » — même catégorie de risque, à dérisquer par un prototype minimal.

4. **Routage à la racine.** Le client web Selkies + son signaling doivent fonctionner à
   la racine du port 8000 comme KasmVNC. À confirmer côté chemins/préfixes.

5. **Audio.** Selkies stream aussi l'audio (PipeWire/PulseAudio). À câbler, mais mineur.

6. **Contrat plateforme.** Réutiliser tel quel : attente de `WORKSPACE_USER_ID`, home
   persistant, seed `/etc/skel`, probe TCP. Ne pas casser ce qui marche déjà.

---

## Questions ouvertes à trancher (ordre du jour de notre design)

1. **GPU ou GPU-less ?** — détermine si « performance maximale » est atteignable ou s'il
   faut redéfinir l'objectif. _Tout le reste en découle._
2. **TURN/coturn** — obligatoire ou contournable dans notre topologie Istio ?
3. **Chemin de capture Wayland** — wlr-screencopy vs PipeWire portal.
4. **Version/fork de Selkies** — laquelle gère le mieux Wayland + notre réseau ?
5. **Arch en conteneur** — rolling release vs reproductibilité des builds ; épingler quoi ?
6. **Stratégie de repli** — si Hyprland GPU-less ne tient pas la fluidité : compositeur
   wlroots plus léger (Sway/labwc), ou exiger un GPU ?

---

## Critères de réussite

`workspace_three` est un succès si, par rapport à `workspace_two` :

- [ ] La latence et la fluidité perçues dans le navigateur sont **nettement** meilleures.
- [ ] Hyprland tourne de façon stable (headless, et dans le mode GPU retenu).
- [ ] Le flux Selkies/WebRTC traverse Istio de bout en bout (média compris).
- [ ] Le contrat plateforme Nimbus (user, home persistant, probe) reste intact.
- [ ] Le tout reste reproductible depuis un Dockerfile, comme les niveaux précédents.

---

_Prochaine étape : on s'attaque d'abord à la décision GPU et au prototype « Hyprland
headless démarre-t-il ? », car ce sont les deux qui peuvent invalider tout le reste._
