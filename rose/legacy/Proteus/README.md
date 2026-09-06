# Proteus

Proteus is a framework to build and deploy Machine Learning models for customer beahviour problems.

## Usage

Model config file should be in the following format:

```yaml
data_structures:
    <data source name>:
        entity_id:
        data_structure_name:
        data_structure_type:
        source_train_table_name:
        source_predict_table_name:
        manual_encoding_choices:
            one_hot_encoder:
            ordinal_encoder:
            numerical_encoder:
            date_encoder:

model_configurations:
    <model name>:
        entity_id:
        model_name:
        model_type:
        source_response_variable_table_name:
        source_output_table_name:
        manual_train_test_split_table_name:
        data_structures_used: [
            "<data source name>"
        ]
        epochs:
        max_trials:
        training_verbose_level:
```

Then the model can be trained using the following code:

```python
from pathlib import Path
from proteus.traning_workflow import model_train_workflow

model_train_workflow(
    model_name=<model name>,
    dev_test_prod_setting=true,
    config_dir=Path.cwd() / "model_config.yml",
    re_train_model="train",
    root_path=Path.cwd(),
)

```

## To Do's:

1. Test coverage is non-existent.
2. Modernise the connections with the DB.
3. I'd like to make this a more utils package rather than having the workflow structure in here.
4. Polars comaptibility.
