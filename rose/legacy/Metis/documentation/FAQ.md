# Frequently Asked Questions (FAQ)

## Why am I unable to delete the deployment?

The workflow provided by this application builds a new workflow. This new workflow then relies on the previous image built for the initial workflow. This connection means the platform will refuse to delete the deployment until the created workflow is manually deleted.

Steps to fix:
1. Delete the workflow called: `metis-build-shap-values`
2. Try to delete the deployment again.
3. If the problem persists, it's likely a platform problem; thus, a support ticket may be in order.

In the future, we aim to deploy a new image for the new workflow to run from.
