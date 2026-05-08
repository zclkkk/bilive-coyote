# AGENTS.md

## Project Philosophy

This project is a small Rust real-time control system. It bridges Bilibili live events and manual controls to DG-LAB Coyote output through typed state, explicit ownership, and clear runtime boundaries.

Favor domain adequacy: keep what the domain requires, remove what the domain does not justify.

## Principles

### Meaningful Adequacy

Prefer direct, explicit, easy-to-justify code. Avoid layers, wrappers, fallback paths, or abstractions that do not carry real domain meaning.

Before adding code, ask: What concept does this represent? Who owns this state? Why does this boundary exist? Is this adequate for correctness, safety, and clarity without adding unjustified weight?

### Ownership Shapes Design

Use Rust ownership as a design tool. State should have a clear owner. Mutation should happen where ownership naturally belongs. Shared state should be introduced only when sharing is part of the domain, not merely to make code easier to wire.

Prefer designs where invalid or contradictory states are hard to represent.

### Validate Boundaries, Trust Invariants

External inputs are unreliable and should be parsed, validated, and classified carefully.

Inside the system, avoid redundant defensive logic that hides broken assumptions. Do not silently repair internal invariant failures with default values unless the default is a real domain rule.

A good boundary makes the inside simpler.

### Model the Domain

Choose types, modules, and communication patterns based on the current domain. Do not add generic glue, event buses, dynamic objects, or broad abstractions unless they express something real.

The structure should follow ownership, lifecycle, protocol boundaries, and state transitions.

### Best Practices Reduce Noise

Use engineering practices to make the project more reliable, reproducible, and understandable. Tests, docs, CI, errors, and abstractions should protect important behavior or clarify intent, not inflate the project.

## Refactoring Guidance

Actively look for opportunities to simplify the system in ways that match the project philosophy.

Good refactoring candidates include code that only forwards data without owning meaning, duplicated validation after a trusted boundary, shared mutable state that could have a clearer owner, types that allow impossible or contradictory states, fallback behavior that hides broken assumptions, abstractions that do not represent a real domain concept, and implementation details that obscure lifecycle, protocol, or state transitions.

Before proposing or applying a refactor, explain the improvement in terms of clearer ownership, smaller state space, stronger boundaries, less glue, fewer invalid states, or more explicit domain meaning.

Prefer small, reviewable refactors. Do not rewrite broadly just because code can be made more elegant.

## Change Style

Prefer small, focused changes. When refactoring, favor clearer ownership, fewer false abstractions, smaller state space, stronger boundaries, less duplicated validation, and more explicit domain meaning.

Avoid broad rewrites unless the task is explicitly architectural.

## Project Ethos

This project should feel like a small, typed, ownership-driven real-time system.

Guiding sentence:

> Keep what is adequate, remove what is unjustified, give state a clear owner, make boundaries honest, and let Rust express the shape of the domain.
