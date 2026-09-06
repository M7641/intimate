
# Model Explainability Data Requirements

Model explainability will require specific data artefacts to exist inside the database to run.

## Required

The attribute specified in `entity_id` will be expected in all tables.

### Table containing the response variable to explain - response_variable_table_name

| [Table key defined in entity_id] | [Attribute defined in response_variable_field_name] |
|----------------------------------|-----------------------------------------------------|
|                                  |                                                     |

For example, if it's a standard Customer Intelligence solution, you will have `entity_id` being `customer_id`. If you then set `response_variable_field_name` to be, say, `target`, then the desired table would be:

| customer_id | target |
|------------ |--------|
|             |        |

Other attributes in this table are ignored for this application.

### Table containing data used to explain the response variable supplied above - table_used_to_explain

At the bare minimum, this table must contain the attribute defined in `entity_id` and one other attribute to explain the response variable above.

| [Table key defined in entity_id] | [Any attribute] |
|----------------------------------|-----------------|
|                                  |                 |

You can use the `fields_to_explain_with` setting to select only a subset of the available data. By default, it will select all the attributes in the table.

Note that attributes defined in `fields_to_inlcude_in_individual_table` will also be taken from this data artefact.

## Optional

### Model Validation Table

Used for the Model Validation tab, this table includes data on how well a model has been trained. The following is required for this feature to be enabled:

In the Binary Classification case:

| [Table key as defined in entity_id] | score | target | predicted |
|----------------------------------|-------|-------:|----------:|
|                                  |       |        |           |

MultiClass:

| [Table key as defined in entity_id] | target | predicted |
|----------------------------------|-------:|----------:|
|                                  |        |           |

Regression:

| [Table key as defined in entity_id] | target | predicted |
|----------------------------------|-------:|----------:|
|                                  |        |           |

In all cases, the target represents the response variable the model is attempting to predict, and the predicted is the model output for that particular table id.
