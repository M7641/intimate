# mikes_emporium_of_components

## Install

```shell
pip install the_emporium
```

## Development
### Getting Started

1. Create a new python environment:
   ```shell
   python -m venv venv
   . venv/bin/activate
   ```
   _Note: venv\Scripts\activate for windows_

2. Install python dependencies:
   ```shell
   pip install -r requirements.txt
   ```
3. Install npm packages:
   1. Optional: use [nvm](https://github.com/nvm-sh/nvm) to manage node version:
      ```shell
      nvm install
      nvm use
      ```
   2. Install:
      ```shell
      npm install
      ```
4. Build:
   ```shell
   npm run build
   ```


## Further steps to get this working:

1. I've had to augment the `__init__.py` in the top level of this package to inlucde:

```python
from importlib.metadata import version
from the_emporium import *  # noqa: F401, F403
from the_emporium import __all__

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
```

I am not 100% it is all needed yet, but I am waiting to test it further when I have multiple components.

2. When adding new components, do add them to the index.ts as well. This way they can be picked up and found by the build process.
