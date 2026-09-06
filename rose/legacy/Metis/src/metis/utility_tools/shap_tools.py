from __future__ import annotations

import warnings

import numpy as np
import pandas as pd
import shap
from metis.configuration.configuration_classes import ExplainModel
from metis.utility_tools.data_constructor import DataConstructor

with warnings.catch_warnings():
    warnings.simplefilter("ignore")
    shap.explainers._deep.deep_tf.op_handlers["AddV2"] = (
        shap.explainers._deep.deep_tf.passthrough
    )


class MetisSHAP:
    def __init__(
        self,
        model,
        model_config: ExplainModel,
        data_model_was_trained_on: pd.DataFrame,
    ) -> None:
        self.model = model
        self.model_config = model_config
        self.data_to_evaluate_shap = data_model_was_trained_on

    def init(self, explainer_type="KernelExplainer"):
        constructor = DataConstructor(save_append_name=self.model_config.name)
        processed_data = constructor.process_data_for_model(
            self.data_to_evaluate_shap,
            train_encoders="use_saved",
        )

        processed_data_as_np = np.float32(processed_data)

        explainer_class = {
            "Explainer": shap.Explainer,
            "TreeExplainer": shap.TreeExplainer,
            "LinearExplainer": shap.LinearExplainer,
            "PermutationExplainer": shap.PermutationExplainer,
            "PartitionExplainer": shap.PartitionExplainer,
            "SamplingExplainer": shap.SamplingExplainer,
            "AdditiveExplainer": shap.AdditiveExplainer,
            "DeepExplainer": shap.DeepExplainer,
            "KernelExplainer": shap.KernelExplainer,
            "GradientExplainer": shap.GradientExplainer,
            "ExactExplainer": shap.ExactExplainer,
        }.get(explainer_type, shap.DeepExplainer)

        expl = explainer_class(self.model, processed_data_as_np)

        self.explainer = expl

        if explainer_type == "DeepExplainer":
            self.base_value = expl.expected_value[0].numpy()
        else:
            self.base_value = expl.expected_value

        if explainer_type == "TreeExplainer":
            shap_values = expl.shap_values(processed_data_as_np, check_additivity=False)
        else:
            shap_values = expl.shap_values(processed_data_as_np)

        self.shap_values = shap_values.reshape(
            shap_values.shape[0], shap_values.shape[1]
        )
        df_columns = processed_data.columns

        self.shap_as_df = pd.DataFrame(
            np.float32(self.shap_values),
            columns=df_columns,
            index=processed_data.index,
        )

    def get_shap_values_as_dataframe(self):
        """
        Returns the SHAP values of the model as a pandas DataFrame.
        """
        return self.shap_as_df

    def get_data_model_was_trained_on_data(self):
        """
        Returns the data shap was evaluated on.

        :return: The data model was trained on, either the unprocessed data or the data to evaluate SHAP values.
        """
        return self.data_to_evaluate_shap

    def get_base_value_for_explainer(self):
        """
        Get the base value for the explainer.

        :return: The base value for the explainer.
        """
        return self.base_value

    def get_explainer_obejct(self):
        """
        Returns the explainer object.

        :return: The explainer object.
        """
        return self.explainer
