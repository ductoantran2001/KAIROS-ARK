import os
import requests

class ArkTools:
    """Collection of native tools optimized for ARK integration."""
    
    @staticmethod
    def tavily_search(query: str, api_key: str = None) -> list:
        """ARK Tool: AI-optimized web search via Tavily."""
        key = api_key or os.getenv("TAVILY_API_KEY")
        if not key:
            raise ValueError("TAVILY_API_KEY required")
            
        url = "https://api.tavily.com/search"
        payload = {"api_key": key, "query": query, "search_depth": "basic"}
        
        try:
            response = requests.post(url, json=payload, timeout=10)
            response.raise_for_status()
            data = response.json()
            return data.get('results', [])
        except Exception as e:
            return [{"error": str(e)}]

    @staticmethod
    def firecrawl_scrape(url: str, api_key: str = None) -> str:
        """ARK Tool: Convert website to clean Markdown via Firecrawl."""
        key = api_key or os.getenv("FIRECRAWL_API_KEY")
        if not key:
            raise ValueError("FIRECRAWL_API_KEY required")
            
        # Mock implementation matching the architectural pattern
        # In a real deployed scenario, this would hit the Firecrawl API
        # header = {"Authorization": f"Bearer {key}"}
        # resp = requests.get(f"https://api.firecrawl.dev/v1/scrape?url={url}", headers=header)
        return f"[MOCK] Scraped content for {url}"

