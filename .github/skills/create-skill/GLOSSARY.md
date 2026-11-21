# Skill glossary

This file defines the terms that [create-skill](SKILL.md) uses. The rules are
in `SKILL.md`.

## Contents

- [Invocation](#invocation)
- [Structure](#structure)
- [Quality](#quality)
- [Failure modes](#failure-modes)

## Invocation

### Model-invoked skill

A model-invoked skill has a description that lets the agent find and use it.
Other skills can also start it.

### User-invoked skill

A user-invoked skill runs only when the user names it.

### Description

The description is the machine-readable trigger of a skill. It states the
action and each distinct request branch.

### Context load

Context load is the text and attention cost that a skill adds before it runs.

## Structure

### Branch

A branch is one distinct way to start or run a skill.

### Steps

Steps are the ordered actions that the agent does.

### Completion criterion

A completion criterion states the observable condition that ends a step or a
task. A strong criterion is clear, checkable, and exhaustive.

### Post-completion steps

Post-completion steps are the steps that come after the current step. If the
agent can see later work, that work can pull attention from the current criterion.

### Reference

Reference is information that the agent reads when a branch needs it. Examples
are rules, facts, schemas, and examples.

### External reference

External reference is information outside the skill package, for example a
document in `docs/`. More than one skill can use it.

### Information hierarchy

The information hierarchy ranks content by the time when the agent needs it:
steps first, shared reference second, and branch-only reference last.

### Progressive disclosure

Progressive disclosure keeps branch-only reference out of `SKILL.md`. A context
pointer tells the agent when to read that reference.

### Context pointer

A context pointer names material that is not in the context and states when
to read it. Its words control whether the agent finds the material.

### Granularity

Granularity is the division of work across skills and reference files.

## Quality

### Predictability

Predictability is the degree to which a skill gives the same process on each
run. The output can change with the request.

### Single source of truth

A single source of truth gives each instruction, definition, or contract one
owner.

## Failure modes

### Premature completion

Premature completion ends a step before the step meets its criterion. Usually
the criterion is not precise, or post-completion steps pull attention away.

### Duplication

Duplication gives one meaning more than one owner. It increases the context
load, and the copies can disagree.

### Sediment

Sediment is stale text that stays after the skill or the repository changes.

### Sprawl

Sprawl is too much skill length, even when each line is current.

### No-op

A no-op instruction does not change agent behavior.
