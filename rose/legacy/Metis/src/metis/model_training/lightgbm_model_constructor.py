try:
    import lightgbm as lgb
except ImportError:
    lgb = None

import logging

import numpy as np
import pandas as pd
from metis.utility_tools.model_tools import (return_class_weights,
                                             return_response_variable_type)
from scipy.stats import randint
from sklearn.model_selection import RandomizedSearchCV


class LightGBMConstructor:

    initial_parameters = {
        "boosting_type": "dart",
        "verbose": -1,
        "data_sample_strategy": "bagging",
        "force_col_wise": True,
    }

    hyper_tuning_parameters = {
        "num_leaves": randint(10, 100),
        "max_bin": randint(15, 150),
    }

    def __init__(self, parameters: dict | None) -> None:
        if parameters is None:
            self.parameters = self.initial_parameters
        else:
            self.parameters = parameters

    @staticmethod
    def learning_rate_decay_power(current_iter):
        base_learning_rate = 0.05
        lr = base_learning_rate * np.power(0.99, current_iter)
        return lr if lr > 1e-3 else 1e-3

    def fit(self, train_data: dict, target: pd.DataFrame):

        model_target_type = return_response_variable_type(train_data["target_train"])

        class_weights = (
            return_class_weights(target)
            if model_target_type in ["binary_classifier", "multi_classifier"]
            else None
        )

        if model_target_type == "binary_classifier":
            self.parameters["objective"] = "binary"
            self.parameters["metric"] = ["binary_logloss", "auc"]
        elif model_target_type == "multi_classifier":
            self.parameters["objective"] = "softmax"
            self.parameters["metric"] = "multi_logloss"
            self.parameters["num_class"] = len(target["target"].unique())
        else:
            self.parameters["objective"] = "regression"
            self.parameters["metric"] = "l2"

        self.parameters["class_weight"] = class_weights

        if model_target_type in ["binary_classifier", "multi_classifier"]:
            model = lgb.LGBMClassifier(**self.parameters)
        else:
            model = lgb.LGBMRegressor(**self.parameters)

        metric_to_optimize = {
            "binary_classifier": "roc_auc",
            "multi_classifier": "roc_auc_ovr",
            "regression": "r2",
        }

        havling_grid_search = RandomizedSearchCV(
            estimator=model,
            param_distributions=self.hyper_tuning_parameters,
            n_iter=10,
            scoring=metric_to_optimize.get(model_target_type),
            verbose=0,
        )

        logging.warning("Hyperparameter tuning for LightGBM model.")

        havling_grid_search.fit(
            train_data["data_to_use_train"],
            train_data["target_train"].values.ravel(),
            eval_set=[
                (
                    train_data["data_to_use_test"],
                    train_data["target_test"].values.ravel(),
                )
            ],
            callbacks=[
                lgb.reset_parameter(
                    learning_rate=lambda iter: self.learning_rate_decay_power(iter)
                )
            ],
        )

        havling_grid_search.best_params_["verbose"] = 1
        model.set_params(**havling_grid_search.best_params_)

        logging.warning("Training LightGBM model with best hyperparameters.")

        model.fit(
            train_data["data_to_use_train"],
            train_data["target_train"].values.ravel(),
            eval_set=[
                (
                    train_data["data_to_use_test"],
                    train_data["target_test"].values.ravel(),
                )
            ],
            callbacks=[
                lgb.reset_parameter(
                    learning_rate=lambda iter: self.learning_rate_decay_power(iter)
                )
            ],
        )

        return model
