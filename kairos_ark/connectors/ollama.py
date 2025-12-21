from .base import ArkBaseConnector

try:
    import ollama
    HAS_OLLAMA = True
except ImportError:
    HAS_OLLAMA = False

class ArkOllamaConnector(ArkBaseConnector):
    """Connector for Local Ollama models."""
    
    def __init__(self, model="llama3"):
        if not HAS_OLLAMA:
            raise ImportError("ArkOllamaConnector requires 'ollama' package.")
        self.model = model

    def generate(self, prompt: str) -> str:
        response = ollama.chat(model=self.model, messages=[
            {'role': 'user', 'content': prompt},
        ])
        return response['message']['content']

    def embed(self, text: str):
        response = ollama.embeddings(model=self.model, prompt=text)
        return response['embedding']
