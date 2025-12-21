from abc import ABC, abstractmethod
from typing import Optional, List, Any

class ArkBaseConnector(ABC):
    """Base interface for all KAIROS-ARK LLM Connectors."""

    @abstractmethod
    def generate(self, prompt: str) -> str:
        """Generate text from a prompt."""
        pass

    def embed(self, text: str) -> List[float]:
        """Generate embeddings for text (optional implementation)."""
        raise NotImplementedError("Embedding not supported by this connector.")
