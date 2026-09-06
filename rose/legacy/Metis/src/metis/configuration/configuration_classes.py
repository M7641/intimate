from typing import Literal, Optional

from pydantic import BaseModel, ConfigDict, Field, model_validator
from typing_extensions import Self


class ModelLabelSetting(BaseModel):
    use_bins: bool = Field(True, description="Whether to use bins.")
    use_quartiles: bool = Field(False, description="Whether to use quartiles.")
    bin_labels: list[str] = Field(
        ["Low", "Medium", "High"], description="The labels for the bins."
    )


class LLMSettings(BaseModel):
    llm_run_llm: bool = Field(
        False, description="Whether to run the LLM model for the given model."
    )
    llm_response_variable: Optional[str] = Field(
        "",
        description="Attribute expected in the response_variable_table_name table. The value to pass to an LLM model.",
    )
    llm_input_description: Optional[str] = Field(
        "",
        description="The description of the input for the LLM model.",
    )
    llm_response_variable_description: Optional[str] = Field(
        "",
        description="The description of the response variable for the LLM model.",
    )
    llm_model_description: Optional[str] = Field(
        "",
        description="The description of the model for the LLM model.",
    )


class UIConfig(BaseModel):
    show_individual_page: bool = Field(
        True, description="Whether to show the individual page."
    )
    show_model_performance_page: bool = Field(
        True, description="Whether to show the model performance page."
    )


class ExplainModel(BaseModel):

    # https://docs.pydantic.dev/latest/api/config/#pydantic.config.ConfigDict.protected_namespaces
    model_config = ConfigDict(protected_namespaces=(""))

    name: str = Field("", description="Do not use, will be auto filled.")
    response_variable_table_name: str = Field(
        description="The table that contains the model response variable to be explained.",
    )
    response_variable_field_name: str = Field(
        "model_prediction_for_explain",
        description="The name of the response variable in the table defined in response_variable_table_name.",
    )
    fields_to_explain_with: list[str] | str = Field(
        "*", description="The fields to explain the model with."
    )
    table_used_to_explain: str = Field(
        "publish.ucv_ucv",
        description="The table used to explain the model outputs providedin response_variable_table_name.",
    )
    visible_name: str = Field(
        "",
        description="The name of the model used on the front page. Will auto fill if not provided based on the name.",
    )
    fields_to_inlcude_in_individual_table: list[str] = Field(
        [], description="Fields to include on the Individual page."
    )
    description: str = Field(
        "", description="The description of the model used on the front page."
    )
    entity_id: str = Field(
        "customer_id", description="The id of the entity to be explained."
    )
    entity_name: str = Field(
        "Customer", description="The name of the entity to be explained."
    )
    output_name_in_plots: str = Field(
        "Hit Rate (%)", description="Used in the Hit Rate Plot."
    )
    model_validation_table: str = Field(
        "", description="The table used to validate the model."
    )
    max_number_of_rows_to_explain: int = Field(
        50000, description="The maximum number of rows to explain."
    )
    model_type_to_use: Literal["neural_network", "lightgbm"] = Field(
        "lightgbm", description="The type of model to use."
    )
    model_label_setting: ModelLabelSetting = Field(
        ModelLabelSetting(
            use_bins=True,
            use_quartiles=False,
            bin_labels=["Low", "Medium", "High"],
        ),
        description="The model label setting.",
    )
    llm_settings: LLMSettings = Field(
        LLMSettings(
            llm_run_llm=False,
            llm_response_variable="",
            llm_input_description="",
            llm_response_variable_description="",
            llm_model_description="",
        ),
        description="The LLM settings.",
    )
    model_type: Literal["binary_classifier", "regression"] = Field(
        "binary_classifier", description="The type of model."
    )
    explaining_model_metric_to_show: Literal["AUC", "F1", "Accuracy", "RSquared"] = (
        Field(
            "",
            description="The metric shown in the 'How well was the output explained' component.",
        )
    )
    ui_config: UIConfig = Field(
        UIConfig(
            show_individual_page=True,
            show_model_performance_page=True,
        ),
        description="The UI configuration.",
    )

    @model_validator(mode="after")
    def check_passwords_match(self) -> Self:

        if (
            not self.explaining_model_metric_to_show
            and self.model_type == "binary_classifier"
        ):
            self.explaining_model_metric_to_show = "AUC"
        elif (
            not self.explaining_model_metric_to_show
            and self.model_type == "multi_classifier"
        ):
            self.explaining_model_metric_to_show = "Accuracy"
        elif (
            not self.explaining_model_metric_to_show and self.model_type == "regression"
        ):
            self.explaining_model_metric_to_show = "RSquared"

        given_model_metric = self.explaining_model_metric_to_show

        if self.model_type == "binary_classifier" and given_model_metric not in [
            "AUC",
            "F1",
            "Accuracy",
        ]:
            raise ValueError("metric not supported for binary_classifier")
        elif self.model_type == "regression" and given_model_metric not in [
            "RSquared",
        ]:
            raise ValueError("metric not supported for regression")
        elif self.model_type == "multi_classifier" and given_model_metric not in [
            "Accuracy",
        ]:
            raise ValueError("metric not supported for multi_classifier")

        return self


class ConfigFile(BaseModel):
    define: list = None
    models_to_explain: dict[str, ExplainModel]
