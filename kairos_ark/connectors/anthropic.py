import os
from .base import ArkBaseConnector

try:
    import anthropic
    HAS_ANTHROPIC = True
except ImportError:
    HAS_ANTHROPIC = False

class ArkClaudeConnector(ArkBaseConnector):
    """Connector for Anthropic Claude models."""
    
    def __init__(self, model="claude-3-5-sonnet-20240620", api_key=None, agent=None):
        super().__init__(agent=agent)
        if not HAS_ANTHROPIC:
            raise ImportError("ArkClaudeConnector requires 'anthropic' package.")
            
        self.api_key = api_key or os.getenv("ANTHROPIC_API_KEY")
        if not self.api_key:
            raise ValueError("ANTHROPIC_API_KEY required.")
            
        self.client = anthropic.Anthropic(api_key=self.api_key)
        self.model = model

    def generate(self, prompt: str) -> str:
        self._enforce_policy()
        response = self.client.messages.create(
            model=self.model,
            max_tokens=1024,
            messages=[{"role": "user", "content": prompt}]
        )
        return response.content[0].text
