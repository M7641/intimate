from pathlib import Path
import torch
import onnx


class ModelManager:
    def save_onnx(
        self, model: torch.nn.Module, input_dim: tuple, file_path: Path
    ) -> None:
        file_path.parent.mkdir(parents=True, exist_ok=True)
        dummy_input = torch.zeros(1, *input_dim, dtype=torch.float32)
        torch.onnx.export(
            model,
            dummy_input,
            file_path,
            input_names=["x"],
            output_names=["output"],
            # both input and output need the batch dimension to be dynamic
            # otherwise the output will always expect batch size of 1
            dynamic_axes={"x": {0: "batch"}, "output": {0: "batch"}},
            do_constant_folding=True,
        )

    def load_onnx(self, file_path: Path) -> torch.nn.Module:

        if not file_path.exists():
            raise FileNotFoundError(f"ONNX model file {file_path} does not exist.")

        onnx_model = onnx.load(file_path)
        onnx.checker.check_model(onnx_model)
        return onnx_model
