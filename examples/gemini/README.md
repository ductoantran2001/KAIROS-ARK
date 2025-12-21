# Gemini Integration Example

This example connects ARK to Google's **Gemini 2.0 Flash** model.

## Features
- Unified Connector interface
- "Mock Mode" for testing without API keys (CI/CD friendly)
- Zero-Copy ready architecture

## Setup

```bash
pip install google-generativeai kairos-ark
```

## Running the Demo

```bash
# With API Key (Live)
export GEMINI_API_KEY="your_key"
python ark_connector_demo.py

# Without API Key (Mock/Dry-Run)
python ark_connector_demo.py
```
