def get_schema(dev_test_prod_setting: str) -> str:
    if dev_test_prod_setting.lower() in ["test", "dev"]:
        schema = "TRANSFORM"
    elif dev_test_prod_setting.lower() == "prod":
        schema = "PUBLISH"
    else:
        msg = (
            f"Invalid input {dev_test_prod_setting} for 'dev_test_prod_setting', only"
            + "dev, test, and prod allowed!"
        )
        raise ValueError(msg)

    return schema


def get_model_save_name(dev_test_prod_setting: str, model_name: str) -> str:
    if dev_test_prod_setting.lower() in ["dev", "test", "prod"]:
        model_name += dev_test_prod_setting.lower()
    else:
        msg = (
            f"Invalid input {dev_test_prod_setting} for 'dev_test_prod_setting', only"
            + "dev, test, and prod allowed!"
        )
        raise ValueError(msg)

    return model_name
