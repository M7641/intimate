## The unifying view

Every optimisation problem reduces to the same schema:

> Find **x** that minimises **f(x)** subject to constraints **g(x) ≤ 0**, **h(x) = 0**.

What distinguishes the paradigms is not the schema but **how much structure of _f_ and _g_ you can exploit**. This is the master axis of the entire field:

- **More structure** (linearity, convexity, differentiability, closed form) → stronger guarantees, faster convergence, larger solvable problems.
- **Less structure** (black-box evaluation, noise, discontinuities, expensive queries) → you fall back to sampling, surrogate modelling, or population search, and give up guarantees in exchange for generality.

The tool you reach for is a consequence of the structure you have, not a choice you make independently. When a problem feels confusing, the productive questions are:

1. Can I write _f_ and _g_ in closed algebraic form?
2. Is _f_ convex? Differentiable?
3. Are variables continuous, integer, or mixed?
4. Is evaluation cheap or expensive?
5. Is there noise in the evaluation?
6. Is there a single objective, or multiple conflicting ones?
7. Is there uncertainty in the parameters themselves?
8. Are decisions sequential with state dynamics?

Answering those pins you to a paradigm, and each paradigm has two or three mature Python tools.

---

## Quick decision framework

|Structure of the problem|Paradigm|Primary Python tools|
|---|---|---|
|Linear/convex, closed form|Convex programming|CVXPY, Pyomo + LP/QP solver|
|Algebraic with integers|Mixed-integer programming|Pyomo, OR-Tools, gurobipy|
|Combinatorial, heavy constraints|Constraint programming|OR-Tools CP-SAT, MiniZinc|
|Smooth, nonlinear, modest scale|Nonlinear programming|scipy.optimize, IPOPT via Pyomo|
|Differentiable, huge scale, noisy gradients|Stochastic gradient methods|PyTorch, JAX + Optax|
|Black-box, expensive evaluation|Bayesian / derivative-free|Optuna, BoTorch, Ax|
|Black-box, cheap, multi-objective|Evolutionary / metaheuristic|pymoo, DEAP, Nevergrad|
|Parameters themselves are uncertain|Stochastic / robust programming|Pyomo + mpi-sppy, CVXPY robust|
|Sequential, state-dependent decisions|Reinforcement learning / DP|stable-baselines3, RLlib|

The rest of this document discusses each paradigm in turn.

---

## 1. Linear and convex programming

**Problem form.** _f_ and _g_ are linear (LP), convex quadratic (QP), or more generally convex. Variables are continuous.

**Why it is the gold standard.** Convexity guarantees that any local minimum is the global minimum. This single property unlocks algorithms with strong theoretical convergence rates and makes the problem reliably solvable at enormous scale — millions of variables for LPs, hundreds of thousands for convex QPs and SOCPs.

**Algorithms.** The simplex method and its dual (for LPs), interior-point methods (for LPs, QPs, SOCPs, SDPs), and first-order splitting methods (ADMM, proximal gradient) for very large or distributed problems.

**Python tooling.**

- **CVXPY** — disciplined convex programming. You describe the problem using a grammar that guarantees convexity by construction; the library refuses non-convex formulations by design. Best choice when you want to be sure your problem is convex and you want concise, mathematical-looking code.
- **Pyomo** — a general algebraic modelling language. More verbose than CVXPY but supports the full spectrum from LPs up through MINLPs. Pick it when you need the same modelling environment to cover linear, integer, and nonlinear models.
- **Direct solver APIs** — `gurobipy`, `docplex`, `highspy`. Lower-level and solver-specific; chosen when you want maximum performance or access to solver-specific callbacks, lazy constraints, or warm-starting.

**Typical applications.** Portfolio optimisation (Markowitz), network flow problems, production planning, regression with convex regularisers (Lasso, Ridge), optimal power flow (convex relaxations), many problems in control and signal processing.

**When not to use.** The moment you introduce integer variables, combinatorial structure, or non-convex nonlinearity, you leave this paradigm and need the next ones down.

---

## 2. Mixed-integer programming (MIP / MILP / MINLP)

**Problem form.** Algebraic like LP/NLP, but some variables are restricted to integers (often binary). This is enough to make the problem NP-hard in the worst case.

**Why it remains tractable.** The last three decades have produced extraordinary progress in MIP solvers. Branch-and-bound exploits LP relaxations as bounds; cutting-plane methods tighten those relaxations; presolve, heuristics, conflict analysis, and parallel tree search combine to crush problems that would be hopeless under naïve enumeration. Real industrial problems with hundreds of thousands of variables are routinely solved to proven optimality or small optimality gaps.

**Algorithms.** Branch-and-bound, branch-and-cut, branch-and-price (column generation), Benders decomposition. For MINLPs, outer approximation and spatial branch-and-bound.

**Python tooling.**

- **Pyomo** — the most general choice, supports MILP, MIQP, MINLP.
- **OR-Tools** (Google) — has a solid MIP interface plus a very strong CP-SAT solver for combinatorial problems.
- **gurobipy / docplex** — direct APIs for the two leading commercial solvers. Commercial licences are expensive but academic licences are free.
- **HiGHS** — the best open-source MILP solver currently available, usable via `highspy` or through Pyomo.

**Typical applications.** Scheduling (production, staff rostering, sports), vehicle routing, facility location, unit commitment in power systems, network design, cutting stock problems, lot sizing.

**Practical note.** Modelling choice matters enormously. The same combinatorial problem can have formulations that differ by orders of magnitude in solver time. Knowing standard tricks (big-M vs. indicator constraints, tight formulations, symmetry breaking) is often more valuable than which solver you pick.

**When not to use.** When the problem is dominated by intricate logical constraints rather than arithmetic (see constraint programming), or when the objective is a black-box simulator (see black-box methods).

---

## 3. Constraint programming (CP)

**Problem form.** Discrete variables with domains, and constraints expressed as relations (all-different, cumulative, table, regular, etc.). The objective, if any, is usually secondary to feasibility.

**How it differs from MIP.** MIP works by solving a continuous relaxation and branching. CP works by **constraint propagation** — each constraint prunes values from variable domains, and the solver searches over remaining possibilities. On heavily combinatorial problems with no useful linear relaxation (puzzles, complex scheduling rules, configuration problems), CP often dominates MIP.

**Algorithms.** Domain propagation, arc consistency, backtracking search with intelligent variable and value ordering, nogood learning (CP-SAT combines CP with SAT solving techniques).

**Python tooling.**

- **OR-Tools CP-SAT** — the current state of the art for combinatorial problems. Free, extremely fast, excellent Python API.
- **MiniZinc** (via `minizinc-python`) — a high-level modelling language that can dispatch to multiple solver backends, useful for benchmarking.

**Typical applications.** Employee scheduling with complex labour rules, timetabling, job-shop and flow-shop scheduling, resource allocation with intricate compatibility constraints, puzzles (Sudoku, nurse rostering benchmarks).

**When to reach for CP instead of MIP.** When constraints are naturally expressed as "this set of activities cannot overlap", "these values must all differ", or "this sequence must match a pattern" — things that translate clumsily into linear inequalities. Try both if you are unsure; CP-SAT is free and fast enough to be worth a benchmark.

---

## 4. Smooth nonlinear programming (NLP)

**Problem form.** _f_ is continuous and differentiable, constraints likewise. No convexity requirement — so only local optima are guaranteed without additional global search machinery.

**Algorithms.** Gradient descent and its variants (conjugate gradient, L-BFGS), Newton and quasi-Newton methods, trust-region methods, sequential quadratic programming (SQP), interior-point methods (IPOPT). All rely on first or second derivatives, ideally computed exactly via automatic differentiation.

**Python tooling.**

- **scipy.optimize** — `minimize` with methods like `L-BFGS-B`, `trust-constr`, `SLSQP`. The go-to for modest-scale unconstrained or lightly-constrained NLPs.
- **IPOPT** (via Pyomo or `cyipopt`) — the standard for large-scale constrained NLPs, widely used in engineering and optimal control.
- **JAX / PyTorch + manual optimiser** — when you need automatic differentiation through a complex computational graph but the scale isn't large enough to need SGD.
- **NLopt** (via `nlopt` Python bindings) — a collection of global and local algorithms with a uniform interface, good for quick experimentation.

**Typical applications.** Maximum-likelihood estimation, generalised nonlinear least squares, parameter fitting in scientific models, trajectory optimisation in robotics, model-predictive control, chemical process design.

**Global vs. local optima.** A non-convex NLP solver returns a local minimum determined by the starting point. If global optimality matters, either use a global solver like BARON or Couenne (through Pyomo), or run many restarts with different initial points. In practice, local solutions are often acceptable because the modeller brings enough domain knowledge to initialise sensibly.

---

## 5. Stochastic gradient methods (deep learning)

This deserves its own paradigm because the scale and noise profile change everything.

**Problem form.** _f(θ) = E[ℓ(θ; data)]_ — the expectation of a per-example loss over a very large dataset. Parameters _θ_ number from millions to trillions. Full-batch gradients are computationally infeasible; you compute noisy stochastic gradients on minibatches.

**Why classical methods fail.** Second-order methods need a Hessian whose size is quadratic in parameter count — impossible at scale. Line searches and trust regions don't play well with stochastic gradients. Even computing the full gradient is usually too expensive. The field therefore evolved its own algorithmic toolkit.

**Algorithms.** SGD with momentum, Nesterov momentum, Adam, AdamW, RMSprop, Adagrad, Lion, Muon. Learning-rate schedules (cosine decay, warmup, one-cycle). Gradient clipping. Regularisation through weight decay, dropout, batch/layer normalisation. Convergence is to a stationary point, not a global minimum — and in practice the loss landscapes of large neural networks are benign enough that this is fine.

**Python tooling.**

- **PyTorch** (with `torch.optim`) — the dominant framework for research and much of production. Eager by default, with `torch.compile` for graph-level optimisation.
- **JAX + Optax** — functional, composable optimiser library; favoured where you want explicit control over the gradient pipeline or first-class support for `vmap`, `pmap`, and TPUs.
- **TensorFlow + Keras** — still widely used, particularly in production systems at Google-scale companies.

**Typical applications.** Training neural networks of every variety — vision, language, speech, recommendation, contrastive and self-supervised representation learning, diffusion models, reinforcement-learning policies.

**Relationship to the rest of optimisation.** SGD-family methods are a very specific adaptation of gradient descent to the particular regime of "cheap noisy gradients, huge parameter counts, non-convex landscapes that happen to be trainable anyway". They are not a general-purpose replacement for NLP solvers — try Adam on a small constrained engineering problem and it will perform dreadfully compared to IPOPT.

---

## 6. Derivative-free and Bayesian optimisation

**Problem form.** _f_ is a black box — you can evaluate it, but cannot differentiate it or look inside. Crucially, **evaluations are expensive**: a single call might entail training a model, running a CFD simulation for hours, or conducting a physical experiment.

**Key idea.** Since you can afford only a small number of evaluations, you build a **surrogate model** of _f_ from the points you have observed, and use an **acquisition function** over the surrogate to decide where to sample next. The acquisition function trades off exploitation (sampling where the surrogate predicts low _f_) and exploration (sampling where the surrogate is uncertain).

**Algorithms.** Bayesian optimisation with Gaussian process surrogates is the canonical approach. Tree-structured Parzen estimators (TPE) scale better to many dimensions and mixed variable types. Trust-region Bayesian optimisation (TuRBO) handles higher-dimensional problems. For purely derivative-free local search without surrogates, methods like Nelder-Mead simplex, Powell's method, and pattern search are available.

**Python tooling.**

- **Optuna** — broad, practical, widely adopted for hyperparameter tuning. Uses TPE by default, supports pruning of bad trials.
- **BoTorch** (built on PyTorch) — research-grade Bayesian optimisation with GPU-accelerated Gaussian processes and flexible acquisition functions. More complex to use but far more powerful.
- **Ax** (Meta, built on BoTorch) — higher-level, handles experiment tracking and multi-objective BO.
- **scikit-optimize** — simpler GP-based BO, good for quick starts.
- **Nevergrad** (Meta) — collects many derivative-free algorithms (including CMA-ES, BO, and evolutionary methods) behind one API.

**Typical applications.** Hyperparameter tuning for ML models, experimental design (chemistry, materials, drug discovery), calibration of simulators, A/B test allocation, automated design space exploration.

**Sample efficiency is the whole point.** If you can afford thousands of evaluations, you are in the next paradigm (evolutionary). If you can afford only tens or hundreds, Bayesian methods pay off.

---

## 7. Evolutionary and metaheuristic optimisation

**Problem form.** Also black-box, but evaluations are **cheap** enough to afford thousands or millions of them. Often handles discrete, mixed, or unusual variable types, and often solves multi-objective problems directly.

**Key idea.** Maintain a **population** of candidate solutions and evolve it via stochastic operators (mutation, crossover, selection). The population serves as an implicit model of the landscape and, in multi-objective settings, as an approximation of the Pareto front.

**Algorithms.**

- _Single-objective._ Genetic algorithms (GA), differential evolution (DE), particle swarm optimisation (PSO), simulated annealing (SA), CMA-ES (usually the strongest general-purpose continuous black-box optimiser).
- _Multi-objective._ NSGA-II, NSGA-III, MOEA/D, SPEA2 — these return an approximate Pareto front rather than a single point.

**Python tooling.**

- **pymoo** — the standout library for multi-objective evolutionary optimisation. Clean API, comprehensive algorithm catalogue, good visualisation of Pareto fronts.
- **DEAP** — older and lower-level; powerful if you want to build custom evolutionary algorithms from components.
- **Nevergrad** — again, worth considering for its unified interface across many algorithms.
- **`cma`** (the CMA-ES reference implementation) — if you specifically want CMA-ES for continuous black-box problems.

**Typical applications.** Engineering design with fast simulators, antenna and structural design, game AI, hyperparameter search when evaluation is cheap, neural architecture search in some regimes, any problem where you want to explore trade-offs across multiple objectives rather than collapse them into one.

**Guarantees.** None. These are heuristics; they work empirically well but cannot prove optimality. The trade-off is generality: they handle non-differentiable, discontinuous, discrete, mixed, and multi-objective problems without fuss.

**When to prefer evolutionary over Bayesian.** When evaluations are cheap and you want to run many. When the problem is multi-objective and you want a Pareto front directly. When the variable space is mixed or unusual and surrogate models struggle.

---

## 8. Stochastic programming and robust optimisation

This is an **orthogonal axis** rather than a separate paradigm — it stacks on top of the structural paradigms above.

**Problem form.** The parameters of the optimisation problem (demands, prices, returns, arrival rates) are themselves uncertain. Optimising against point estimates gives solutions that are fragile in practice.

**Two main approaches.**

- **Stochastic programming.** Model uncertainty as a probability distribution over scenarios and optimise expected cost (sometimes plus a risk measure like CVaR). Two-stage stochastic programs distinguish _here-and-now_ decisions (before uncertainty is resolved) from _recourse_ decisions (after). Multi-stage extends this to a decision tree.
- **Robust optimisation.** Model uncertainty as a set (a box, an ellipsoid, a polyhedron) and optimise worst-case cost over the set. Often more tractable than stochastic programming because the robust counterpart of a convex problem is often itself convex.
- **Chance-constrained programming.** Require constraints to hold with at least some probability (e.g. 95%), rather than always or in expectation.

**Python tooling.**

- **Pyomo + mpi-sppy** — stochastic programming at scale with scenario decomposition (progressive hedging, L-shaped method).
- **CVXPY** — supports robust convex formulations naturally via its disciplined convex grammar.
- **PySP** — an older Pyomo-based stochastic programming extension.
- **`rsome`** — a newer library focused specifically on robust optimisation.

**Typical applications.** Portfolio optimisation with return uncertainty, supply chain design with demand uncertainty, energy markets with renewable generation uncertainty, inventory management, reservoir operation.

**Practical warning.** Stochastic and robust formulations multiply problem size (by the number of scenarios or by the dimension of the uncertainty set). Decomposition methods and careful scenario reduction are often essential.

---

## 9. Sequential decision-making: dynamic programming and reinforcement learning

**Problem form.** Decisions are made **in sequence**, each action influences the next state, and you want to minimise expected cumulative cost (or maximise expected cumulative reward) over a horizon. Formally, a Markov decision process (MDP).

**Why it is its own world.** Static optimisation finds a point _x_. Sequential optimisation finds a **policy** _π(state) → action_ — a function. The space of policies is infinite-dimensional in general, and the Bellman equation replaces the first-order optimality conditions.

**Algorithms.**

- _Known dynamics, small state space._ Dynamic programming — value iteration, policy iteration, linear programming formulations of MDPs.
- _Unknown dynamics, learn from samples._ Reinforcement learning — Q-learning, SARSA, policy gradient methods (REINFORCE, PPO, TRPO), actor-critic (A2C, SAC, DDPG, TD3), model-based RL, offline RL.
- _Continuous state and action with known dynamics._ Optimal control — LQR for linear-quadratic problems, iLQR / DDP for nonlinear, model-predictive control (MPC) for online replanning.

**Python tooling.**

- **Stable-Baselines3** — mature, well-tested implementations of common RL algorithms.
- **RLlib** (part of Ray) — distributed RL, scales across clusters.
- **CleanRL** — single-file reference implementations, excellent for learning and research.
- **Gymnasium** (successor to OpenAI Gym) — the standard environment interface.
- **do-mpc** or **CasADi** — for nonlinear MPC and optimal control with known dynamics.

**Typical applications.** Robotics, game-playing (AlphaGo, OpenAI Five), resource allocation over time, inventory and pricing policies, recommendation systems, autonomous driving, large-language-model alignment via RLHF/RLAIF.

**Relationship to the rest.** Many RL algorithms internally use gradient-based optimisation (SGD on neural policies), Bayesian methods (for exploration), or classical optimisation (MPC uses NLP at each step). RL is a different _framing_, not a disjoint technique set.

---

## Cross-cutting observations

**Composition is the norm.** Real systems blend paradigms. An ML-driven supply chain might use SGD to train a demand forecasting model, Bayesian optimisation to tune its hyperparameters, stochastic programming to produce a robust ordering policy against forecast uncertainty, and a MIP solver to schedule the actual truck dispatches. Recognising which paradigm solves which sub-problem is the core skill.

**Problem reformulation beats algorithm choice.** A huge amount of practical optimisation lives in reformulating a problem into a stronger class. Convex relaxations of non-convex problems, linearisation of bilinear terms via McCormick envelopes, piecewise linear approximations of nonlinear functions, Lagrangian decomposition to expose structure — these techniques often matter more than which solver you pick.

**Know your scale and your budget.** The decisive questions in picking a paradigm are usually operational: how many variables, how expensive is one evaluation, how many evaluations can I afford, what guarantees do I actually need, and what are my latency constraints? The mathematical character of the problem narrows the paradigm; these operational constraints narrow the tool within it.

**The Pyomo / pymoo naming collision is misleading.** They sound similar but live at opposite ends of the structure axis: Pyomo assumes you can write the mathematics down and hands it to a solver with guarantees; pymoo assumes you can only evaluate the mathematics and explores the landscape with a population. If you ever need both in the same project, that is a signal that your problem decomposes into a structured optimisation inside a black-box outer loop — a very common pattern, and one worth recognising explicitly.

---

## Summary table

|Paradigm|Structure assumed|Typical variables|Guarantee|Scale|Key tools|
|---|---|---|---|---|---|
|Convex programming|Linear or convex, closed form|Continuous|Global optimum|Millions|CVXPY, Pyomo|
|Mixed-integer programming|Algebraic + integers|Mixed|Global (ε-optimal)|10⁵–10⁶|Pyomo, OR-Tools, Gurobi|
|Constraint programming|Discrete, relational constraints|Integer|Feasibility/optimum over search|10³–10⁵|OR-Tools CP-SAT|
|Nonlinear programming|Smooth, differentiable|Continuous|Local optimum|10³–10⁵|scipy, IPOPT|
|Stochastic gradient|Differentiable, huge scale, noisy|Continuous (θ)|Stationary point|10⁶–10¹²|PyTorch, JAX|
|Bayesian / derivative-free|Black-box, expensive|Any|Empirical, sample-efficient|10–10³ evaluations|Optuna, BoTorch|
|Evolutionary / metaheuristic|Black-box, cheap|Any, incl. multi-objective|Empirical, Pareto front|10⁴–10⁶ evaluations|pymoo, DEAP|
|Stochastic / robust programming|Uncertain parameters|Mixed|Varies by formulation|Scales paradigm chosen|Pyomo + mpi-sppy, CVXPY|
|RL / dynamic programming|Sequential, stateful|Policy|Optimal policy (in limit)|Problem-dependent|SB3, RLlib, CleanRL|