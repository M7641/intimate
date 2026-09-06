import torch
from pure.logging import NimbusLogger

logger = NimbusLogger.get_logger(__name__)


class SimpleModel(torch.nn.Module):
    def __init__(self, input_dim: int):
        super(SimpleModel, self).__init__()
        self.device = "cpu"
        self.fc = torch.nn.Linear(input_dim, 1)
        self.loss_fn = torch.nn.MSELoss()
        self.optimizer = torch.optim.Adam(self.parameters(), lr=0.001)

    def forward(self, x):
        return self.fc(x)

    def train_model(self, dataloader):
        size = len(dataloader.dataset)
        self.train()
        for batch, (X, y) in enumerate(dataloader):
            X, y = X.to(self.device), y.to(self.device)

            # Compute prediction error
            pred = self(X)
            loss = self.loss_fn(pred, y)

            # Backpropagation
            loss.backward()
            self.optimizer.step()
            self.optimizer.zero_grad()

            if batch % 1000 == 0:
                loss, current = loss.item(), (batch + 1) * len(X)
                logger.info(f"loss: {loss:>7f}  [{current:>5d}/{size:>5d}]")

    def test_model(self, dataloader):
        num_batches = len(dataloader)
        self.eval()
        test_loss = 0

        with torch.no_grad():
            for X, y in dataloader:
                X, y = X.to(self.device), y.to(self.device)
                pred = self(X)

                test_loss += self.loss_fn(pred, y).item()

        test_loss /= num_batches
        logger.info(f"Test Error: Avg loss: {test_loss:>8f}")

    def predict(self, X):
        self.eval()
        with torch.no_grad():
            X = X.to(self.device)
            predictions = self(X)
        return predictions
