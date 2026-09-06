# Setup Guide

## Setup steps:

1. Set tenant secrets: `GITHUB_TOKEN` as a `GITHUB` secret and `API_KEY` as a `CUSTOM` token. The API key being the tenant's API key.

2. Create and fill in the config file please see `documentation/Configuration File.md` for more details.

3. Deploy app from library.

4. run the workflow `build-metis-workflow` which will build you a new workflow.

5. Manually run the newly minted workflow called `metis-build-shap-values`.

You should now have a working application!
