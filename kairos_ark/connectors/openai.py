import os
from .base import ArkBaseConnector

try:
    from openai import OpenAI
    HAS_OPENAI = True
except ImportError:
    HAS_OPENAI = False

class ArkOpenAIConnector(ArkBaseConnector):
    """Generic Connector for OpenAI-compatible APIs (OpenAI, Groq, DeepSeek)."""
    
    def __init__(self, model: str, api_key: str = None, base_url: str = None, env_key: str = "OPENAI_API_KEY", agent=None):
        super().__init__(agent=agent)
        if not HAS_OPENAI:
            raise ImportError("ArkOpenAIConnector requires 'openai' package. Install via pip.")
            
        self.api_key = api_key or os.getenv(env_key)
        if not self.api_key:
            raise ValueError(f"{env_key} required.")
            
        self.model = model
        self.client = OpenAI(api_key=self.api_key, base_url=base_url)

    def generate(self, prompt: str) -> str:
        self._enforce_policy()
        response = self.client.chat.completions.create(
            model=self.model,
            messages=[{"role": "user", "content": prompt}]
        )
        return response.choices[0].message.content

    def embed(self, text: str):
        # Default to small embedding model if using official OpenAI
        # Subclasses or specific instances might override model choice
        response = self.client.embeddings.create(
            input=text,
            model="text-embedding-3-small"
        )
        return response.data[0].embedding


class ArkGroqConnector(ArkOpenAIConnector):
    """Connector for Groq (Llama 3, Mixtral) via OpenAI Protocol."""
    def __init__(self, model="llama3-70b-8192", api_key=None, **kwargs):
        super().__init__(
            model=model,
            api_key=api_key,
            base_url="https://api.groq.com/openai/v1",
            env_key="GROQ_API_KEY",
            **kwargs
        )

class ArkDeepSeekConnector(ArkOpenAIConnector):
    """Connector for DeepSeek via OpenAI Protocol."""
    def __init__(self, model="deepseek-chat", api_key=None, **kwargs):
        super().__init__(
            model=model,
            api_key=api_key,
            base_url="https://api.deepseek.com",
            env_key="DEEPSEEK_API_KEY",
            **kwargs
        )
