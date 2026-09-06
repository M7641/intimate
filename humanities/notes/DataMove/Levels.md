
I see it as 4 ways to process data from a movement perspective:

1. Real time pull - What their systems and do CDC 
2. Real time push - Collect Messages from them
3. Batch push - Collect large files periodically
4. Batch pull - Pull large files when we wish to from their systems.

At it's limits this would be 4 services. This would also mean that schemas would need to be shared across all 4 in a way that ensures they are all consistent. This is a hard problem in actuality as you would have to reload the schema every time you use it as one approach. If you made the schemas customer-specific, then they go out of sync too quickly; they are best being totally central. I think it would be worth being more flexible on where and how strongly you apply them. The idea being that if you apply them at their strongest levels within the apps, the data in the lake could be grander.

This leads to one of the other points I had considered, it's going to have to be flexible until you can ensure you have the power to get what you ask for. 

Either way, the schemas are a separate problem from the machinery to move the data. Doing anything else is trying to do too much. 

I'd personally start all schemas around the idea of DV2, as it's just the most robust to whatever you get. For example, if you ask for hubs, links and satellites, then in the worst case, you just get the hubs and links. You should be able to make a solution out of that. 

Slight side note, you want the contents of the hubs to be as close to reality as possible. This is a principle, but the situation I was reflecting on was returns. Ideally, you get both the original transaction and the return transaction as two separate entities, as the sum comes out as reality. With both, you can also edit them into a single row that cancels out, or do something else. That becomes a decision for the apps to make. 

The image that comes to mind is an autoencoder. In the sense that as long as you have one point where you can crush the data into the right format, then that is all you need. Therefore, I would data eng my way into the DV2 structure, then I would data eng my way into the apps. It's the most flexible and extensible thing I think of. That DV2 layer becomes the wall, and that wall then becomes the DV2 as that data format is the closest thing to reality so the apps should respect it. 

With more thinking, I'm starting to parse what would make up this structure.

1. Push service with routes for the speed. Real-time would potentially batch messages up into more mini-batch-like constructs and ingest with larger inserts, or have a cut-off that then relies on copy logic, if we ever reach that level of extreme.
2. Pull service like MCBC, which subscribes to an external source, or pulls data on a periodic scheduler. 
3. A schema service which enforces the DV2 layer - Does data reflect sound reality?
4. A second schema app-based service which handles their respective logics - does the data reflect the reality the apps need. 

Then you would have some fabric around the schema layers to connect them up, which would be app-specific. 

The counterpoint is, what if you only had one app? Would this still be a valid idea? Yes, it would, but you could argue that the extra steps are redundant and that you could just have one schema layer. I do just think it should be based on reality, though. If the reality is a stream of transactions, then that's the data model; that is the data model everyone most likely has, or will have as time erodes non-standard ways. 

Could I be bothered if we only had one app? No, I'd be gone, as that's boring as hell. So yeah, I'm not going to think about that permutation.

Back to the services, then: the two schema layers can come from the same place and the same ideas; you would just note that there are at least two layers. Internal module schemas for the apps are also a distinct possibility. This is to say that's the level of flexibility you need to plan for. 

#### Planife

I imagine six concepts that need to be worked out in order to put all of this togther:

1. Real Time Push - A route that accepts json and or any other message data structure, batches over a period of time and ingests into a warehouse.
2. Batch Push - A route that accepts files and ingests into a warehouse.
3. Real Time Pull - A route that queries a warehouse and returns data in real time. Primiarily through a subscription model and is alawys on.
4. Batch Pull - Data is pulled on a cadence.
5. Base schema validation - Is the data a reasonable reflection of reality of the source systems?
6. App schema validation - Is the data structured to be used in the applications.

Spliting schema in two so that you can have one that covers everything, but lacks the specificity needed to perfectly fit the apps.

On the data itself, I have the idea of a two sided funnel in my mind where the center of the funnel is the level we apply our base schemas to. Then the end of the funnel is the app schemas. The start of the funnel is the raw data. It would be anticptated that the data would need to have two sets of transformations the raw to our raw, then our raw to app. We could just do raw to our app if we had one app. However, we are better served having an intermediate object layer that can serve them all.
