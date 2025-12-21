import os
import sys

# Ensure we can import from local source for testing
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '../../')))

from kairos_ark import Agent
from kairos_ark.integrations.langgraph import ArkNativeCheckpointer

def main():
    print("--- LangGraph + KAIROS-ARK Demo ---")
    
    # 1. Initialize ARK Agent (The Kernel)
    # -------------------------------------
    print("Initializing ARK Kernel...")
    ark_agent = Agent(seed=42)
    
    # 2. Configure LangGraph Checkpointer
    # -----------------------------------
    # This adapter routes all LangGraph state persistence to the Rust kernel
    checkpointer = ArkNativeCheckpointer(ark_agent)
    print("Attached Native Checkpointer to ARK Kernel.")
    
    # 3. Simulate LangGraph Workflow
    # ------------------------------
    # (Normally you would use `workflow.compile(checkpointer=checkpointer)`)
    
    thread_id = "conversation_123"
    print(f"\nSimulating conversation step for thread: {thread_id}")
    
    # State 1: User says Hello
    state_v1 = {
        "v": 1,
        "ts": "2025-01-01T10:00:00Z",
        "channel_values": {
            "messages": [{"role": "user", "text": "Hello ARK"}]
        }
    }
    
    # Save State to Kernel
    config = {"configurable": {"thread_id": thread_id}}
    checkpointer.put(config, state_v1, {}, {})
    print(f" Saved state v1 to Kernel Store.")
    
    # State 2: AI Responds
    state_v2 = {
        "v": 2,
        "ts": "2025-01-01T10:00:01Z",
        "channel_values": {
            "messages": [
                {"role": "user", "text": "Hello ARK"},
                {"role": "ai", "text": "Hello! I am running on the native kernel."}
            ]
        }
    }
    
    # Save State v2
    checkpointer.put(config, state_v2, {}, {})
    print(f" Saved state v2 to Kernel Store.")
    
    # 4. Durable Retrieval
    # --------------------
    # Later, retrieve the state directly from Rust memory
    print("\nRetrieving state from Kernel...")
    retrieved = checkpointer.get_tuple(config)
    
    last_msg = retrieved["channel_values"]["messages"][-1]
    print(f" Retrieved State v{retrieved['v']}")
    print(f" Latest Message: '{last_msg['text']}'")
    
    print("\n--- Success ---")

if __name__ == "__main__":
    main()
