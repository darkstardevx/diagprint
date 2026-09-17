# diagprint implementation plans

Rust implementation is plan-first.

The active plan is named by `.plans/ACTIVE`.

Plan states:
- Status: Draft
- Status: Approved
- Status: Complete

The pre-commit hook reads the active plan from HEAD, so the plan must be
committed before Rust implementation.

Typical flow:

1. `./scripts/plan new M4-causal-graph`
2. Edit the plan.
3. `./scripts/plan approve`
4. `git add .plans/ && git commit`
5. Implement Rust.
6. After milestone completion: `./scripts/plan close`
