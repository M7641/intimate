"""
Q-Learning canonique sur un GridWorld 4x4
==========================================

L'exemple « hello world » du reinforcement learning tabulaire.

Carte:
    S . . .
    . X . .
    . . . .
    . . . G

  S = départ      (0, 0)
  G = but         (3, 3)   récompense +10, épisode terminé
  X = piège       (1, 1)   récompense -10, épisode terminé
  .              chaque pas coûte -1 (incite l'agent à finir vite)

Actions: 0 = haut, 1 = bas, 2 = gauche, 3 = droite.
Le monde est déterministe (pas de bruit sur les transitions) pour rester lisible.
"""

import random

import numpy as np

# ---------------------------------------------------------------------------
# Environnement
# ---------------------------------------------------------------------------

GRID_H, GRID_W = 4, 4
START = (0, 0)
GOAL = (3, 3)
PIT = (1, 1)

# (delta_ligne, delta_colonne) pour chaque action
ACTIONS = [(-1, 0), (1, 0), (0, -1), (0, 1)]
N_ACTIONS = len(ACTIONS)


def step(state, action):
    """Un pas dans l'environnement. Renvoie (next_state, reward, done)."""
    dr, dc = ACTIONS[action]
    r, c = state
    # Les murs bloquent : on reste sur place si on sort de la grille
    nr = max(0, min(GRID_H - 1, r + dr))
    nc = max(0, min(GRID_W - 1, c + dc))
    next_state = (nr, nc)

    if next_state == GOAL:
        return next_state, 10.0, True
    if next_state == PIT:
        return next_state, -10.0, True
    return next_state, -1.0, False


# ---------------------------------------------------------------------------
# Q-learning
# ---------------------------------------------------------------------------

ALPHA = 0.1  # taux d'apprentissage : combien on bouge Q vers la nouvelle estimation
GAMMA = 0.95  # facteur d'actualisation : à quel point on valorise le futur (0..1)
EPSILON = 0.1  # taux d'exploration : proba de jouer au hasard
N_EPISODES = 500

# Table Q : pour chaque (ligne, colonne, action) -> valeur estimée
# Forme (4, 4, 4). On initialise à zéro : pas de biais a priori.
Q = np.zeros((GRID_H, GRID_W, N_ACTIONS))


def choose_action(state):
    """Politique ε-greedy : explore avec proba ε, sinon exploite le meilleur Q connu."""
    if random.random() < EPSILON:
        return random.randint(0, N_ACTIONS - 1)
    return int(np.argmax(Q[state]))


for episode in range(N_EPISODES):
    state = START
    done = False

    while not done:
        action = choose_action(state)
        next_state, reward, done = step(state, action)

        # Cœur de l'algorithme : mise à jour de Bellman.
        #
        #   Q(s,a) <- Q(s,a) + alpha * [ r + gamma * max_a' Q(s',a')  -  Q(s,a) ]
        #             \_________/   \_________________________________/
        #              ancienne          cible TD (Temporal Difference)
        #              estimation
        #
        # Sur un état terminal, il n'y a pas de futur : max_a' Q(s',a') = 0.
        best_next = 0.0 if done else np.max(Q[next_state])
        td_target = reward + GAMMA * best_next
        td_error = td_target - Q[state][action]
        Q[state][action] += ALPHA * td_error

        state = next_state


# ---------------------------------------------------------------------------
# Affichage de la politique apprise
# ---------------------------------------------------------------------------

ARROWS = ["^", "v", "<", ">"]

print("Politique apprise (flèche = meilleure action selon Q):\n")
for r in range(GRID_H):
    row = ""
    for c in range(GRID_W):
        if (r, c) == GOAL:
            row += " G "
        elif (r, c) == PIT:
            row += " X "
        else:
            row += f" {ARROWS[int(np.argmax(Q[r, c]))]} "
    print(row)

print("\nValeur de l'état V(s) = max_a Q(s,a):\n")
for r in range(GRID_H):
    print("  ".join(f"{np.max(Q[r, c]):+6.2f}" for c in range(GRID_W)))
