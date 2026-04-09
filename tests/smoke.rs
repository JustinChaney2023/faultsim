use faultsim::clock::Clock;
use faultsim::config::DetectorStrategy;
use faultsim::engine::Engine;
use faultsim::event::{Event, EventKind, EventQueue};
use faultsim::network::{Network, NetworkConfig};
use faultsim::node::Node;
use faultsim::scenario;

#[test]
fn clock_advances_monotonically() {
    let mut clock = Clock::new();
    assert_eq!(clock.now(), 0);
    clock.advance_to(100);
    assert_eq!(clock.now(), 100);
    clock.advance_to(200);
    assert_eq!(clock.now(), 200);
}

#[test]
fn event_queue_ordering() {
    let mut q = EventQueue::new();
    q.schedule(Event {
        tick: 50,
        kind: EventKind::HeartbeatSend { from: 1 },
    });
    q.schedule(Event {
        tick: 10,
        kind: EventKind::NodeCrash { node: 2 },
    });
    q.schedule(Event {
        tick: 30,
        kind: EventKind::DetectorTick { node: 3 },
    });

    let first = q.pop().unwrap();
    assert_eq!(first.tick, 10);
    let second = q.pop().unwrap();
    assert_eq!(second.tick, 30);
    let third = q.pop().unwrap();
    assert_eq!(third.tick, 50);
}

#[test]
fn node_state_transitions() {
    let mut node = Node::new(1);
    assert!(node.is_alive());
    node.crash();
    assert!(!node.is_alive());
    node.recover();
    assert!(node.is_alive());
}

#[test]
fn network_delivers_within_bounds() {
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    let net = Network::new(NetworkConfig {
        base_latency: 10,
        jitter: 5,
        drop_probability: 0.0,
    });
    let mut rng = StdRng::seed_from_u64(0);

    for _ in 0..50 {
        let tick = net.delivery_tick(1, 2, 100, &mut rng).unwrap();
        assert!(tick >= 110 && tick <= 115);
    }
}

#[test]
fn engine_runs_to_completion() {
    let mut engine = Engine::new(1000);
    engine.queue.schedule(Event {
        tick: 100,
        kind: EventKind::HeartbeatSend { from: 1 },
    });
    engine.queue.schedule(Event {
        tick: 500,
        kind: EventKind::NodeCrash { node: 2 },
    });
    engine.run();
    // Engine should have processed both events
    assert_eq!(engine.clock.now(), 500);
}

#[test]
fn config_parses_baseline() {
    let config_path = std::path::Path::new("configs/scenarios/baseline.toml");
    let config = scenario::load_config(config_path).expect("should parse baseline config");
    assert_eq!(config.cluster.node_count, 10);
    assert_eq!(config.simulation.seed, 42);
    assert_eq!(config.detector.strategy, DetectorStrategy::FixedTimeout);
}

#[test]
fn baseline_simulation_no_false_positives() {
    let config_path = std::path::Path::new("configs/scenarios/baseline.toml");
    let config = scenario::load_config(config_path).expect("should parse baseline config");
    let mut engine = scenario::build_engine(&config, None);
    engine.run();

    // Stable network with no crashes should produce messages but no detections.
    assert!(engine.metrics.message_count > 0, "should deliver messages");
    assert!(
        engine.metrics.detections.is_empty(),
        "stable network should have no detections, got {}",
        engine.metrics.detections.len()
    );
    assert_eq!(engine.metrics.crashes.len(), 0);
}

#[test]
fn crash_detected_as_true_positive() {
    let config_path = std::path::Path::new("configs/scenarios/baseline.toml");
    let config = scenario::load_config(config_path).expect("should parse baseline config");
    let mut engine = scenario::build_engine(&config, None);

    // Inject a crash at tick 500 for node 1.
    engine.queue.schedule(Event {
        tick: 500,
        kind: EventKind::NodeCrash { node: 1 },
    });

    engine.run();

    // At least one detection should be a true positive for node 1.
    let tp_for_node1 = engine
        .metrics
        .detections
        .iter()
        .any(|d| d.node == 1 && d.true_positive);
    assert!(
        tp_for_node1,
        "node 1 crash should be detected as true positive"
    );
}

#[test]
fn deterministic_replay() {
    let config_path = std::path::Path::new("configs/scenarios/baseline.toml");
    let config = scenario::load_config(config_path).expect("should parse baseline config");

    let mut engine1 = scenario::build_engine(&config, Some(123));
    engine1.run();

    let mut engine2 = scenario::build_engine(&config, Some(123));
    engine2.run();

    assert_eq!(engine1.metrics.message_count, engine2.metrics.message_count);
    assert_eq!(
        engine1.metrics.detections.len(),
        engine2.metrics.detections.len()
    );
}
