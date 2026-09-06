
The following data entities make up a Data Valut:

1. Hubs - Hubs are businness entities that have a primary key and the date of when they were ingested into the warehouse.
2. Links - Links represent the relationships between hubs. Invoices and Products are both hubs, but invoices are descriptions of captial exchanging for products, and so there is a link between the two.
3. Satellites - Sattellites describe hubs. An example would be a table which describes the products, so it would contain the same ID as the hub, and then a series of attributes.

This approach is in comparison with Kimball's methodology which uses Star Schemas to describe the data entities. One would identify the "grain" of a data enetity and then create further tables to describe dimesions within that data enetity.

For example, for a transaction, you would have a transaction ID, the customer ID, the Product ID, the Date ID, and the store it happened in ID. You then create dimension tables which map each ID onto the relevant attributes. For example, a customer dimension table would contain the customer name, address, phone number, email, etc.

You can and will end up with nested dimensions, in the date dimension you can end up with further dimension tables describing the quaters, or the weeks of the year.

A modern approach is to use both sets of principles. Your raw data is structured as DV2 as this enables the best auditability, adatability, and scalability. Then you can use the data warehouse to transform this into the more obvious star schema.

Why is it more sclable? Consider adding a new customer meta data to your description of what a customer is. Let's also say this is coming from a new source, say it's their online behaviour from Big Query. You would then have to join this onto your fact table for customers. This means rebuilding every single row in your fact table. If you have a billion rows, this would be a lot of work. Whereas you could just add it as a staellite table and use this later in the transformation pipeline. You don't touch the previous data in the warehouse.

Also, with a DV2 structure you can indefinitly add attrribute tables to further enrich your descriptions. We have a minimal set of hubs and links needed to run a solution with the minimal attrbiutes needed. However, we then push and get it all added as satellites. DV2 is additive.

Why more auditable? You would truncate and rebuild data such as customers or products as you add new attributes or update rows. DV2 is insert-only, allowing one to avoid write locks. We would compare row hashes to check if the row already exists in the table. If it does we can ignore the new row given it's a perfect copy. If the key is different you do add it, and then you use the load_data to use the most recent in downstream processes.

We have already covered why it is more adaptable. You can easily add new staellites, links and hubs to further evole your graph of facts. You can do this witout having to re-create old data too which further adds performance.

This handles the duplication issue too. You are comparing hashs, and so you can avoid perfect duplications totally and only add the new row if the hash is different.

Hash Vs. Int Ids. Hash IDs are actually a lot faster as you don't need to find the Max to add new ones, you can just throw the data in and it will be a unqiue key. This also means you don't lock the table for writes which enables you to do a lot of parallel writes.

Therefore, a long term ingestion process would be to move data quickly into an unstructured data lake, create a DV2 base layer to build the core data eneties with hard business rules applied, you then have a DBT like process to build a Kimball like layer which is then used by applications.

## Perfect world

The desired end state is to have a set of data that will enable the applciations to make a lot of money for our customers.

### Flexibility

To begin I would split our solutions into two kinds. First are predictive models, second are exploring known state spaces. Markdown has both, the elasticities and the forecasts are predicitive, they are trying to fill in gaps in the data. Whereas the optimiser is seraching a search space for an optimal solution.

This split is made as in the case of search space seraching, you can define a ridgid data model that is required and there's no grey area. In the case of markdown it is model outputs, but if they were to be provided as inputs then it's not predictive. New columns or data determines features, and that's reasonably black and white, if you don't provide X you don't get Y. Tax for example.

The first case though is materially different. You can create a rigid data model that creates a functional model, but that is going to discard valuable information that is context specific to that business. Either, you have a massive schema which includes every permutation of attribute to describe each object, or you find a way to add flexibility to the data ingest process that enables you to collect information and that enables customers to put more data in to get better results. Or you just have an average model and be outpeformed by another vendor who will put a tiny bit of effort in to manually use the extra data to create a better model.

Given the above dichotomy is real, it then leads to the question on how do you enable that flexibility and how do you allow for undefined data. Well, you could just choose not to do predictive modelling.

Our first requirement for a perfect world is then this flexibility.

### Correctness

Given the supposed primiary value adds of our solutions are predictive modelling or state space exploration, we need the data to reflect reality. Otherwise, we are using made up information to create other made up information.

This is where the idea of data validation was incubated. Which honestly, does not make sense when you think it far enough. As the only true validation is to compare with reality. We don't have access to reality. So you can try specific a range of value, but that does not stop it from being wrong. The None Null one is intersting. I'm not sure what you say can never be null and for that to not still be potenailly true in some way. Extreme example is if there was a null value in the invoice value. It's not out landish to suggest an invoice does not have a value. An invoice is a lit of itemized goods or services that have been given to the buyer. It's only society that has determined that it's nearly always a currecny value we would exchange. It's not a total requirement.

Although, in the case of invoicing, the EU and USA have got standards for electronic invoices. Two standards we don't have to follow, but it's worth nothing a level of consistency and standardisation can start to be expected.

The point being, many of the touted validations are what we would call soft business rules rather than hard business rules. Soft business rules are not going to be consistent across companies.

The example I outlined, is in the gray area where it's not clear if it's hard or soft given so much of human culture would expect an invoice to always have a numeric currency value. This does make the not null validation intersting. I'd put it in soft and can be handled outside of the API.

Therefore, to me the question becomes what's the process to elucidate if your data does not align with reality. The best solution I've seen is to report it back to the users. Transparency in a word.

Given a DV2 structure, it's simple to count the number of unique rows, key summary metrics and the like and then report this back to the user in the form of a dashboard. It would be part of the onboarding process that the customer should validate and sign it off.

The idea of sign off is something I think will be required. I've seen it enough times now where data never seems to fully align with reality due to less than ideal business practices. A pure reflection might not be feasible.

### Structure

Applications will have core data models that require a given structure. This states we need to have the right names and types of columns. These are the only hard business rules that I can idenfity so far.

If you had the right schema and types, then the application should run. It might have nothing to say on the reality what so ever, but as a set of data structures it would work.

For example, the minimal hard rules for a recommender would be the user and product IDs. That is it. You can run a basic service with just this.

If you attempt to pretend soft rules are hard rules, then you are then lost in subjective interpretation. The hard rules enable the app to run in it's most minimal form. If features are then shut off due to missing data, then we should have this clear and part of the onboarding that certain data is required for certain features to work.

I think it's fine for data required for features to be included in the hard rules given I can see commericals including all feature sets. However, this would still just be schema and types. Just having the right data structures in place is enough to go all the way through.

### Performance

Cheap and fast.

Further, I do think there's a lot of value in the auditability too and the sclability of enabling parallel writes. However, I am also aware that these are not a priority.

To conclude that gives us 4 parameters which define good and evil:

1. Correctness
2. Structure
3. Performance
4. Flexibility

Correctness in our walled rose garden is subjective as we simply don't know anything outside of the data we are given. Therefore, there has to be an external reflection of what we have been provided that requires customer engagement.

Structure is determined by the required data entities with their schemas and types. Initially, this can be easy depending on what you are willing to enable. If it's just relational data, then you can pass the data through polars with a preloaded schema and that ticks the box immediately. There will be details to resolve in that. This is why Parquet would be the best file format as that has types and structure built in. CSV can be a bit random. Allowing non-relational would probably warrent it's own route. You would define a NoSQL schema with types and to check the types in the API, you would cast it into the langauge types which will map to a database type.

For the record, I think there's value in the idea of being able to ingest NoSQL data, as it could be easier for users who are more engineer than data practitioners. No idea how you handle many to many though.

Peformance. The obvious bottle necks are bandwidth of moving the data across the web to start. Then having an API that can parse the data fast enough. Then desiging the ingest so that the API does not get stuck waiting on the database to accept data. From there you can delegate to workflows and other distinct compute units which don't obstrcut the process of getting data in.

Therefore, you should really use a background process to manage the data warehouse ingest, and instead the API should send to the datalake and regsiter a job from which the backroung process can pick up. I would try put this backround process in the API as well as it only needs to get the async right and it should handle it. Also worth noting we would be using COPY commands here which then enables much faster write speeds than the insert commands as the warehouses we have can run the first over many nodes. You can develop the idea by inserting small amount of data and only copying after a point. If we wanted to live stream data in, we would want to review that idea and look at what services such as Kafka are doing to acheive this. I would guess they collect data over a period of time and then copy it into the warehouse.

On the parsing, this is the only thing that should bottle neck the API, also the beahviour of large files and what they do to the API too if it's waiting to get packaets from the network or if it does something else. Honestly, having batch sizes problably stops this ever being an issue in both the parsing and how the api responds to large files. I am also of the impression that the network breaks first before parsing is a bottle neck. Even in Python, modern data frame tools can handle that much.

Flexibility. I suggest that there can be a core data model that enables the apps to function, not nessisarily thrive, but function. I'd want to see this be minimal as possbile, then we just make it flexibile to extend it. We can also just lie and say our requirements are bigger than they actually are. Human problem at that point though.

Given a core model. If we used the DV2 stucture, then we enable adding bespoke satellites. The Hub is specified so then subsequent joins are trivial. There would be a core satellite, and rather than chaging the schema of the core, we would just allow the user to ingest more data with the specified hub.

We then avoid having to manage schemas in a dynamic way. There would need to be a process on how to manage chagning satellites, but initially, just drop and rebuild them.

The final key bit is where do you draw the finish line. I'd say it's with a DV2 model. From there you start incrementally building a UDM between applications, at least to the point of shared data entities. You can also just let the apps build their own marts tables from the staging DV2 structure. We can build the reporting on the core model, and I am sure there will be away to automatically generate the reporting on bespoke satellites. Even if it's just sums, distincts, etc.

## Extra bits

You can't get duplicates in this system. I'm not sure what part acutally blocks duplications in the data warehouse, and if there's a way to get a copy command to do that. However, worst case you can do clean up on the row hash.

API rejections would just be schema and type errors, the rest would come through other means.

I'd have this by tenant. Seperate compute resource and the data does not cross paths which might not be real problems, but I know they absolutely would not be if it was decentralised to tenant level resources. Saying that, they all have to go through the same AWS gateway so the crossed paths is not going to change.

Parsing in an API should be done by converting the value into the languages natural type. Would be worth comparing approaches between using Polars. I'd say we don't need polars now given the regression of the validations. Saying that I really like the reading of the file types being that easy. Worth a play. It's fine for now though.

I also know that the vast majority of people won't ever want to move to DV2 given the nature of it being a subtle abstraction that saves pain down the line. However, it would be for the best. There is potentially a way to convert the current asked for data into the three data entities which is likely the only middle ground that could work.

### Resources
1. [Practical Introduction to Data Vault Modelling](https://medium.com/@nuhad.shaabani/practical-introduction-to-data-vault-modeling-1c7fdf5b9014)
2. https://medium.com/@lsleena/designing-a-schema-agnostic-etl-system-de92b8a04435

Upon looking into data warehouse design, I came across patterns associated with the name Data Vault 2.0 (DV2). It's atomic, but I am really liking the idea.

Within this paradigm, every table is one of three kinds:
1. Hub - The business key of the fact and when it was ingested into the warehouse.
2. Link - How hubs relate to each other, this is a series of the business keys and also when this link was ingested into the warehouse. 
3. Satellite - The description of that business key. For example, if it's a customer, it would be the customer metadata. If it's an aeroplane flight, then it could be flight information, such as how long it took. 

I like this structure because it's graph-based, and you can just organically add hubs and links over time. You can add new satellites as well, even if it's harmless to change the base object, I would imagine.

It creates a lot of tables, and I think you could argue that the satellite and the hub could be one and the same. However, your multiple satellites could come from different systems, and if one changes, you only need to update that satellite. That's far less data to update, and it's less exposed to damaging other parts of the graph. 

Plus, you can always create an abstraction on top that acts as that single table. 
