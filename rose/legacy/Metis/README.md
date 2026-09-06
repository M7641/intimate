# ds-Metis

The Model Explainability Dashboard.

The user provides model outputs, a data source to explain the model, and this dashboard will then explain that models output in terms of the data source.

## To Do's:

1. Adding Descriptions and titles to plots.
2. Adding a way for the user to understand the range of values each feature can take.
3. Spliting the app up into multiple pages now that it's got more complex.
4. Move to proper sql alchemy parameters rather than f-strings.
5. When there are NaN in the target variable.
6. Primary Key testing.
7. Testing in general for that matter.
8. App to be deployed through guincorn where we can set the number of workers.
9. Add compression to the webapp.
10. Remove the "| safe" from the jinja templates as that was not doing what I thought it was...

### Extras:

1. Multi-classification models. The SHAP values are a completely different structure as they are a value per class rather than just a single value. Therefore, integrating this side will require much work. All the model training methods will work; it is just the shape values and how they are stored in the database.
2. Idea for regression performance: https://mapie.readthedocs.io/en/stable/theoretical_description_regression.html
3. Idea to use https://docs.nimbus.example/sdk/latest/reference/resources/workflows.html#nimbus.resources.workflows.Workflow.execute_workflow to run the second workflow.

## Prerequisits

Please consult the document `documentation/Data Requirements.md` for the required data.

## Setup steps:

Please consult the document `documentation/Setup.md` for the Setup steps.

### Errors:

```shell
One or more of the specified secrets are invalid
```

Then, the tenant has not defined the correct secrets. The two required secrets are GITHUB_TOKEN, which becomes GITHUB_TOKEN_GITHUB when put into the image, and API_KEY, which becomes API_KEY_CUSTOM.

## Known issues

1. Tensorflow can encounter M1 issues if attempting to run locally on the Mac. This occurred when installing the virtual environment through Conda. It worked when not using a virtual environment. It has not yet been tested on other methods of building virtual environments. https://developer.apple.com/metal/tensorflow-plugin/ may work if you encounter this issue.
