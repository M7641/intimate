Ce qui manque (feedback pour mimic)

Manque: Model save/load (torch.save/load marche mais pas d'API)
Impact: Pas de checkpoint, pas de reprise
Contournement: torch.save(model.state_dict(), path) manuellement
────────────────────────────────────────
Manque: Early stopping
Impact: Risque de sur-entraînement sans feedback
Contournement: Callback maison via epoch_callback
────────────────────────────────────────
Manque: Mixed precision (AMP)
Impact: 2x plus lent que nécessaire sur GPU
Contournement: Pas de contournement propre
────────────────────────────────────────
Manque: Batched embedding extraction
Impact: OOM si dataset trop gros pour un seul batch
Contournement: Boucle manuelle sur des mini-batches
────────────────────────────────────────
Manque: EMA encoder (listé comme "Future")
Impact: BYOL/MoCo style momentum = meilleurs embeddings
Contournement: Pas disponible
────────────────────────────────────────
Manque: TabularElementEncoder dans SetTransformer
Impact: SetTransformer n'accepte que input_dim, pas un
ElementEncoder
Contournement: Pré-encoder séparément puis passer au
SetTransformer
