
import os
import sys
import time
import statistics
from kairos_ark.connectors import ArkCohereConnector

# Ensure imports work from project root
sys.path.insert(0, ".")

def benchmark_cohere(n_runs=5):
    print(f"\n🚀 Benchmarking Cohere Connector (n={n_runs})")
    print(f"   Model: command-r-08-2024")
    print("=" * 60)

    api_key = os.getenv("COHERE_API_KEY")
    if not api_key:
        print("! COHERE_API_KEY missing.")
        return

    connector = ArkCohereConnector(model="command-r-08-2024", api_key=api_key)
    prompt = "Explain the importance of deterministic execution in AI agents in one short sentence."

    latencies = []
    
    for i in range(n_runs):
        print(f"   Run {i+1}/{n_runs}...", end="", flush=True)
        start = time.perf_counter()
        try:
            _ = connector.generate(prompt)
            duration_ms = (time.perf_counter() - start) * 1000
            latencies.append(duration_ms)
            print(f" {duration_ms:.2f}ms")
        except Exception as e:
            print(f" Failed: {e}")

    if not latencies:
        print("No successful runs.")
        return

    avg = statistics.mean(latencies)
    median = statistics.median(latencies)
    min_l = min(latencies)
    max_l = max(latencies)

    print("-" * 60)
    print(f"📊 Results:")
    print(f"   Average Latency: {avg:.2f} ms")
    print(f"   Median Latency:  {median:.2f} ms")
    print(f"   Min Latency:     {min_l:.2f} ms")
    print(f"   Max Latency:     {max_l:.2f} ms")
    print("=" * 60)
    
    # Save results to file
    with open("cohere_benchmark_results.txt", "w") as f:
        f.write("KAIROS-ARK Cohere Benchmark Results\n")
        f.write("===================================\n")
        f.write(f"Model: command-r-08-2024\n")
        f.write(f"Runs: {n_runs}\n\n")
        f.write(f"Average: {avg:.2f} ms\n")
        f.write(f"Median:  {median:.2f} ms\n")
        f.write(f"Min:     {min_l:.2f} ms\n")
        f.write(f"Max:     {max_l:.2f} ms\n")

if __name__ == "__main__":
    benchmark_cohere()
