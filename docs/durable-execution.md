# Durable Execution with Graph Database

The graph database provides persistent storage for execution history, enabling agents to pause, resume, and inspect their execution state at any point. This architecture supports long-running agents that span multiple sessions and survive system restarts.

## Storage Model

The execution history stores as a graph where contexts form a tree structure connected by parent-child relationships. Each context node represents a scope in the execution, containing variables, events, and metadata. Events capture the sequence of operations performed within each context. Variables track declarations and assignments throughout the scope chain.

Kuzu serves as the embedded graph database, requiring no separate server process. The schema defines node tables for contexts, events, variables, and parameters, with relationship tables connecting them. This structure mirrors the runtime context hierarchy while adding temporal ordering and persistence.

## Schema Design

Context nodes store an identifier, creation timestamp, scope boundary flag, return value presence indicator, and depth in the tree. The depth allows efficient querying of execution levels without traversing relationships.

Event nodes capture individual operations with a unique identifier, optional name, timestamp, sequence number within the context, and serialized bytecode. The sequence number preserves ordering when multiple events occur in the same context.

Variable nodes record each variable operation with an identifier, variable name, declaration flag, timestamp, and serialized bytecode representing the value. Both declarations and assignments create variable nodes, distinguished by the operation type.

Parameter nodes hold function or event parameters, storing the parameter name and serialized bytecode value.

The parent-of relationship connects contexts in the execution tree, flowing from parent to child. The has-event relationship links contexts to their events, carrying a local index property for ordering. The has-variable relationship connects contexts to variables, annotated with the operation type (declare or assign). Parameter relationships connect events and variables to their parameter nodes, ordered by position.

## Query Patterns

Retrieving the full execution path requires matching from root contexts (those with no incoming parent relationships) to leaf contexts (those with no outgoing parent relationships), returning the path ordered by depth.

Chronological event sequences within a context subtree match the context and all descendants using variable-length path patterns, joining to events through has-event relationships, then ordering by descendant depth and local index.

Tracking variable assignments across scopes follows the context tree from a starting point, matching all has-variable relationships to variable nodes, filtering by variable name, and ordering by timestamp to show the assignment history.

Finding events by name requires a simple match on event nodes with the specified name property, joining to their contexts, ordered by timestamp.

## Execution State Persistence

When execution yields or pauses, the VM serializes its state to the graph database. This snapshot includes the current program counter, function name, evaluation stack contents, call stack frames, and context identifier. The bytecode program references a version identifier stored separately.

Context state serialization walks the context tree from the current context to the root, storing each context's variables as variable nodes and events as event nodes. The serialization preserves the parent-child relationships and scope boundary markers.

The execution snapshot combines three components: the bytecode version identifier, the serialized VM state, and the root context identifier in the graph. These three pieces provide complete restoration capability.

## Restoration Process

Restoring execution begins by loading the bytecode program for the specified version. The VM state deserializes to reconstruct the program counter, stacks, and current function. The context tree rebuilds by querying the graph database starting from the root context identifier, following parent-of relationships to reconstruct the hierarchy.

Variable values deserialize from their bytecode representation into runtime values. Event history loads from event nodes in sequence order. The VM resumes execution from the saved program counter with the restored state.

## Temporal Queries

The graph structure enables querying execution history at any point in time. Finding the state at a specific timestamp matches contexts and events created before that time, ordering by timestamp to reconstruct the historical state.

Comparing execution between two points involves querying snapshots at each timestamp and computing the difference in contexts, variables, and events. This supports debugging by showing what changed between states.

Time travel debugging loads a historical snapshot and resumes execution from that point, potentially with modified bytecode or context state to test alternative paths.

## Checkpoint Strategy

Automatic checkpoints occur at regular intervals during execution, creating snapshots without pausing the agent. The checkpoint includes the full execution state and a reference timestamp.

Manual checkpoints allow explicit state capture at meaningful points, such as before risky operations or after completing major tasks. These checkpoints carry descriptive labels for later identification.

Checkpoint retention policies manage storage growth by expiring old checkpoints while preserving important milestones. Recent checkpoints retain full detail, while older checkpoints may compress or summarize.

## Advantages Over Traditional Persistence

Traditional databases store application state but not execution state. Resuming requires reconstructing the call stack and program counter from application data, often impossible for complex workflows.

The graph database captures not just what data exists but where in the program execution currently stands. This enables true pause-resume rather than restart-and-recover.

The hierarchical context structure maps naturally to graph relationships. Querying the execution tree uses native graph operations rather than recursive SQL queries.

Temporal ordering comes naturally from timestamps and sequence numbers, avoiding complex versioning schemes in relational databases.

## Integration with Bytecode VM

The VM queries the graph database to check for existing snapshots when starting execution. If a snapshot exists for the requested execution, the VM restores from that point rather than starting fresh.

During execution, yield instructions trigger snapshot creation. The VM serializes its state and issues graph database mutations to create context, event, and variable nodes.

The VM maintains a write-ahead log of operations since the last snapshot, allowing recovery if a crash occurs between snapshots. The log replays against the last snapshot to reach the current state.

## Debugging Capabilities

The graph database enables powerful debugging workflows. Loading any historical snapshot into the VM allows stepping through execution from that point. Modifying variables or bytecode before resuming tests hypothetical scenarios.

Querying the graph reveals execution patterns, such as which functions call which others, how often variables change, or where execution spends the most time. These queries inform optimization efforts.

Comparing snapshots across different execution runs identifies divergence points, useful when debugging non-deterministic behavior or understanding why different inputs produce different outcomes.

## Deployment Model

Production agents ship with both the bytecode program and a checkpoint strategy. Agents checkpoint regularly, with snapshots stored in the graph database. When deploying updates, the system loads the latest checkpoint, applies bytecode changes, and resumes execution.

This model eliminates downtime for long-running agents. Updates apply while preserving accumulated state and conversation history. Users experience continuity rather than disruption.

The graph database replicates to backup storage, ensuring checkpoint durability. Recovery from hardware failure involves loading the latest replicated checkpoint and resuming execution.

## Performance Considerations

Write performance matters during checkpoint creation. Batching graph mutations reduces transaction overhead. Asynchronous checkpoint writing prevents blocking execution, with the VM continuing while the database write completes.

Read performance affects restoration time. Indexes on context identifiers, timestamps, and depth accelerate common queries. The columnar storage format in Kuzu optimizes for analytical queries over the execution history.

Storage growth requires management. Compression reduces checkpoint size without losing information. Pruning old checkpoints based on retention policies prevents unbounded growth while preserving important milestones.

## Comparison to Temporal

Temporal stores workflow execution state as an append-only event log. Each state transition appends an event to the log. Resuming execution replays the entire log from the beginning, rebuilding state by processing each event sequentially. The workflow function executes deterministically, making decisions based on replayed events rather than external calls.

This log-based approach encounters scaling problems with long-running workflows. The event log grows unbounded as execution continues. Temporal imposes size limits on event histories because replaying millions of events becomes prohibitively expensive. When a workflow hits the history size limit, it must continue-as-new, starting a fresh workflow with reset history. This breaks the execution chain and complicates debugging across the boundary.

The graph database architecture sidesteps this limitation through random access. Restoring execution at any point requires only the context subtree relevant to that execution state, not the complete history. Querying the graph with a context identifier retrieves that context and its ancestors in a single operation. No replay occurs. The tree structure provides direct access to any execution point.

Storage costs differ fundamentally. Temporal's log records every state transition, creating redundancy as the same state appears in multiple log positions. The graph stores each context node once, with relationships indicating the execution flow. Variable assignments create new variable nodes rather than duplicating entire state snapshots.

The tree structure also enables efficient pruning. Garbage collection can remove old context subtrees without affecting current execution state. Temporal's append-only log complicates selective deletion because events reference previous events by position. Breaking the log chain corrupts replay semantics.

Query patterns favor the graph model. Finding when a specific variable changed requires scanning the entire Temporal event log. The graph database indexes variable nodes by name and timestamp, returning results directly. Analyzing execution patterns across multiple runs aggregates graph data without replaying histories.

## Garbage Collection

The context tree grows as execution progresses, eventually requiring pruning to manage storage. Garbage collection removes historical context subtrees while preserving the ability to restore current execution.

The active execution path defines the retention boundary. Contexts in the current call stack and their ancestors must remain accessible. The VM maintains references to these contexts, marking them as live. Any context not reachable from the current execution state becomes eligible for collection.

Checkpoint-based retention preserves snapshots at regular intervals while removing intermediate states. A checkpoint marks specific contexts as milestones. Garbage collection removes contexts between checkpoints, keeping only the milestone snapshots. This reduces storage while maintaining the ability to restore from checkpoints.

Time-based retention expires contexts older than a threshold. Recent execution history remains fully detailed. After a retention period, older contexts undergo collection. This policy suits debugging workflows where recent history matters more than distant past.

Reference counting tracks dependencies between contexts. Each context counts how many child contexts reference it. When a context's children are collected and no execution snapshot references it, the reference count reaches zero and collection proceeds. This ensures no dangling references corrupt the graph.

Tombstones mark collected contexts without removing them entirely. The context node remains with minimal metadata indicating collection status. Queries skip tombstoned contexts. This preserves graph structure for analytics while reclaiming storage from events and variables.

Incremental collection processes portions of the context tree over time rather than scanning the entire graph. The collector selects old context subtrees, determines eligibility, and removes them in batches. Spreading collection work across multiple operations prevents long pauses during execution.

Collection policies combine multiple strategies. A typical configuration retains all contexts for the last hour, keeps hourly checkpoints for the last day, preserves daily checkpoints for a week, and maintains weekly snapshots indefinitely. This tiered approach balances storage costs against debugging needs.

The graph database supports efficient deletion of context subtrees. A query identifies contexts older than the retention threshold with no incoming references from newer contexts. Deleting the context node cascades to remove associated events, variables, and parameters through relationship constraints. Batch deletion processes thousands of nodes in a single transaction.

## Comparison to Alternative Approaches

Event sourcing captures state changes as events but typically lacks execution position. Resuming requires replaying all events, with no notion of program counter or call stack.

Process orchestration systems like workflow engines provide pause-resume but with coarse granularity. They checkpoint between steps, not within expressions. The bytecode VM checkpoints at instruction granularity.

Virtual machine snapshots capture entire memory state but lack semantic structure. The graph database provides queryable, analyzable execution history rather than opaque binary dumps.

The architecture combines ideas from all three approaches: event sourcing for history, workflow engines for durable execution, and VM snapshots for complete state capture, while adding graph database queryability and addressing the replay scalability problems inherent in log-based systems.