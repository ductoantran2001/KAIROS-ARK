import os
try:
    import google.generativeai as genai
    HAS_GEMINI = True
except ImportError:
    HAS_GEMINI = False

class ArkAIConnector:
    """Unified connector for ARK's native state store and Gemini models."""
    
    def __init__(self, model_name="gemini-2.0-flash", api_key=None):
        if not HAS_GEMINI:
            raise ImportError("ArkAIConnector requires 'google-generativeai' package. Install it via pip.")
        
        self.api_key = api_key or os.getenv("GEMINI_API_KEY")
        if not self.api_key:
            raise ValueError("GEMINI_API_KEY not found in environment or arguments.")
            
        genai.configure(api_key=self.api_key)
        self.model = genai.GenerativeModel(model_name)
        
    def generate(self, prompt: str) -> str:
        """
        Generate content using Gemini.
        In a full implementation, this could leverage ARK's zero-copy memory 
        to pass large contexts directly if the Python bindings supported it natively.
        """
        response = self.model.generate_content(prompt)
        return response.text

    def embed(self, text: str):
        """
        Generate embeddings for text.
        """
        result = genai.embed_content(
            model="models/text-embedding-004",
            content=text,
            task_type="retrieval_document"
        )
        return result['embedding']
