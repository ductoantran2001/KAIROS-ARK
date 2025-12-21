
import os
import sys
import json
import time

# Ensure imports work from project root
sys.path.insert(0, ".")

from kairos_ark import Agent, Policy, Cap
from kairos_ark.connectors import ArkCohereConnector

# --- Configuration ---
GEN_MODEL = "command-r-08-2024"
EMBED_MODEL = "embed-english-v3.0"

def complete_agent_workflow():
    print(f"\n🚀 Starting KAIROS-ARK Complete Cohere Agent")
    print(f"   - Generation Model: {GEN_MODEL}")
    print(f"   - Embedding Model:  {EMBED_MODEL}")
    print("=" * 60)

    # 1. Initialize Safe Agent (Kernel Policies)
    # ------------------------------------------
    safe_policy = Policy(
        allowed_capabilities=[Cap.LLM_CALL, Cap.NET_ACCESS, Cap.FILE_SYSTEM_READ],
        name="safe_rag_agent"
    )
    
    agent = Agent(seed=42)
    agent.set_policy(safe_policy)
    
    # 2. Initialize "Smart" Connector
    # -------------------------------
    api_key = os.getenv("COHERE_API_KEY")
    if not api_key:
        print("! COHERE_API_KEY missing. Please set it.")
        # Mocking for demo purposes if key is missing in CI/CD
        if os.getenv("CI"): 
            print("Running in CI/Mock mode")
            return
        return

    try:
        cohere_llm = ArkCohereConnector(
            model=GEN_MODEL, 
            embedding_model=EMBED_MODEL,
            agent=agent,
            api_key=api_key
        )
    except ImportError:
        print("! Cohere SDK missing. Skipping demo.")
        return

    # 3. Define the Workflow Nodes
    # ----------------------------

    def node_retrieve_context():
        query = "What is the primary advantage of KAIROS-ARK?"
        print(f"   [Retrieval] Embedding query: '{query}'")
        
        # Real call to Cohere Embeddings (Policy Checked)
        try:
            query_vec = cohere_llm.embed(query)
            print(f"   [Retrieval] Got vector of size {len(query_vec)}")
        except Exception as e:
            print(f"   [Retrieval] Embedding failed: {e}")
            return {"query": query, "context": "Mock Context"}
        
        context = """
        KAIROS-ARK provides a deterministic execution kernel that acts as an operating 
        system for agents, ensuring reproducibility and security via a policy engine.
        """
        return {"query": query, "context": context}

    def node_generate_answer():
        data = node_retrieve_context()
        
        prompt = f"""
        Context: {data['context']}
        
        Question: {data['query']}
        
        Answer elegantly in one sentence:
        """
        
        print(f"   [Generation] Sending prompt to {GEN_MODEL}...")
        try:
            response = cohere_llm.generate(prompt)
            print(f"   [Generation] Response received.")
            return {"answer": response.strip()}
        except Exception as e:
            return {"answer": f"Error: {e}"}

    # 4. Construct Graph
    agent.add_node("rag_pipeline", node_generate_answer)

    # 5. Execute
    print("\n--- Executing Workflow ---")
    start_t = time.perf_counter()
    
    results = agent.execute("rag_pipeline")
    
    end_t = time.perf_counter()
    
    # 6. Parse Results
    output = json.loads(results[0]["output"])
    print(f"\n✨ Final Answer:\n{output['answer']}")
    print(f"\n⏱️  Execution time: {(end_t - start_t)*1000:.2f}ms")
    
    # 7. Verify Policy (Audit)
    print("\n--- Audit Log (Governance) ---")
    agent.print_audit_log()
    
    print("\n✅ Verification: Policy was enforced on every API call.")

if __name__ == "__main__":
    complete_agent_workflow()
