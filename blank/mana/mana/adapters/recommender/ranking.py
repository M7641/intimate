import dataclasses
import datetime
import json
import time
from dataclasses import dataclass
from pathlib import Path

import torch
from pure.logging import NimbusLogger
from rich.console import Console
from rich.progress import Progress

from mana.adapters.recommender.interactions import InteractionDataLoader

logger = NimbusLogger.get_logger(__name__)


@dataclass
class RankingTrainConfig:
    # Architecture
    hidden_dim: int = 32
    dropout: float = 0.1

    # Optimizer
    learning_rate: float = 0.001
    weight_decay: float = 0.0
    grad_clip_max_norm: float = 1.0

    # LR Scheduler
    lr_factor: float = 0.75
    lr_patience: int = 3
    lr_min: float = 1e-6

    # Training loop
    early_stopping_patience: int = 5
    monitor_metric: str = "spearman"

    # Loss
    loss: str = "huber"  # "mse" | "huber" | "mae"
    huber_delta: float = 1.0


class RankingModel(torch.nn.Module):
    def __init__(
        self,
        embedding_dim: int,
        target_variable: str = "labels",
        config: RankingTrainConfig | None = None,
    ) -> None:

        super().__init__()

        self.device = torch.device("cuda" if torch.cuda.is_available() else "cpu")

        self.config = config if config is not None else RankingTrainConfig()

        valid_losses = {"mse", "huber", "mae"}
        if self.config.loss not in valid_losses:
            raise ValueError(
                f"Unknown loss {self.config.loss!r}, expected one of {sorted(valid_losses)}"
            )
        valid_metrics = {"pearson", "spearman", "mae"}
        if self.config.monitor_metric not in valid_metrics:
            raise ValueError(
                f"Unknown monitor_metric {self.config.monitor_metric!r}, "
                f"expected one of {sorted(valid_metrics)}"
            )

        self.embedding_dim = embedding_dim
        self.hidden_dim = self.config.hidden_dim
        self.target_variable = target_variable

        self.query_projection = torch.nn.Linear(embedding_dim, self.hidden_dim)
        self.candidate_projection = torch.nn.Linear(embedding_dim, self.hidden_dim)
        self.regressor = torch.nn.Sequential(
            torch.nn.Linear(self.hidden_dim * 2, self.hidden_dim),
            torch.nn.ReLU(),
            torch.nn.Dropout(self.config.dropout),
            torch.nn.Linear(self.hidden_dim, 1),
        )

        # Metrics tracking
        self.metrics_history = {
            "train_loss": [],
            "test_loss": [],
            "test_metrics": [],
            "epochs": [],
            "learning_rates": [],
        }

        self._setup_optimizer_and_schedulers()

        self.to(self.device)

    def _setup_optimizer_and_schedulers(self) -> None:
        """Setup optimizer, loss function, and learning rate schedulers."""
        self.optimizer = torch.optim.Adam(
            self.parameters(),
            lr=self.config.learning_rate,
            weight_decay=self.config.weight_decay,
        )

        loss_map = {
            "mse": torch.nn.MSELoss,
            "huber": lambda: torch.nn.HuberLoss(delta=self.config.huber_delta),
            "mae": torch.nn.L1Loss,
        }
        self.loss_fn = loss_map[self.config.loss]()

        self.lr_scheduler = torch.optim.lr_scheduler.ReduceLROnPlateau(
            self.optimizer,
            mode="min",
            factor=self.config.lr_factor,
            patience=self.config.lr_patience,
            min_lr=self.config.lr_min,
        )

    def forward(
        self,
        input_dict: dict[str, torch.Tensor],
    ) -> torch.Tensor:
        query_embeds = self.query_projection(
            input_dict["query_embedding"].to(self.device)
        )
        candidate_embeds = self.candidate_projection(
            input_dict["candidate_embedding"].to(self.device)
        )
        combined_embeds = torch.cat([query_embeds, candidate_embeds], dim=-1)
        return self.regressor(combined_embeds).squeeze(-1)

    def train_model(
        self,
        data_loader: torch.utils.data.DataLoader,
        console: Console,
    ) -> float:
        """Train the model for one epoch.

        Returns the eval-mode loss (dropout off) so it is directly
        comparable with the test loss reported by ``test_model``.
        """
        self.train()

        for x in data_loader:
            self.optimizer.zero_grad()
            outputs = self.forward(x)
            target = x[self.target_variable].to(self.device)
            loss = self.loss_fn(outputs, target)
            loss.backward()
            torch.nn.utils.clip_grad_norm_(
                self.parameters(), max_norm=self.config.grad_clip_max_norm
            )
            self.optimizer.step()

        # Re-evaluate in eval mode so train loss is comparable to test loss
        self.eval()
        total_loss = 0
        batch_count = 0
        with torch.no_grad():
            for x in data_loader:
                outputs = self.forward(x)
                target = x[self.target_variable].to(self.device)
                loss = self.loss_fn(outputs, target)
                total_loss += loss.item()
                batch_count += 1

        avg_loss = total_loss / batch_count if batch_count > 0 else 0.0
        console.log(f"Train Loss: {avg_loss:>7f}")
        return avg_loss

    def test_model(
        self,
        data_loader: torch.utils.data.DataLoader,
        console: Console,
    ) -> tuple[float, dict[str, float]]:
        """Test the model and compute ranking metrics (MAE, Pearson, Spearman)."""
        self.eval()
        total_loss = 0
        batch_count = 0
        all_preds: list[torch.Tensor] = []
        all_targets: list[torch.Tensor] = []

        with torch.no_grad():
            for x in data_loader:
                outputs = self.forward(x)
                target = x[self.target_variable].to(self.device)
                loss = self.loss_fn(outputs, target)
                total_loss += loss.item()
                batch_count += 1
                all_preds.append(outputs.cpu())
                all_targets.append(target.cpu())

        avg_loss = total_loss / batch_count if batch_count > 0 else 0.0

        metric_values: dict[str, float] = {}
        if all_preds:
            preds = torch.cat(all_preds)
            targets = torch.cat(all_targets)

            # MAE
            metric_values["mae"] = (preds - targets).abs().mean().item()

            # Pearson correlation
            p_centered = preds - preds.mean()
            t_centered = targets - targets.mean()
            numer = (p_centered * t_centered).sum()
            denom = (p_centered.pow(2).sum() * t_centered.pow(2).sum()).sqrt()
            metric_values["pearson"] = (numer / denom).item() if denom > 0 else 0.0

            # Spearman correlation (Pearson on ranks)
            pred_ranks = preds.argsort().argsort().float()
            target_ranks = targets.argsort().argsort().float()
            pr_centered = pred_ranks - pred_ranks.mean()
            tr_centered = target_ranks - target_ranks.mean()
            s_numer = (pr_centered * tr_centered).sum()
            s_denom = (pr_centered.pow(2).sum() * tr_centered.pow(2).sum()).sqrt()
            metric_values["spearman"] = (
                (s_numer / s_denom).item() if s_denom > 0 else 0.0
            )

        metrics_str = "  ".join(f"{k}={v:.4f}" for k, v in metric_values.items())
        console.log(f"Test Loss: {avg_loss:>4f}  {metrics_str}")

        return avg_loss, metric_values

    def train_loop(
        self,
        epochs: int,
        dataset: InteractionDataLoader,
        metrics_file_path: str | Path | None = None,
        checkpoint_dir: str | Path = Path.cwd() / "checkpoints/",
    ) -> None:
        """Main training loop with learning rate scheduling and early stopping."""
        early_stopping_patience = self.config.early_stopping_patience
        monitor_metric = self.config.monitor_metric

        # Reset metrics history for this training run
        self.metrics_history = {
            "train_loss": [],
            "test_loss": [],
            "test_metrics": [],
            "epochs": [],
            "learning_rates": [],
        }

        # Early stopping variables
        best_test_loss = float("inf")
        best_metric = float("-inf")
        epochs_without_improvement = 0
        best_epoch = 0

        if isinstance(checkpoint_dir, str):
            checkpoint_dir = Path(checkpoint_dir)

        training_start = time.time()
        with Progress() as progress:
            task = progress.add_task("[green]Training...", total=epochs)

            for epoch in range(1, epochs + 1):
                current_lr = self.optimizer.param_groups[0]["lr"]
                progress.console.log(f"Epoch {epoch}/{epochs}")

                train_loss = self.train_model(
                    dataset.train_dataset,
                    console=progress.console,
                )

                test_loss, test_metrics = self.test_model(
                    dataset.test_dataset,
                    console=progress.console,
                )

                self.metrics_history["epochs"].append(epoch)
                self.metrics_history["train_loss"].append(train_loss)
                self.metrics_history["test_loss"].append(test_loss)
                self.metrics_history["learning_rates"].append(current_lr)
                self.metrics_history["test_metrics"].append(test_metrics)

                # Check for improvement
                metric_improved = False
                monitored_value = test_metrics.get(monitor_metric)
                if monitored_value is not None and monitored_value > best_metric:
                    best_metric = monitored_value
                    metric_improved = True

                if test_loss < best_test_loss or metric_improved:
                    epochs_without_improvement = 0
                    best_epoch = epoch
                    best_test_loss = test_loss
                    self.save_model(checkpoint_dir)
                    progress.console.log(
                        f"[bold green]New best model saved![/bold green] Test loss: {test_loss:.4f}, {monitor_metric}: {monitored_value:.4f}"
                        if monitored_value is not None
                        else f"[bold green]New best model saved![/bold green] Test loss: {test_loss:.4f}",
                    )
                else:
                    epochs_without_improvement += 1
                    progress.console.log(
                        f"No improvement for {epochs_without_improvement}/{early_stopping_patience} epochs",
                    )

                # Learning rate scheduling
                self.lr_scheduler.step(test_loss)

                # Early stopping check
                if epochs_without_improvement >= early_stopping_patience:
                    progress.console.log(
                        f"[bold yellow]Early stopping triggered![/bold yellow] No improvement for {early_stopping_patience} epochs.",
                    )
                    logger.debug(
                        "Early stopping at epoch %d. Best test loss: %.4f at epoch %d",
                        epoch,
                        best_test_loss,
                        best_epoch,
                    )
                    progress.update(task, completed=epochs)
                    break

                progress.update(task, advance=1)

        training_elapsed = time.time() - training_start
        actual_epochs = len(self.metrics_history["epochs"])
        logger.info(
            "Training loop completed in %.3fs (%.3fs avg/epoch)",
            training_elapsed,
            training_elapsed / actual_epochs,
        )

        # Restore best model
        if checkpoint_dir.exists():
            logger.info(
                "Loading best model from epoch %d with test loss %.4f",
                best_epoch,
                best_test_loss,
            )
            loaded_model = RankingModel.load_model(checkpoint_dir)
            self.load_state_dict(loaded_model.state_dict())
            logger.info("Best model restored successfully")

        if metrics_file_path is not None:
            self.save_metrics_history(metrics_file_path)

    def predict_scores(
        self,
        input_data: dict[str, torch.Tensor],
    ) -> torch.Tensor:
        """Generate predictions for given query IDs against all candidate IDs."""
        self.eval()
        with torch.no_grad():
            return self.forward(input_data).cpu()

    def save_metrics_history(self, file_path: str | Path) -> None:
        """Save metrics history to a JSON file."""

        output_path = Path(file_path) if isinstance(file_path, str) else file_path

        output_path.parent.mkdir(parents=True, exist_ok=True)

        metrics_data = {
            "metadata": {
                "num_epochs": len(self.metrics_history["epochs"]),
                "timestamp": datetime.datetime.now(
                    tz=datetime.timezone.utc,
                ).isoformat(),
            },
            "history": self.metrics_history,
        }

        with output_path.open("w") as f:
            json.dump(metrics_data, f, indent=2)

    def save_model(self, file_dir: str | Path = Path.cwd() / "checkpoints/") -> None:
        """Save the complete model state including weights, optimizer, and configuration."""
        if isinstance(file_dir, str):
            file_dir = Path(file_dir)

        file_dir.mkdir(parents=True, exist_ok=True)

        checkpoint = {
            "model_state_dict": self.state_dict(),
            "optimizer_state_dict": self.optimizer.state_dict(),
            "lr_scheduler_state_dict": self.lr_scheduler.state_dict(),
            "config": {
                "embedding_dim": self.embedding_dim,
                "target_variable": self.target_variable,
                **dataclasses.asdict(self.config),
            },
            "metrics_history": self.metrics_history,
        }

        torch.save(checkpoint, file_dir / "model_checkpoint.pt")
        logger.info("Model saved to %s", file_dir / "model_checkpoint.pt")

    @classmethod
    def load_model(
        cls,
        file_dir: str | Path,
    ) -> "RankingModel":
        """Load a saved model from checkpoint."""

        if isinstance(file_dir, str):
            file_dir = Path(file_dir)

        device = torch.device("cuda" if torch.cuda.is_available() else "cpu")

        # Try new filename first, fall back to legacy
        new_path = file_dir / "model_checkpoint.pt"
        legacy_path = file_dir / "ranking_model_checkpoint.pth"
        file_path = new_path if new_path.exists() else legacy_path

        checkpoint = torch.load(file_path, map_location=device, weights_only=False)

        config = checkpoint["config"]

        # Extract fields that belong to RankingTrainConfig
        config_field_names = {f.name for f in dataclasses.fields(RankingTrainConfig)}
        train_config_dict = {k: v for k, v in config.items() if k in config_field_names}
        train_config = RankingTrainConfig(**train_config_dict)

        model = cls(
            embedding_dim=config["embedding_dim"],
            target_variable=config.get("target_variable", "labels"),
            config=train_config,
        )

        model.load_state_dict(checkpoint["model_state_dict"])
        model.optimizer.load_state_dict(checkpoint["optimizer_state_dict"])
        model.lr_scheduler.load_state_dict(checkpoint["lr_scheduler_state_dict"])

        if "metrics_history" in checkpoint:
            model.metrics_history = checkpoint["metrics_history"]

        model.to(device)
        logger.info("Model loaded from %s", file_path)

        return model
