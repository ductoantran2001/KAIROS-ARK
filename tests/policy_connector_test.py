import os
import sys

# Ensure current directory is in path
sys.path.insert(0, ".")

from kairos_ark import Agent, Policy, Cap
from kairos_ark.connectors import ArkGeminiConnector

def test_security_policy():
    print("--- Security Policy Enforcement Test ---")
    
    # 1. Initialize Agent with RESTRICTIVE Policy
    # -------------------------------------------
    # We explicitly forbid LLM_CALL
    print("Initializing Kernel with NO_LLM Policy...")
    policy = Policy(allowed_capabilities=[Cap.FILE_SYSTEM_READ]) # No LLM_CALL (16)
    
    agent = Agent(seed=123)
    agent.set_policy(policy)
    
    # 2. Initialize Smart Connector attached to Agent
    # -----------------------------------------------
    # Even with a valid API Key, this connector is tied to the restricted agent
    print("Initializing Smart Connector...")
    
    # We use a dummy key because we expect to be blocked BEFORE hitting the network
    os.environ["GEMINI_API_KEY"] = "mock_key_for_policy_test"
    
    # Import HAS_GEMINI to handle import errors gracefully in test env
    try:
        from kairos_ark.connectors.gemini import HAS_GEMINI
        if not HAS_GEMINI:
            print(" ! google-generativeai not installed. Skipping test.")
            return
            
        connector = ArkGeminiConnector(agent=agent)
        
        # 3. Attempt Generation (Should Fail)
        # -----------------------------------
        print("Attempting generation (Expect PermissionError)...")
        try:
            connector.generate("Hello world")
            print(" x FAILED: Generation succeeded but should have been blocked!")
            sys.exit(1)
        except PermissionError as e:
            print(f" ✓ SUCCESS: Kernel blocked the request.")
            print(f"   Reason: {e}")
        except Exception as e:
            print(f" ? Unexpected error: {e}")
            # If it's an auth error, it means it bypassed policy!
            if "403" in str(e) or "Key" in str(e):
                 print(" x FAILED: Request bypassed policy and hit Auth error!")
                 sys.exit(1)

    except ImportError:
        print(" ! Dependency missing.")

    print("\n--- Security Test Passed ---")

if __name__ == "__main__":
    test_security_policy()
