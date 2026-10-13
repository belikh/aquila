# Orchestration policy — how agents are dispatched on Aquila

This file exists because a transcript audit
(`docs/plan/orchestrator-omissions.md`) found 35 cases where builder agents were
blocked, limited, or forced into stubs/non-TDD work by the **orchestrator's**
choices rather than by the domain. These rules exist to stop that recurring.

## Rules

1. **Freeze the baseline before fan-out.** Before dispatching builders, commit a
   baseline where the toolchain, the crate graph, and the shared types all
   actually exist and the workspace **builds green**. Dispatch against that
   commit. Never fan out against a dirty tree.
2. **No per-agent worktree, no parallel writers.** Do not run several agents
   that write the same tree concurrently. Either dispatch **sequentially**, or
   give each writer its own `git worktree` on its own branch. Concurrent writers
   were the single largest source of wasted work (agents saw each other's
   half-written files).
3. **Provision the toolchain the prompts assume.** If a ticket needs wasm, HA,
   a TOML parser, or a hash crate, it must be present in the dev shell or
   `[workspace.dependencies]` **before** the agent runs — not forbidden in the
   prompt. `nix develop` (Rust + wasm32 + trunk), `nix develop .#ha` (Python +
   Home Assistant).
4. **Do not over-restrict scope.** "Edit ONLY this crate", "do NOT add
   dependencies", "do NOT create issues" are tools for isolation, not policy.
   When the correct fix needs a shared type, a new crate the register already
   names, or a dependency, the prompt must *require* it, not forbid it. Prefer
   "add the dependency if needed, and update `[workspace.dependencies]`" over a
   blanket ban.
5. **Reality must match the register.** Every crate, seam, and file the
   interface register names must exist (even as a compiling scaffold) or be
   struck from the register. A register that names absent artefacts is a trap.
6. **One number, one name.** Tickets are their GitHub issue number; types are
   defined once in `aquila-types`; there is one `CsiFrame` and one on-air layout.
   Never dispatch with two unresolved spellings of the same concept.
7. **Sequence dependent specs.** Do not author specs that define the same
   interface in parallel. Freeze shared interfaces first, then the specs that
   consume them.
8. **Verify, don't trust.** Every dispatched agent must be told the exact build
   command and must return its passing-test tail. A report that says "should
   compile" is a failure.
9. **Honest fallback is allowed; silent accommodation is not.** If something
   genuinely cannot be done here (hardware flash, GPU training), the agent must
   say so loudly and leave a typed failure — never a fake number, a stub that
   looks real, or a hand-rolled re-implementation of a shared utility.

## The check before any fan-out

- [ ] `nix develop --command cargo test --workspace` is green at the baseline commit.
- [ ] Every crate in `docs/spec/interface-register.md` exists or is struck.
- [ ] One definition each of `CsiFrame`, `FeatureVector`, the on-air layout, and
      the evidence-grade set.
- [ ] `tools/dag.toml` + `xtask dag-check` exist and pass.
- [ ] The dev shell provides every tool the tickets need.
- [ ] Writers are dispatched sequentially, or in isolated worktrees.
