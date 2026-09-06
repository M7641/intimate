import torch
import onnxruntime as ort


class ORTModelRuntime:
    def __init__(self, model_path: str):
        self.session = ort.InferenceSession(
            model_path, providers=["CPUExecutionProvider"]
        )

    def predict(self, input_data: torch.Tensor, input_dim: tuple) -> list:
        """
        Perform inference using the ONNX Runtime session.

        Only accepts a single input tensor currently.

        Args:
            input_data (torch.Tensor): A tensor providing input data for the model.

        Returns:
            list: The output from the model inference.
        """

        onnx_inputs = input_data.numpy(force=True).reshape(-1, *input_dim)
        input_name = self.session.get_inputs()[0].name
        return self.session.run([], {input_name: onnx_inputs})[0]
