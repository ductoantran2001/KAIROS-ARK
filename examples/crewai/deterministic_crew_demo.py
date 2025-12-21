import os
import sys

# Ensure we can import from local source
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '../../')))

from kairos_ark.integrations.crewai import ArkCrewAdapter

def main():
    print("--- CrewAI + KAIROS-ARK Demo ---")
    
    # 1. Define Dummy Agents (Mocking CrewAI for demo if not installed)
    # ----------------------------------------------------------------
    # In a real app: from crewai import Agent, Task
    print("Defining Agents...")
    
    class MockAgent:
        def __init__(self, role, goal):
            self.role = role
            self.goal = goal
            
    researcher = MockAgent(
        role='Researcher', 
        goal='Analyze System Performance'
    )
    
    # 2. Define Tasks
    # ---------------
    print("Defining Tasks...")
    tasks = [
        {"description": "Check kernel latency", "agent": researcher}
    ]
    
    # 3. Create Deterministic Crew
    # ----------------------------
    # The adapter configures the 'process' flow to use ARK's deterministic scheduling
    print("\nInitializing ARK-Orchestrated Crew...")
    try:
        crew = ArkCrewAdapter.create_deterministic_crew(
            agents=[researcher],
            tasks=tasks
        )
        print(" Crew created successfully.")
        print(f" Process strategy: {crew.process}")
        
    except ImportError:
        print(" ! CrewAI not installed. This is expected in the base checking environment.")
        print("   (Install 'crewai' to run full orchestration)")

    print("\n--- Success ---")

if __name__ == "__main__":
    main()
