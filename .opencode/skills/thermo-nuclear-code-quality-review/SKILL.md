---
name: thermo-nuclear-code-quality-review
description: Run an extremely strict maintainability review for abstraction quality, giant files, and spaghetti-condition growth. Use for a thermo-nuclear code quality review, thermonuclear review, deep code quality audit, or especially harsh maintainability review.
disable-model-invocation: true
---

# Thermo-Nuclear Code Quality Review

Use this skill for an unusually strict review focused on implementation quality, maintainability, abstraction quality, and codebase health.

Above all, this skill should push the reviewer to be ambitious about code structure. Do not merely identify local cleanup opportunities. Actively search for code-judo restructurings that preserve behavior while making the implementation dramatically simpler, smaller, more direct, and more elegant.

## Core Prompt

Perform a deep code quality audit of the current branch's changes.
Rethink how to structure or implement the changes to meaningfully improve code quality without impacting behavior.
Work to improve abstractions, modularity, reduce spaghetti code, improve succinctness and legibility.
Be ambitious if there is a clear path to improving the implementation that involves restructuring some of the codebase.
Be extremely thorough and rigorous. Measure twice, cut once.

## Review Priorities

1. Structural code-quality regressions
2. Missed opportunities for dramatic simplification
3. Spaghetti or branching complexity increases
4. Boundary, abstraction, and type-contract problems
5. File-size and decomposition concerns
6. Modularity and abstraction issues
7. Legibility and maintainability concerns

## Approval Bar

Do not approve merely because behavior seems correct.
The change should have no clear structural regression, no obvious spaghetti growth, no unnecessary abstraction churn, and no visible missed opportunity for a much simpler design.
