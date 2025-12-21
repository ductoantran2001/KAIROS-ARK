import os
from .base import ArkBaseConnector

try:
    from mistralai.client import MistralClient
    from mistralai.models.chat_completion import ChatMessage
    HAS_MISTRAL = True
except ImportError:
    HAS_MISTRAL = False

class ArkMistralConnector(ArkBaseConnector):
    """Connector for Mistral AI."""
    
    def __init__(self, model="mistral-tiny", api_key=None, agent=None):
        super().__init__(agent=agent)
        if not HAS_MISTRAL:
            raise ImportError("ArkMistralConnector requires 'mistralai' package.")
            
        self.api_key = api_key or os.getenv("MISTRAL_API_KEY")
        if not self.api_key:
            raise ValueError("MISTRAL_API_KEY required.")
            
        self.client = MistralClient(api_key=self.api_key)
        self.model = model

    def generate(self, prompt: str) -> str:
        self._enforce_policy()
        response = self.client.chat(
            model=self.model,
            messages=[ChatMessage(role="user", content=prompt)]
        )
        return response.choices[0].message.content
