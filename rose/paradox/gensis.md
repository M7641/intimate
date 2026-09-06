# Elasticity Estimation

The core framing: we're not predicting sales, we're estimating a causal quantity — dQ/dP holding all else equal. In observational data, price is endogenous (firms raise prices when they expect high demand; price co-moves with promos, seasonality, competitor moves, unobserved quality), so a model fit naively to price → quantity learns the equilibrium relationship and can even produce upward-sloping "demand." Every method below is judged by how well it isolates the causal effect, not how well it fits. The gold standard for identification is deliberate price variation (A/B / geo experiments); no architecture recovers elasticity that isn't in the data.
The options:

Double / Debiased ML (DML) — Partial confounders out of both price and quantity with flexible ML, estimate elasticity on residuals. Orthogonality + cross-fitting → low-bias causal estimates; R-learner extension gives heterogeneous elasticity by segment. Best default for tabular retail data.
Deep IV (DeepIV, DeepGMM, adversarial GMM) — Use instruments (input/wholesale cost shocks, same-product prices in other markets) to break endogeneity; a first-stage net models the price distribution, second stage fits structural demand. Use when you have credible instruments.
Continuous-treatment / dose-response nets (VCNet, DRNet, SCIGAN) — Treats price as a continuous treatment and estimates the whole demand curve directly; output is the elasticity curve. The most natural deep-learning framing of elasticity.
Deep choice / structural demand models (deep logit/mixed-logit, BLP, TasteNet) — Neural-net utility inside an economic choice model; preserves downward-sloping demand and substitution structure. Use when cross-price elasticity and cannibalization across a product line matter (gives the full elasticity matrix, not a scalar).
Representation learning for confounders — Embed product text/images, customer history, store context to control for unobserved quality, then feed into a DML or IV stage. Where deep learning beats log-log regression.
Forecasting transformers (TFT, DeepAR, N-BEATS) + gradient — Strong demand forecasters; gradient of forecast w.r.t. price gives a local sensitivity. Caveat: predictive, not causal, unless endogeneity is handled separately.

Practical notes that cut across all six: SKU data is sparse, so hierarchical/multi-task pooling (often hierarchical Bayes) frequently beats a monolithic net; elasticity is nonlinear, heterogeneous, and entangled with cross-price effects — design for a curve and a matrix, not a coefficient.
