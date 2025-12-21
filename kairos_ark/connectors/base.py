from abc import ABC, abstractmethod
from typing import Optional, List, Any

class ArkBaseConnector(ABC):
    """Base interface for all KAIROS-ARK LLM Connectors."""

    def __init__(self, agent=None):
        self.agent = agent

    def _enforce_policy(self):
        """Check if the kernel allows LLM calls."""
        if self.agent:
            # Check for LLM_CALL capability (0b00010000 = 16)
            # We import Cap locally to avoid circular imports if possible, 
            # or hardcode the flag if Agent exposes a simpler check.
            # Assuming agent.check_capability takes an int flag.
            CAP_LLM_CALL = 16 
            allowed, reason = self.agent.check_capability(CAP_LLM_CALL)
            if not allowed:
                raise PermissionError(f"Policy Violation: LLM calls denied by kernel. ({reason})")

    @abstractmethod
    def generate(self, prompt: str) -> str:
        """Generate text from a prompt."""
        self._enforce_policy()
        pass

    def embed(self, text: str) -> List[float]:
        """Generate embeddings for text (optional implementation)."""
        self._enforce_policy()
        raise NotImplementedError("Embedding not supported by this connector.")
