# ADR 0002: Four-state recoverability semantics

- Status: Accepted
- Date: 2026-11-20

## Context

A three-state model collapses not checked and confirmed absent, causing false-loss claims. Recovery is data-class-specific and evidence-driven.

## Decision

Use exactly four statuses: yes, partial, no, unknown.

Use unknown whenever evidence is missing, stale, incomplete, failed, or contradictory. Device presence does not prove content accessibility. A backup timestamp does not prove coverage. Assess each class independently.

## Consequences

Reports support null timestamps/counts and explicit evidence. Failed connectors cannot downgrade a verified backup to no. Wipe gating rejects unknown for required classes. Unit tests cover truth-table branches and boundary cases.
