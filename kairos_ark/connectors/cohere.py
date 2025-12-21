import os
from typing import List
from .base import ArkBaseConnector

try:
    import cohere
    HAS_COHERE = True
except ImportError:
    HAS_COHERE = False

class ArkCohereConnector(ArkBaseConnector):
    """Connector for Cohere models."""
    
    def __init__(self, model="command-r-08-2024", embedding_model="embed-english-v3.0", api_key=None, agent=None):
        super().__init__(agent=agent)
        if not HAS_COHERE:
            raise ImportError("ArkCohereConnector requires 'cohere' package. Install via pip.")
            
        self.api_key = api_key or os.getenv("COHERE_API_KEY")
        if not self.api_key:
            raise ValueError("COHERE_API_KEY required.")
            
        self.client = cohere.Client(api_key=self.api_key)
        self.model = model
        self.embedding_model = embedding_model

    def generate(self, prompt: str) -> str:
        self._enforce_policy()
        response = self.client.chat(
            model=self.model,
            message=prompt
        )
        return response.text

    def embed(self, text: str) -> List[float]:
        self._enforce_policy()
        response = self.client.embed(
            texts=[text],
            model=self.embedding_model,
            input_type="search_document"
        )
        return response.embeddings[0]
