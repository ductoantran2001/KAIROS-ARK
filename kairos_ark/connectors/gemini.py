import os
from .base import ArkBaseConnector

try:
    import google.generativeai as genai
    HAS_GEMINI = True
except ImportError:
    HAS_GEMINI = False

class ArkGeminiConnector(ArkBaseConnector):
    """Connector for Google Gemini models via google-generativeai."""
    
    def __init__(self, model_name="gemini-2.0-flash-lite", embedding_model="models/text-embedding-004", api_key=None, agent=None):
        super().__init__(agent=agent)
        if not HAS_GEMINI:
            raise ImportError("ArkGeminiConnector requires 'google-generativeai'. Install via pip.")
        
        self.api_key = api_key or os.getenv("GEMINI_API_KEY")
        if not self.api_key:
            raise ValueError("GEMINI_API_KEY required.")
            
        genai.configure(api_key=self.api_key)
        self.model = genai.GenerativeModel(model_name)
        self.embedding_model = embedding_model
        
    def generate(self, prompt: str) -> str:
        self._enforce_policy()
        response = self.model.generate_content(prompt)
        return response.text

    def embed(self, text: str):
        self._enforce_policy()
        result = genai.embed_content(
            model=self.embedding_model,
            content=text,
            task_type="retrieval_document"
        )
        return result['embedding']
