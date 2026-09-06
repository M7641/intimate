# Recommender

https://www.tensorflow.org/recommenders/examples/quickstart?_gl=1*kfk8hi*_up*MQ..*_ga*MTU3NTY4Nzg4Mi4xNzU0MTY3ODY1*_ga_W0YLR4190T*czE3NTQxNjc4NjUkbzEkZzAkdDE3NTQxNjc4NjUkajYwJGwwJGgw

https://cloud.google.com/blog/products/ai-machine-learning/scaling-deep-retrieval-tensorflow-two-towers-architecture

[code](https://github.com/keras-team/keras/blob/v3.3.3/keras/src/layers/core/embedding.py#L14-L427)

## Questions for myself

1. What is the impact of normalisation?

2. What's the best way to combine representations in a model? I've only seen concat so far.
   - Weighted averaging where you weight the more valuable features more.
   - Late fusion is a third way where you pass the embeddings through another model to see which has the most predictive power, then you can use that to weight the features.
   - Model stacking, chaining transformers together.

3. A deep review on of the metrics work and how I can implement them manually.

4. Learn one of the algorithms for nearest neighbour search.

5. Do our use cases suit list wise ranking more?

6. How would we host the model in an API and for it to be performant?

7. Playing with quantization if possible.

8. Other ways of doing retrieval - Finding categories they buy frequently in and rank within those categories.

### Calculating similarity

The scores are the embeddings `matmul` together. The labels are `tf.eye(queries, candidates)`.

Labels are therefore an identity matrix with queries rows and candidates columns.

Queries and candidates are both 32 dimensional vectors at this point, or what ever input dimension you have done. There could be a case where they don't have the same dimensions.

1. Take two embeddings, A and B.
2. Calculate the scores as `scores = A B^T`. This is the dot product.
3. Take the labels to be an identity matrix of the same size as the scores. If the batch size is 2 where we have two queries and two candidates, then the labels are `[[1, 0], [0, 1]]`. First element of this shows a hit with the first query and first candidate, and not hitting the second candidate. Second row is the opposite.
4. Calculate the loss using Categorical Cross Entropy.

This idea uses the dot product as the similarity metric. There are others.

https://zilliz.com/blog/similarity-metrics-for-vector-search

Google note their popular videos have large norms, and rate items with large norms
at initialization that don't get updated often can be a problem as well.

https://developers.google.com/machine-learning/recommendation/overview/candidate-generation#dot-product

### ANN

https://pynndescent.readthedocs.io/en/latest/how_to_use_pynndescent.html - On mac
https://github.com/google-research/google-research/blob/master/scann/docs/example.ipynb - On Linux
https://ann-benchmarks.com/glove-100-angular_10_angular.html

### Other links

https://developers.google.com/machine-learning/recommendation/dnn/re-ranking
https://github.com/google-research/google-research/blob/master/scann/docs/algorithms.md
https://proceedings.mlr.press/v9/glorot10a/glorot10a.pdf
https://docs.pytorch.org/tutorials/beginner/basics/intro.html

## Review

### Creating a numeric representation

The first stage of a recommendation engine is to create a numeric representation of the queries and the candidate items. You can't use strings in models and so you need to convert them into something you can put through a model.

Our current approach is to use **pre-computed embeddings** from the upstream pipeline (mimic/EmbeddingStore). The embedding step (contrastive learning on sets, autoencoders, etc.) is decoupled from the recommendation models. This keeps the recommender models simple — they accept fixed-size embedding vectors and learn lightweight projection layers to adapt them for the task.

The recommender models keep a small learnable projection layer so they can still adapt the upstream embeddings to the specific retrieval or ranking objective:
- **Retrieval**: 2-layer MLP projection (Linear → ReLU → Linear) per side, because the dot-product scoring provides no further nonlinearity
- **Ranking**: single Linear projection per side, because the downstream regressor MLP already provides nonlinearity

This separation means you can retrain embeddings without touching the recommender, or swap the recommender architecture without re-encoding features.

### Defining Retreival

A good Retrieval model will consume a query embedding and return a list of candidates which go "well" with that query.

At this point our query is a N dimensional vector and our candidates are also N dimensional vectors.

By multiplying the query vector with each of the candidate vectors, we get a score for each candidate. This score is the dot product of the two vectors.

If two vectors are identical then the dot product is 1. Therefore, the closer the dot product is to 1, the better the score. This is why we use the categorical cross entropy loss function. The labels are an identity matrix and then we compare the scores to that.
