
What would a replenishment engine look like for perishable goods and goods in general? One would be a demand reduction of stock, the other is a forced end of stock, where you would want to minimise waste. You only need to find the optimal for this part, though, rather than anything else.

A forecast is one element of this, and then you would need to manage the time until you can refresh your supply.

Where do you start this, though? 

1. Current inventory at a single location.
2. Forecast demand.
3. Evaluate the future levels of stock given the demand and the other forces that play, such as yield and expiry.
4. Identify the point in time where you would reorder that product. Point in time determined by the point where 95% of the time you would maintain stock to be saleable. 
5. You would also look to maximise profit, but not sure where that would come in. 


# Replenishment Engine — Mathematical Specification

**v0.1 — Working Draft | March 2026**

*Abstract, distribution-agnostic formulation for optimal inventory replenishment under uncertainty.*

---

## 1. Problem Statement

Given a single product at a single location, the engine must answer two questions: when should a replenishment order be placed, and how many units should be ordered? The answers must be economically optimal under demand uncertainty, perishability, and operational constraints, without presupposing the shape of the demand distribution.

The engine treats the service level not as a fixed input but as a derived quantity that emerges from the economic trade-off between the cost of holding excess inventory and the cost of failing to meet demand. This unifies availability targets and profit maximisation into a single framework.

---

## 2. Notation and Definitions

| Symbol | Meaning |
|--------|---------|
| *D* | Random variable representing demand per unit time |
| *L* | Lead time: elapsed time from order placement to receipt of goods |
| *R* | Review period: elapsed time between successive inventory inspections |
| *W = L + R* | Exposure window: the total period the current inventory must cover |
| *μ_W* | Expected (mean) demand over the exposure window W |
| *σ_W* | Standard deviation of demand over the exposure window W |
| *F_W(·)* | Cumulative distribution function (CDF) of demand over W |
| *F_W⁻¹(·)* | Quantile function (inverse CDF) of demand over W |
| *C_u(x, t)* | Cost of underage: cost incurred per unit of unsatisfied demand, as a function of shortage magnitude x and time t |
| *C_o(x, t)* | Cost of overage: cost incurred per unit of excess inventory, as a function of surplus magnitude x and time t |
| *q\** | Optimal target quantile (derived service level) |
| *r* | Reorder point: inventory level at which an order is triggered |
| *Q* | Order quantity |
| *S* | Order-up-to level (for periodic review policies) |
| *K* | Fixed cost per order (administrative, transport, receiving) |
| *h* | Holding cost per unit per unit time |
| *η* | Yield factor: proportion of ordered stock that becomes saleable (0 < η ≤ 1) |
| *τ* | Remaining shelf life of product at time of receipt |

---

## 3. Cost Structure

The cost structure is the foundation from which the target service level is derived. Both underage and overage costs are modelled as functions rather than constants, allowing the engine to capture real-world non-linearities.

### 3.1 Underage Cost Function

The underage cost function *C_u(x, t)* represents the total cost incurred when demand exceeds available inventory by *x* units at time *t*. This function captures:

- **Lost margin:** the gross profit forfeited on each unsold unit. This is the baseline and is typically linear in x.
- **Customer goodwill:** the long-term revenue impact of a stockout. This may be convex in x if large stockouts cause disproportionate brand damage.
- **Contractual penalties:** any fixed or graduated penalties triggered by service failures. These introduce step functions or thresholds.
- **Substitution effects:** if the customer switches to a competitor product, the cost includes the probability-weighted lifetime value of the lost customer.

The time dependence allows the cost to vary seasonally or by day-of-week. For example, a stockout on a peak trading day may carry a higher underage cost than one on a quiet Tuesday. In the simplest case, *C_u* collapses to a constant *c_u* per unit, but the engine does not assume this.

### 3.2 Overage Cost Function

The overage cost function *C_o(x, t)* represents the total cost incurred when inventory exceeds demand by *x* units at time *t*. This function captures:

- **Holding cost:** warehousing, insurance, capital opportunity cost. Typically linear in x and proportional to time held.
- **Waste (perishables):** for products with shelf life τ, any unit still in stock at expiry has overage cost equal to its full landed cost. This introduces a hard boundary: C_o jumps discontinuously when remaining shelf life reaches zero.
- **Markdown cost:** if excess stock is sold at a discount rather than wasted, the overage cost per unit is the difference between the full price and the markdown price.
- **Disposal cost:** any cost associated with physically removing unsaleable stock.

For perishable goods, the overage cost is strongly time-dependent because the cost of one excess unit increases as the product approaches its expiry date. The engine models this by allowing *C_o* to be evaluated at any point in the product's remaining life.

### 3.3 Marginal Cost Ratio

At any given inventory position, the decision to stock one additional unit is governed by the marginal economics. The expected marginal cost of understocking by one unit is the underage cost weighted by the probability that the unit would have been demanded. The expected marginal cost of overstocking by one unit is the overage cost weighted by the probability that the unit would not have been demanded.

Define the marginal cost ratio at inventory level y as:

> *ρ(y, t) = E[C_u(1, t)] / ( E[C_u(1, t)] + E[C_o(1, t)] )*

When costs are constant (the simple case), this reduces to:

> *ρ = c_u / (c_u + c_o)*

When costs are functions, the expectation is taken over the relevant time horizon and demand scenarios, which the engine evaluates numerically.

---

## 4. Deriving the Target Quantile

The classical newsvendor result establishes that the optimal stocking level is the one at which the CDF of demand equals the critical ratio. The engine generalises this to functional costs.

### 4.1 Constant-Cost Case

When underage and overage costs are constants c_u and c_o respectively, the optimal target quantile is:

> *q\* = c_u / (c_u + c_o)*

This is the well-known critical fractile. When c_u ≫ c_o (high-consequence items where stockouts are very costly), q* approaches 1.0 and the engine prescribes heavy safety stock. When c_o is large relative to c_u (short-life perishables where waste dominates), q* pulls back well below 0.95, and the engine accepts more frequent stockouts to avoid waste.

### 4.2 Functional-Cost Case

When costs are functions of shortage/surplus magnitude and time, the optimal inventory level y* is the solution to the integral equation:

> *∫₀^∞ C_u′(y\* − d, t) · f_W(d) dd = ∫₀^∞ C_o′(d − y\*, t) · f_W(d) dd*

where *C_u′* and *C_o′* are the marginal cost functions (partial derivatives with respect to shortage and surplus respectively), and *f_W* is the probability density function of demand over the exposure window.

This equation states that at the optimum, the expected marginal cost of adding one more unit of safety stock exactly equals the expected marginal benefit of avoiding one more unit of shortage. The engine solves this numerically using bisection or Newton's method over the inventory level y, evaluating the integrals via quadrature or Monte Carlo sampling depending on the demand representation provided.

The derived service level is then read off as *q\* = F_W(y\*)*, the probability that demand does not exceed the optimal inventory level. Note that q* is an *output* of the optimisation, not an input.

### 4.3 Override Mechanism

The engine allows the consumer to override the derived quantile with a fixed target (e.g., a contractual 95% service level). When an override is active, the engine skips the cost optimisation in this step and uses the supplied value directly. However, it still reports what the economically optimal quantile would have been, so that the consumer can see the gap between the contractual obligation and the economic optimum.

---

## 5. Demand Characterisation

The engine is agnostic to the form of the demand distribution. It requires only that the demand model satisfies a minimal interface contract.

### 5.1 The Quantile Contract

The demand model must expose a quantile function:

> *F_W⁻¹ : [0, 1] → ℝ≥0*

Given a probability *q* in [0, 1], the function returns the demand level *d* such that *P(D_W ≤ d) = q*. This is the sole mandatory interface. The engine never needs to know the parametric form of the distribution, only the answers to quantile queries.

### 5.2 Extended Contract (for Functional Costs)

When the engine uses functional costs (Section 4.2), it additionally requires the ability to evaluate expected costs over the demand distribution. This can be provided via:

1. **A density function** *f_W(d)* that the engine integrates against the cost functions numerically.
2. **A sample generator** that produces independent draws from the demand distribution, enabling Monte Carlo evaluation of the cost integrals.
3. **A moment generator** that returns the mean, variance, and optionally higher moments, allowing the engine to use analytical approximations where the cost functions permit.

At least one of these must be available when functional costs are in use. For the constant-cost case, only the quantile function is needed.

### 5.3 Exposure Window Aggregation

Demand models are typically calibrated at a base time granularity (e.g., daily demand). The engine must aggregate to the exposure window *W = L + R*. If daily demands are independent and identically distributed with mean μ and variance σ², then:

> *μ_W = W · μ*
>
> *σ_W = √W · σ*

If daily demands are autocorrelated (e.g., a busy day tends to be followed by another busy day), the variance aggregation must account for the covariance structure. The engine does not prescribe how the demand model handles this internally; it only requires that the quantile function for the full window W is provided.

---

## 6. Reorder Point Computation

The reorder point r is the inventory level at which the engine triggers a replenishment order. It is computed as:

> *r = F_W⁻¹(q\*)*

This single expression encodes both the expected demand during the exposure window and the safety buffer. Decomposing it:

> *r = μ_W + SS*

where SS = *r − μ_W* is the safety stock. Under a normal demand assumption, this simplifies to SS = *z · σ_W* where z = Φ⁻¹(q*) is the standard normal quantile. But the engine does not require normality; the quantile function handles arbitrary distributions transparently.

The engine evaluates r in the units of the demand model (typically individual sellable units) and rounds up to the nearest integer, since fractional units cannot be stocked.

---

## 7. Order Quantity Determination

The engine supports two canonical replenishment policies. The choice between them depends on the operational context and is a configuration parameter.

### 7.1 Continuous Review: (r, Q) Policy

Under continuous review, the inventory position is monitored at all times. When it drops to or below the reorder point *r*, a fixed quantity *Q* is ordered.

The economically optimal *Q* balances the fixed cost of placing an order against the holding cost of carrying cycle stock. The classical Economic Order Quantity is:

> *Q\* = √(2 · D̄ · K / h)*

where *D̄* is the expected annual demand rate, *K* is the fixed order cost, and *h* is the per-unit per-year holding cost. This assumes deterministic demand for the sizing of Q; the stochastic element is handled entirely by the reorder point r.

### 7.2 Periodic Review: (r, S) Policy

Under periodic review, inventory is checked every *R* time units. If the inventory position is at or below *r*, an order is placed to bring the position up to a target level *S*. The order-up-to level is:

> *S = F_W⁻¹(q\*) where W = L + R*

Note that *S* and *r* use the same quantile function but potentially different exposure windows. In the (r, S) policy, the exposure window explicitly includes the review period R, whereas in (r, Q) under truly continuous review, R = 0 and the window is just the lead time L.

The actual order quantity at each review is:

> *Q = S − IP*

where *IP* is the current inventory position (on-hand + on-order − backorders).

---

## 8. Constraints and Adjustments

The raw outputs from steps 4–7 are adjusted by a pipeline of composable constraint transforms. Each transform takes a proposed order quantity and modifies it to satisfy one operational constraint. The transforms are applied sequentially, and their order may matter (e.g., rounding to a case multiple should happen after shelf-life clipping, not before).

### 8.1 Minimum Order Quantity

If the supplier imposes a minimum order quantity *Q_min*:

> *Q′ = max(Q, Q_min) if Q > 0, else 0*

The zero case ensures we do not order when no replenishment is needed.

### 8.2 Pack Size / Case Multiple

If orders must be placed in multiples of a case size *M*:

> *Q′ = M · ⌈Q / M⌉*

Rounding up biases toward overstocking rather than understocking, which is consistent with the service-level objective. The engine reports the effective service level after rounding so the consumer can see the impact.

### 8.3 Shelf-Life Clipping (Perishables)

For products with remaining shelf life *τ* at time of receipt, the maximum quantity that can be sold before expiry is bounded by the expected demand over the remaining life:

> *Q′ = min(Q, F_τ⁻¹(q_o) − I_onhand)*

where *F_τ⁻¹(q_o)* is the quantile of demand over the shelf-life window at a probability *q_o* chosen to limit waste to an acceptable level, and *I_onhand* is the current on-hand inventory. This prevents the engine from ordering more than can reasonably be sold.

### 8.4 Yield Adjustment

If a proportion of ordered stock is lost to damage, shrinkage, or quality rejection (yield factor *η < 1*), the order quantity is inflated:

> *Q′ = ⌈Q / η⌉*

This ensures that the expected number of saleable units received equals the intended Q.

### 8.5 Supplier Capacity

If the supplier has a maximum available quantity *Q_max* for the order window:

> *Q′ = min(Q, Q_max)*

When this constraint binds, the achieved service level will be lower than the target. The engine reports this degradation.

---

## 9. Decision Output

For each product evaluated, the engine emits a decision record:

| Field | Description |
|-------|-------------|
| `product_id` | Unique identifier for the product |
| `reorder_point` (r) | Inventory level that triggers an order |
| `order_quantity` (Q) | Number of units to order (after all constraint transforms) |
| `raw_order_quantity` | Order quantity before constraint adjustment |
| `derived_service_level` (q*) | The economically optimal quantile |
| `effective_service_level` | Achieved service level after constraints (may differ from q*) |
| `override_active` | Boolean: whether a fixed service-level override was used |
| `safety_stock` (SS) | r − μ_W, the buffer above expected demand |
| `next_review_time` | When the engine should next evaluate this product |
| `binding_constraints` | List of constraints that modified the raw order quantity |

This record provides full transparency into the engine's reasoning. The separation of raw and constrained quantities, together with the binding_constraints field, allows the consumer to understand exactly why the engine recommended a particular order size and where the constraints bit.

---

## 10. Feedback Loop

After each replenishment cycle completes, the engine ingests realised outcomes and updates its internal state.

### 10.1 Demand Model Calibration

Observed demand *d_obs* is compared against the predicted distribution. The engine tracks the empirical quantile of realised demand within the predicted distribution. If the demand model is well-calibrated, realised demands should fall at quantile q* or below approximately q* of the time. Systematic deviations indicate model mis-specification: if actual demand frequently exceeds the predicted q*-quantile, the demand model is underestimating variance or location.

The engine computes a probability integral transform (PIT) score for each observation:

> *p_i = F_W(d_obs,i)*

If the model is correct, the sequence {p_i} should be uniformly distributed on [0, 1]. The engine can apply a Kolmogorov–Smirnov test or track the empirical CDF of {p_i} to detect systematic bias and flag products whose demand models need recalibration.

### 10.2 Cost Parameter Recalibration

The cost functions may need updating as the business learns from experience. If stockouts prove less damaging than assumed (customers wait rather than churn), c_u should decrease, pulling q* down and reducing safety stock. Conversely, if waste costs are lower than expected (markdown channels absorb excess efficiently), c_o decreases and q* increases. The engine exposes hooks for the consuming system to adjust cost parameters based on observed outcomes.

### 10.3 Achieved Service Level Tracking

The engine tracks the realised cycle service level (fraction of replenishment cycles completed without stockout) and the realised fill rate (fraction of total demand satisfied from on-hand stock). These are compared against the derived target q* and reported in the engine's monitoring output. Persistent gaps between target and realised performance indicate either demand model error, constraint effects (e.g., supplier capacity frequently binding), or lead-time variability not captured in the current W estimate.

---

## 11. Computational Architecture Notes

The engine's core is deliberately minimal. The critical path is: derive q* from costs, evaluate F_W⁻¹(q*), apply constraints. Everything else is input preparation or output enrichment.

In Rust, the demand model would be expressed as a trait with a single required method — `fn quantile(&self, q: f64) -> f64` — plus optional methods for density evaluation and sampling. The cost model is a second trait with methods for marginal cost evaluation. The engine's core function is generic over both traits, making it testable with simple mock implementations and extensible to arbitrary demand and cost models without modifying the core logic.

The constraint pipeline is a vector of closures or trait objects, each with the signature `(Q, context) → Q′`, applied in sequence. This makes it trivial to add, remove, or reorder constraints without touching the optimisation logic.

Numerical routines (bisection for the functional-cost case, quadrature for integral evaluation) should be implemented as standalone, well-tested utilities. The engine should not depend on external numerical libraries unless they provide a clear advantage in performance or correctness over a purpose-built implementation.
