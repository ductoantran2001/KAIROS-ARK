import os
from .base import ArkBaseConnector

try:
    import google.generativeai as genai
    HAS_GEMINI = True
except ImportError:
    HAS_GEMINI = False

class ArkGeminiConnector(ArkBaseConnector):
    """Connector for Google Gemini models via google-generativeai."""
    
    def __init__(self, model_name="gemini-2.0-flash-lite", api_key=None):
        if not HAS_GEMINI:
            raise ImportError("ArkGeminiConnector requires 'google-generativeai'. Install via pip.")
        
        self.api_key = api_key or os.getenv("GEMINI_API_KEY")
        if not self.api_key:
            raise ValueError("GEMINI_API_KEY required.")
            
        genai.configure(api_key=self.api_key)
        self.model = genai.GenerativeModel(model_name)
        
    def generate(self, prompt: str) -> str:
        response = self.model.generate_content(prompt)
        return response.text

    def embed(self, text: str):
        result = genai.embed_content(
            model="models/text-embedding-004",
            content=text,
            task_type="retrieval_document"
        )
        return result['embedding']
