# LangGraph Integration Example

This example demonstrates how to use KAIROS-ARK as a **native checkpointer** for LangGraph.

## Why use ARK?
ARK provides a Zero-Copy State Store that persists graph state in microseconds (~4µs) rather than milliseconds, eliminating the serialization bottleneck of standard checkpoints.

## Setup

```bash
pip install langgraph kairos-ark
```

## Running the Demo

```bash
python native_checkpoint_demo.py
```
