import platform

import torch
from pure.logging import NimbusLogger

from .kdtree import KDTreeANN
from .scann import ScANN

logger = NimbusLogger.get_logger(__name__)


class ANNFacade(KDTreeANN, ScANN):
    def __init__(self, query_model: torch.nn.Module, method: str | None = None):
        current_os = platform.system()
        logger.debug(f"Current operating system detected: {current_os}")
        if method is None:
            if current_os == "Linux":
                logger.debug("Using ScANN as the ANN method on Linux.")
                method = "scann"
            else:
                logger.debug("Using KDTree as the ANN method on non-Linux OS.")
                method = "kdtree"

        if method == "kdtree":
            KDTreeANN.__init__(self, query_model)
        elif method == "scann":
            ScANN.__init__(self, query_model)
        else:
            raise ValueError(f"Unknown method: {method}")
