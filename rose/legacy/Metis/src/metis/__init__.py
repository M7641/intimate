from importlib.metadata import version

from .the_emporium.the_emporium._imports_ import *  # noqa: F401, F403
from .the_emporium.the_emporium._imports_ import __all__

__version__ = version("metis")

_js_dist = []
_js_dist.extend(
    [
        {
            "relative_package_path": "the_emporium/the_emporium/the_emporium.js",
            "namespace": "metis",
        },
        {
            "relative_package_path": "the_emporium/the_emporium/the_emporium.js.map",
            "namespace": "metis",
            "dynamic": True,
        },
    ]
)

_css_dist = []


for _component in __all__:
    setattr(locals()[_component], "_js_dist", _js_dist)
    setattr(locals()[_component], "_css_dist", _css_dist)
