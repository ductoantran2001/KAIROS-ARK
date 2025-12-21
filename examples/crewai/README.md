# CrewAI Integration Example

This example demonstrates running a CrewAI agent on the ARK Kernel.

## Why use ARK?
Standard CrewAI "Process" execution relies on Python loops which can be non-deterministic due to race conditions in tool outputs. ARK enforces logical clock ordering, ensuring every run is reproducible.

## Setup

```bash
pip install crewai kairos-ark
```

## Running the Demo

```bash
python deterministic_crew_demo.py
```
