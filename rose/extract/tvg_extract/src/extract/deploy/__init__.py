"""Deploy the extract pipeline as a Nimbus workflow.

A vendored slice of the Nimbus deploy client (`bootstrap-ouroboros`, workflow +
image tiers) plus :func:`build_workflow_spec`, the pure spec builder for this
pipeline. ``deploy_workflows`` builds a workflow image then creates/updates the
workflow, stamping the new image id onto every step.
"""

from __future__ import annotations

from .images import Images, deploy_image
from .spec import build_workflow_spec
from .types import Artifact
from .workflows import Workflows, deploy_workflows

__all__ = [
    "Artifact",
    "Images",
    "Workflows",
    "build_workflow_spec",
    "deploy_image",
    "deploy_workflows",
]
