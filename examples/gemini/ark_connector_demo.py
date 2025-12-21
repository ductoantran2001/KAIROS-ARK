import os
import sys
import time

# Ensure we can import from local source
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '../../')))

from kairos_ark.connectors import ArkAIConnector

def main():
    print("--- Gemini + ARK Connector Demo ---")
    
    # 1. Configuration
    # ----------------
    api_key = os.getenv("GEMINI_API_KEY")
    if not api_key:
        print("No GEMINI_API_KEY found. Running in MOCK mode.")
        api_key = "mock_key"
        
    # 2. Initialize Connector
    # -----------------------
    try:
        connector = ArkAIConnector(model_name="gemini-2.0-flash-lite", api_key=api_key)
        print("Connector initialized.")
        
        # 3. Generate Content
        # -------------------
        prompt = "Explain why deterministic execution is important for AI agents."
        print(f"\nPrompt: '{prompt}'")
        
        if api_key != "mock_key":
            print("\nSending to Gemini...")
            start = time.perf_counter()
            response = connector.generate(prompt)
            print(f"Response ({time.perf_counter()-start:.2f}s):")
            print(f"{response[:100]}...")
        else:
            print("\n[MOCK] Skipping live API call.")
            print("Response would be: 'Deterministic execution ensures reproducibility...'")
            
    except ImportError:
        print(" ! google-generativeai not installed.")
    except Exception as e:
        print(f" ! Error: {e}")

    print("\n--- Success ---")

if __name__ == "__main__":
    main()
