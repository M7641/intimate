# AUTO GENERATED FILE - DO NOT EDIT

from dash.development.base_component import Component, _explicitize_args


class ForcePlot(Component):
    """A ForcePlot component.
    This is a description of the ForcePlot component.

    Keyword arguments:

    - id (string; optional):
        Unique ID to identify this component in Dash callbacks.

    - baseValue (number; default 0):
        The base value.

    - className (string; default ''):
        The CSS class of the component.

    - featureNames (list of strings; optional):
        feature names.

    - features (dict; required):
        The features of the model.

    - hideBars (boolean; default False):
        hide bars.

    - hideBaseValueLabel (boolean; default False):
        hide base value label.

    - labelMargin (number; default 20):
        label margin.

    - link (a value equal to: 'identity', 'logit'; default 'identity'):
        link function.

    - outNames (list of strings; optional):
        The out names.

    - plot_cmap (a value equal to: 'RdBu', 'GnPR', 'CyPU', 'PkYg', 'DrDb', 'LpLb', 'YlDp', 'OrId'; default 'RdBu'):
        plot color map.

    - style (dict; optional):
        The style of the component.

    - title (string; default ''):
        The title of the component."""

    _children_props = []
    _base_nodes = ["children"]
    _namespace = "the_emporium"
    _type = "ForcePlot"

    @_explicitize_args
    def __init__(
        self,
        features=Component.REQUIRED,
        style=Component.UNDEFINED,
        title=Component.UNDEFINED,
        className=Component.UNDEFINED,
        baseValue=Component.UNDEFINED,
        plot_cmap=Component.UNDEFINED,
        link=Component.UNDEFINED,
        featureNames=Component.UNDEFINED,
        outNames=Component.UNDEFINED,
        hideBaseValueLabel=Component.UNDEFINED,
        hideBars=Component.UNDEFINED,
        labelMargin=Component.UNDEFINED,
        id=Component.UNDEFINED,
        **kwargs,
    ):
        self._prop_names = [
            "id",
            "baseValue",
            "className",
            "featureNames",
            "features",
            "hideBars",
            "hideBaseValueLabel",
            "labelMargin",
            "link",
            "outNames",
            "plot_cmap",
            "style",
            "title",
        ]
        self._valid_wildcard_attributes = []
        self.available_properties = [
            "id",
            "baseValue",
            "className",
            "featureNames",
            "features",
            "hideBars",
            "hideBaseValueLabel",
            "labelMargin",
            "link",
            "outNames",
            "plot_cmap",
            "style",
            "title",
        ]
        self.available_wildcard_properties = []
        _explicit_args = kwargs.pop("_explicit_args")
        _locals = locals()
        _locals.update(kwargs)  # For wildcard attrs and excess named props
        args = {k: _locals[k] for k in _explicit_args}

        for k in ["features"]:
            if k not in args:
                raise TypeError("Required argument `" + k + "` was not specified.")

        super(ForcePlot, self).__init__(**args)
