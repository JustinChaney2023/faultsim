# Research Proposal

## Title

Failure Misclassification in Distributed Clusters: A Simulation Study Under Jitter, Churn, and Partitions

## Motivation

Failure detection is fundamental to distributed systems. Protocols like heartbeat-based monitoring and gossip dissemination underpin service discovery, consensus, and replication. However, real-world networks exhibit conditions — jitter, asymmetric delay, node churn, and partial partitions — that cause failure detectors to *misclassify* healthy nodes as failed. These false positives trigger unnecessary recovery actions, degrade availability, and can cascade into broader instability.

Despite extensive theoretical work on failure detectors (Chandra & Toueg, 1996) and practical implementations in systems like Cassandra and Akka, there is limited systematic study of how different detection strategies compare under controlled, reproducible adverse conditions. Most evaluations are either analytical (assuming idealized models) or empirical on specific production systems (where variables are difficult to isolate).

## Research Question

Under what network conditions do common failure-detection strategies misclassify healthy nodes as failed, and how do adaptive or gossip-assisted approaches compare to fixed-timeout methods?

## Approach

We propose a discrete-event simulation framework (`faultsim`) that models a cluster of nodes communicating via heartbeat messages over a configurable network. The simulator allows precise control over:

- **Network delay distributions** (uniform, normal, heavy-tailed)
- **Jitter profiles** (periodic, bursty, correlated)
- **Node churn** (join/leave rates, crash-recovery patterns)
- **Partition topologies** (full, partial, asymmetric)

We implement three failure-detection strategies as pluggable modules:

1. **Fixed-timeout heartbeat detection** — a baseline that declares failure after a static timeout
2. **Adaptive-timeout detection** — adjusts thresholds based on observed message arrival times (inspired by Phi Accrual and TCP RTT estimation)
3. **Gossip-assisted suspicion** — augments local detection with disseminated suspicion scores (inspired by SWIM and Lifeguard)

## Evaluation Metrics

| Metric | Definition |
|---|---|
| Detection latency | Time from actual failure event to detector declaring the node failed |
| False positive rate | Fraction of detection events where a healthy node is declared failed |
| Recovery time | Time from a transient fault resolving to the cluster reaching a correct membership view |
| Messaging overhead | Total number of messages per detection cycle |

## Expected Contributions

1. A reusable, open-source simulation framework for failure-detection research
2. Systematic comparison of three detection strategies across a range of adverse conditions
3. Identification of network regimes where each strategy excels or fails
4. Quantitative analysis of the false-positive/detection-latency tradeoff

## Key References

- Chandra, T. D., & Toueg, S. (1996). Unreliable failure detectors for reliable distributed systems. *JACM*, 43(2), 225–267.
- Das, A., Gupta, I., & Motivala, A. (2002). SWIM: Scalable weakly-consistent infection-style process group membership protocol. *DSN*.
- Hayashibara, N., Défago, X., Yared, R., & Katayama, T. (2004). The Phi Accrual Failure Detector. *SRDS*.
- Hashicorp. (2017). Lifeguard: SWIM-ing with Situational Awareness. *HashiCorp Research*.
