# Value of Software

Software has always been the separation of _instructions_ from the _machine that executes them_.

Every major leap in value comes from making that separation cheaper, more flexible, and accessible to more people.


## The starting place

https://en.wikipedia.org/wiki/Jacquard_machine

https://www.youtube.com/watch?v=J49TTfG3H8w

The Jacquard machine was the first machine to implement instruction sets. The first software to exist. 

The machine was controlled by a "chain of cards"; a number of [punched cards](https://en.wikipedia.org/wiki/Punched_card "Punched card") laced together into a continuous sequence. Multiple rows of holes were punched on each card, with one complete card corresponding to one row of the design.

You can go further back with Basile Bouchon https://en.wikipedia.org/wiki/Basile_Bouchon. 

The punch cards represent binary; each represents either a 1 or a 0. 

The economic value of this was that it made it easy to replicate weaving patterns that previously took a lot of time from skilled practitioners. You reduced the skill required to make the same output. 

Logic became a tradable, copyable asset.

The first time "running instructions on data" became a real industry was with Hollerith's tabulating machines for the 1890 US Census. Hand-tabulation of the 1880 census had taken most of a decade; Hollerith's punched-card system cut it to a couple of years. That company became IBM. So the first genuine software-shaped economic value was **data processing** — taking a clerical task that scaled linearly with population and bending the cost curve.

This data would then go on to be used for: 

- **Resource Allocation:** Governments use census data to determine funding for public services such as healthcare (hospitals), education (schools), and infrastructure (transportation and housing). Then, even going back to manpower for war. 
- **Legislative Representation:** Census results are used to draw electoral boundaries and determine the number of seats a region has in the legislature. 
- **Public Policy & Planning:** Local authorities and national policymakers use demographic data (such as age, employment, and marital status) to plan for future trends, such as an ageing population or growing communities. 
- **Business & Research:** Private companies and researchers use the data to identify market needs, such as where to open new shops or establish community organisations. 

This is something we would have done anyway, but having a machine to implement consistent instructions led to a massive decrease in the time required to process the data. Today, we could run a census in near-real time, as the information-processing machinery enables it. 

## ENIAC

https://en.wikipedia.org/wiki/ENIAC 

was the first [programmable](https://en.wikipedia.org/wiki/Computer_programming "Computer programming"), [electronic](https://en.wikipedia.org/wiki/Electronics "Electronics"), general-purpose [digital computer](https://en.wikipedia.org/wiki/Digital_computer "Digital computer"), completed in 1945.

The need for this came from ballistics tables https://ammo.com/ballistics. If you wanted to work out where artillery would land, you would compute the landing point by using calculus. 

https://ftp.arl.army.mil/~mike/comphist/48eniac-coding/sec1.html

The first wave was _computation for science and war_ — ballistics tables, codebreaking (Colossus at Bletchley), nuclear weapons simulation. High value, narrow audience, funded by states.

The second, much larger wave was _business data processing_: payroll, billing, inventory, and general ledger on IBM mainframes. This is where software first paid for itself at scale in the private economy, and the mechanism was simple — it replaced armies of clerks performing repetitive bookkeeping. The value proposition was automation of routine labour.

The third wave is the subtle one: software _about_ software. Assemblers, then FORTRAN (1957) and COBOL (1959), then operating systems. None of these directly affected the end customer. Their value was that they lowered the cost of producing the first two kinds of software and widened the pool of people who could write it. This is the recurring engine of the whole industry — meta-tooling that compounds.

So if you want the first cut of the abstraction, across all of this, the _why_ of software has been remarkably stable. It does one of a few things:

1. It **substitutes** for human labour, 
2. **augments** human capability, 
3. **coordinates** between people, 
4. or **creates** a new medium. 

Its economic specialness is a property the loom already hinted at: once written, logic copies at near-zero cost, and increasingly _executes_ at near-zero cost too. That combination — zero marginal reproduction plus composability — is what makes software unlike almost any prior economic good.

The natural next era is the one where that "software about software" engine explodes outward — abstraction layers stacking on abstraction layers (high-level languages → applications → operating systems as platforms → networks), and value shifting from _automating existing tasks_ to _creating things that couldn't exist before_.

AI's initial augmentation is that it has sped up and reduced the cost of encoding logic into software. 

## Machine Learning

The role of machine learning in this equation is that there are many sets of logic that we've never been able to work out. We know the elliptical nature of firing an object across a field. However, we do not know what the demand will be tomorrow. There is no function we can define for this, given the infinite number of variables. 

Hence, Machine Learning exists to automatically evaluate a function that maps inputs onto outputs. A learned instruction set that no human ever defined.

An instruction set is ultimately created, but the process is unique, as no human has ever made one.

However, the core value remains: An instruction set to achieve a goal that would require way more cost if done only through humans. 

## Future Value

Value comes from creating instruction sets to do things that people like.
### Proto use cases

The initial instruction sets we spent lots of resources on were: 

1. **Mastery of space**  - **Astronomical and navigational tables.** This was probably the single largest computational enterprise before the 20th century. Predicting where the moon, planets and stars would be, published as ephemerides and almanacks. The driving need was the longitude problem — a ship at sea could easily determine its latitude but had no reliable way to determine its east–west position, and ships full of men and cargo were routinely lost on rocks because of it. Nations offered enormous prize money (as in Britain's Longitude Act, 1714)[https://en.wikipedia.org/wiki/Longitude_Act]  for a solution. The _Nautical Almanac_ was computed by hand by a distributed network of human computers. Astronomy also fed calendars and, frankly, astrology and religious timekeeping, which were taken seriously as state and spiritual functions.
2. **Mastery of time and risk** - **Actuarial and mortality tables.** Halley (yes, the comet one) built one of the first mortality tables in 1693. The why is the birth of _insurance and annuities_ — pricing the risk of a life, a ship, a cargo. Lloyds of London, life insurance, pensions: an entire branch of the economy that exists only because someone can compute an expected value over a population. This is also where modern probability was invented — the demand created the math.
3. **Mastery of the social ledger**- **Tide tables, tax rolls and land surveys, banking/interest tables, and military logistics** (paying, feeding, supplying armies — arguably the original enterprise-resource-planning problem). The Domesday Book (1086) is a census _and_ a cadastral survey rolled into one. Pascal built his mechanical calculator, the Pascaline, specifically to help his father do _tax_ computations. Cryptography and codebreaking were done by hand for diplomacy and war long before Bletchley. 
	1. This is the trust problem: it's what lets strangers transact, lets a state extract resources, lets a partnership survive a disagreement. Double-entry bookkeeping (Italy, 1400s) is arguably as important to capitalism as the steam engine.

The unifying answer to "why did we do these things" is that each one let human coordination scale past what a single mind or a handshake could hold. As soon as you have oceans to cross, populations to tax, armies to feed, and risks to pool across thousands of people, you _must_ externalise computation and memory onto paper and tables — because the unaided human can't track it, and getting it wrong is catastrophic.

And here's the hinge for where we're going: these use cases didn't just precede the computer, they _summoned_ it. Babbage designed his Difference Engine precisely because hand-computed navigation and astronomy tables were riddled with errors, and those errors sank ships. The demand for these high-value computations was so intense, and human computers so slow and error-prone, that mechanizing them became inevitable. The proto-use-cases are the gravitational mass that pulled the machine into existence.

## Our use cases

The areas Peak has traditionally focused on are those with elements of Machine Learning. These are areas that lack well-defined logical instructions. In fact, this is an area we have traditionally been poor at: implementing clean instruction sets.

It's easier to think of Peak as a company that had a hammer called ML, and then sought out nails to hit with it. It's this which best explains how we have ended up with the current solution spaces.

Inventory: the missing instruction set is the set of instructions that tells you how much stock you will need to have on hand to maximise profit. Parts of this instruction set are defined. When we know the distribution of demand, we can define a set of instructions to achieve the desired stock. 

Pricing: the missing instruction set is the one that defines the perfect price for a product. Perfect is the one that would yield the highest return on the stock you already have. You don't know what demand will be, nor do you know how changing the price affects demand. If you had these two as simple instruction sets, the solution would be trivial. However, again, there are infinite variables at play, and it's not something we are ever going to define. 

Customer: The desired instructions are a set that tells you when a customer will churn, for example. The problem with this one is that knowing when a customer will churn is not necessarily useful for stopping that churn. 

Given the above, it's clear to me that improving our solutions is about improving the instruction sets. To improve the instruction sets, you need more relevant data, more data describing the changing variables. I would put guardrails in this, too; guardrails are extra artefacts of data that help define the instruction set. 

Therefore, our objectives should be to work on things that improve our instruction sets:
1. Developing our models so that they can better learn the instruction sets latent in the data.
2. Providing more data.
3. Increasing the efficiency of the instruction set whilst maintaining correctness. 
4. Enabling more data through guardrails to be implemented in the instruction set. 

## Future use cases

New use cases would be converting previously uncovered instruction sets into repeatable logic that a machine can run. 

I feel like this can be broken into three different areas:
1. Using machines we currently have, but no one has created the instruction set.
2. Using machines that will exist in the future with previously made instruction sets. 
3. Using machines that will exist in the future with instruction sets no one has created yet.

Robotics, for example, will be an area that needs development.

Either way, a progressive strategy is to invest in the instruction sets we can provide right now and to dedicate resources to identifying new ones as the physical technology refines.

This does raise an interesting question about the use of LLMs to generate instruction sets: Why did you not do that before? 

You can create a new dashboard based on a prompt, but why did you not make that dashboard before? 

You can create applications to improve processes, such as time logging or reporting, but why did you not do that before? 

The answer is likely grounded in the notion that it was not worth the resources to do previously. The use case is actually just not very valuable.

We will experience what the physical world experienced when China started to provide cheap industrial capacity. We can get many more things and more cheaply, but the value of each good does degrade. 

The issue is that this is not the physical world. Copying is free. Therefore, you would only ever need a limited number of suppliers to supply all of the demand forever. 

Therefore, you will have to become one of those suppliers or find a different profession. 

The most likely to me at this point is personalised software. Would B&M and Debenhams have the same instruction sets for their demand? No, they would not, so there is uniqueness there. Could one software product deliver for both? 

It's worth being careful about what's become essentially free here. It's the ability to implement instruction sets, not actually develop and maintain them. The same way that not every company became masters of the fields they live in just because of all the information on the internet, they still needed to digest this and then act on it in their own circumstances. 

It is also true that not everyone will know the best way to solve a problem. An AI certainly helps collect the information and make good decisions, but it will first need to be asked the question.

The conclusion this leads me to is that it sure as hell won't be enough to be good at software in the world of tomorrow. You have to be good at understanding instruction sets and knowing when to use each one. I would want to be both. The prioritisation then becomes based on what improves the implementation of the instruction set, or the instruction set. 

A task like making a weekly report does neither. Its value is a communication piece to buy us time to implement an instruction set. If it took a day to implement an instruction set, there would be no weekly report to do. Therefore, it's something we should look to make exciting. 

I do think you can apply a similar framework to video games in a way as well. They are an instruction set for creating a virtual world for entertainment purposes. The real value will be what virtual world you create.

In a way, that's what we do: create a virtual world. Just for us people, then take from that virtual world to change the physical one. 
## Further notes

**Logarithm and trig tables.** Napier's logarithms (1614) were a meta-invention — they turn multiplication into addition. This is the proto-version of the "software about software" layer: a tool whose entire value is making all the other computations cheaper.

### The world Today 

Now map it onto the three masteries we ended on — space, time/risk, and the social ledger — and something jumps out immediately: **the overwhelming majority of pure SaaS is the social-ledger lineage.** It's the account book and the census, reborn.

**The system of record — the ledger made live and multiplayer.** Salesforce is a ledger of customer relationships. Workday is the census of your workforce. ServiceNow is a ledger of tickets and processes. Xero, QuickBooks, NetSuite are literally the account book. HubSpot, the contacts book. What every one of these sells is the same thing the 1400s Italian merchant and the Domesday surveyor wanted: an authoritative record of who has what and who owes whom. The reason they're such ferociously good businesses is that once your data lives there, leaving means amnesia — the switching cost compounds with every row you add. They are the digital descendants of double-entry bookkeeping, and they inherit its centrality to commerce.

**The system of engagement — coordination as a product.** Slack, Zoom, Teams, email, and the workflow layer of Asana, Notion, Linear. These descend from the _coordinate_ verb — the proto-versions are the postal system, the telegraph, the meeting room. You pay them to move information between people and keep them aligned. Their distinctive economic feature is network effects: Slack is worthless to one person and indispensable to a company of ten thousand.

**The system of intelligence — foresight as a product.** This is the actuarial table and the almanac lineage — mastery of time and risk. Tableau and Looker tell you what happened; Datadog and the observability tools tell you what's happening and warn you before it breaks; fraud, forecasting, and underwriting tools price the future. Halley's mortality table is the ancestor of every analytics dashboard. You're still paying for foresight, because the alternative is still ruin.

**The system of creation — augmenting the act of making.** Figma, Canva, Adobe, GitHub, Webflow. This is the one that _doesn't_ fit the three masteries cleanly, and that's worth noticing — it's a fourth lineage, the augmentation of craft. The proto-version isn't a table of numbers, it's the drafting board, the workshop, the printing press. These tools don't help you master space, time, or the ledger; they make the human act of producing something faster, better, and collaborative.

And notice what's _missing_: pure SaaS for **mastery of space** is rare, because space means atoms — ships, trucks, warehouses. The moment you need to touch the physical world (Uber, Flexport, logistics), you stop being pure SaaS. Purity correlates with distance from matter.

## What does AI change

For two hundred years — from the Jacquard card to the SaaS workflow — software could only execute _logic a human specified in advance._ The punched card, the COBOL payroll run, the Salesforce automation: all of them are pre-written rules. The defining limitation of every piece of software in our entire history is that **someone had to anticipate the case and write the rule for it.** Software was exact, brittle, and blind to anything its author didn't foresee.

AI breaks precisely that constraint. It is the first software that does what no one specified — it handles ambiguity, reads unstructured input, interpolates into cases never enumerated, and produces novel output. The instructions are no longer written ahead of time; they're learned and applied at runtime. So the one-line version: **software used to do what we told it; AI does what we didn't.** Everything downstream flows from that.

**It adds a fourth property to the SaaS value triad.** The old triad was utility + multiplayer + instant distribution — but the software still did _the same thing for everyone_, like a loom running one card. Every prior abstraction (compilers, then SaaS) drove down the cost of producing and shipping _fixed_ logic. AI drives down the cost of producing _bespoke_ logic, per case, at runtime. Customization used to require human labor — armies of consultants configuring SAP. Now the marginal cost of a tailored answer approaches zero too. That's the genuinely new economic property: **mass customization of logic itself**, not just mass distribution of frozen logic.

**AI is probabilistic; the old software's whole value was being exactly right every time.** A ledger that hallucinates is worthless. So AI's value lands first where _approximately right_ is genuinely useful

So, **what to prioritise building today**, derived rather than asserted: build the _action_ layer, not the thousandth dashboard — close the loop instead of just reporting. Build the _verification and trust_ scaffolding, because that bottleneck is what's keeping AI out of the high-value ledger domain. Get as close as you can to _authoritative data and the right to act_, because that's the moat that survives when workflow and UI commoditize. Design for _intent over interface_ — the UI is collapsing toward "say what you want." And rethink the _business model_: per-seat pricing assumes humans operating software; if agents do the work, value decouples from seats and moves toward outcomes and work-done.

A ledger just records; a judgment carries _values_. When you externalise arithmetic, the only risk is an error. When you externalise judgment, you also externalise a stance about what matters — and the machine's judgment is only ever as good, and as aligned, as what we managed to build into it. So the deepest "what should we prioritise" isn't which app to build. It's deciding _which judgments we actually want to hand over_ — because for the first time in this whole history, the thing doing the deciding isn't us.
