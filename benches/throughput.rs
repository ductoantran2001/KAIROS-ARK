//! Throughput benchmark for KAIROS-ARK scheduler.

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_node_dispatch(c: &mut Criterion) {
    use kairos_ark::core::{Graph, Node, Scheduler};
    
    // Create a graph with 10,000 lightweight nodes
    let mut graph = Graph::new();
    for i in 0..10000 {
        graph.add_node(Node::task(format!("n{}", i), "noop"));
    }
    graph.set_entry("n0");
    
    let scheduler = Scheduler::new(graph);
    scheduler.register_handler("noop", |_, _| Ok(String::new()));
    
    c.bench_function("dispatch_10k_nodes", |b| {
        b.iter(|| {
            // Execute graph
            let _ = black_box(scheduler.execute());
        });
    });
}

fn benchmark_logical_clock(c: &mut Criterion) {
    use kairos_ark::core::LogicalClock;
    use std::sync::Arc;
    
    let clock = Arc::new(LogicalClock::new());
    
    c.bench_function("clock_tick", |b| {
        b.iter(|| {
            let _ = black_box(clock.tick());
        });
    });
}

fn benchmark_ledger_append(c: &mut Criterion) {
    use kairos_ark::core::{AuditLedger, EventType};
    use kairos_ark::core::ledger::Event;
    
    let ledger = AuditLedger::new();
    
    c.bench_function("ledger_append", |b| {
        let mut ts = 0u64;
        b.iter(|| {
            ts += 1;
            ledger.append(Event::new(
                ts,
                "node".to_string(),
                EventType::Start,
                None,
            ));
        });
    });
}

criterion_group!(benches, benchmark_node_dispatch, benchmark_logical_clock, benchmark_ledger_append);
criterion_main!(benches);
