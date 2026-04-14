# Hypothesis

elmq is a public experiment. This document states what it's trying to prove, how
it's trying to prove it, and what would convince us it doesn't work. It is not a
roadmap or a manifesto — it's a claim and a plan for testing that claim.

## The claim

> **Giving a coding agent a structured, semantic CLI for editing Elm reduces the
> cost of Elm coding tasks by a material margin while preserving correctness.**

"Cost" means USD billed to the model provider, which is a function of tokens
consumed *and* turn count (because cache hits depend on stable turn boundaries).
"Correctness" means the resulting code compiles and solves the task. The target
we'd call a win: **~30% or greater cost reduction on Elm-heavy work**, with no
regression in task success rate.

The null hypothesis — the result that would tell us to stop — is any of:

- no meaningful cost reduction (or a cost *increase*),
- a measurable drop in success rate,
- instability: wins that don't hold up across runs or scenarios.

## Why Elm

The hypothesis is *specifically* about Elm, not "structured edits for any
language." Elm's properties make a class of edits deterministic that aren't
deterministic elsewhere:

- no variable shadowing,
- pure functions (moving a function between modules is a zero-risk mechanical
  operation — imports and references can be rewritten without reasoning about
  side effects),
- a restricted ML core small enough that a tree-sitter grammar plus a handful
  of CST-walkers can cover the language surface,
- a strict compiler that will immediately reject any edit that produces
  nonsense, which makes "did the edit land correctly" cheap to check.

We do not claim these results generalize to TypeScript, Rust, or any language
with side effects, ambient mutation, or a looser type system. That's a separate
hypothesis for someone else to test.

## Why a CLI (not MCP, not LSP)

An earlier version of elmq was an MCP server. It was blocked by an external
stdio bug in the host we couldn't fix, so we pivoted to a CLI. The pivot turned
out to have real advantages worth naming:

- agents already know how to invoke shell tools; no protocol overhead,
- composable with `bash`, `rg`, and the rest of the unix toolkit,
- `--help` and `elmq guide` are readable by the same agent that runs the tool,
- cache-friendly: stdout is deterministic text, trivially reproducible across
  turns.

MCP is not ruled out for the future — a `--mcp` translation layer is plausible
— but the claim under test is about the *affordances* (structured edits plus
an integration guide), not about the transport.

LSP is a deliberate non-choice. LSP is designed for interactive editors with a
human in the loop; its incremental-document model and request/response shapes
don't match how an agent actually works. That's a hypothesis, not a proof, and
we'd be interested in evidence against it.

## Mechanism — how the savings are supposed to happen

If the hypothesis holds, the savings come from some combination of:

1. **Fewer tokens per edit.** Sweeping project-wide operations like `move-decl`,
   `rename decl`, `mv`, and `rm variant` do in one call what would otherwise be
   many `Grep` / `Read` / `Edit` round-trips with large file contents in
   context.
2. **Fewer tokens during discovery.** `list`, `get decl`, and
   `grep --definitions --source` let an agent pull only the declarations it
   cares about. Elm files are typically large; reading a whole module to change
   one function is a tax the tool can remove.
3. **Structural guarantees.** `validated_write` refuses to land an edit that
   would produce an unparseable file. This prevents a whole class of
   "agent corrupted the module, now spends 3 turns recovering" failures.
4. **Correctness-first edits.** Commands like `add variant --fill` and
   `move-decl` encode Elm-specific knowledge (case branches, import graphs)
   that an agent would otherwise have to re-derive per task.

The current benchmark can't fully separate these mechanisms — they're
confounded by design. Isolating them is future work.

## Experimental design

The benchmark harness lives in `benchmarks/`. It runs agents through scripted
Elm coding tasks and records cost, turn count, and task outcome. The current
arms:

- **control** — Claude Code with no elmq, no tool, no guidance. Naive Read /
  Edit / Grep on Elm source.
- **treatment** — Claude Code with `elmq` on `PATH` and `elmq guide` injected
  as `CLAUDE.md` in the workdir, so the guidance propagates to spawned
  subagents.

**Planned third arm: `control-guided`.** A minimal Elm-editing playbook
(without the CLI) injected as `CLAUDE.md` — something like *"Elm files are
large, use Grep to locate declarations by name before reading, use
`offset`/`limit` on Read, etc."* This isolates "how much of the win is the
tool vs. how much is any guidance at all." Not all of elmq's guide ports to a
tool-free arm — `move-decl` with import rewriting has no naive equivalent —
but enough of it does that the delta between `control-guided` and `treatment`
is a more honest measure of elmq itself. See *Threats to validity* below.

### Metrics

- **Headline:** cost (USD) and turns, per task, per arm.
- **Constraint:** correctness. Today this is binary (`elm make` passes).
  Planned: a subreviewer agent rating the final diff on a 1–5 rubric, because
  compiling is not the same as solving the task.
- **Variance:** 5 runs per scenario per arm. Model version is pinned; Claude
  Code doesn't expose temperature, so that's fixed by the harness.

### Tasks

Scenarios are intentionally biased toward operations elmq is built for
(project-wide renames, variant propagation, moving declarations). This is a
known weakness — see *Threats to validity*. There are scenarios elmq loses on,
which we keep in the set because they're useful for checking our own
assumptions.

## Threats to validity

Named so a skeptical reader can weigh them without having to reverse-engineer
them from the results.

- **Task selection bias.** We wrote the tool and the benchmark. It would be
  easy (and tempting) to pick scenarios that flatter elmq. The current suite
  is small and skewed; expanding it with more varied, larger, and
  independently-motivated scenarios is ongoing work.
- **The guide confound.** The treatment arm gets both the tool *and* a
  polished Elm-editing playbook. Some fraction of any measured win may be
  attributable to "any guidance beats no guidance." The planned
  `control-guided` arm exists to address this.
- **File-size dependence.** Part of the hypothesis is that elmq pays off
  *because* real Elm modules are large. If the benchmark fixtures are small,
  the effect will be under-measured. Larger and more realistic fixtures are
  future work.
- **Correctness rubric.** `elm make` passing is a weak proxy for "the task
  is solved." A subreviewer agent helps but is itself noisy. Human review
  doesn't scale to the run counts we need.
- **LLM variance.** Five runs per cell may not be enough to separate signal
  from noise on small effect sizes. If results look marginal, we'll raise N
  before declaring anything.

## What this experiment is NOT claiming

- Not a claim that elmq makes *humans* more productive. It might; that's not
  what's under test.
- Not a claim that structured editing tools help in any language other than
  Elm. The Elm-specific properties (purity, no shadowing, strict compiler)
  are load-bearing.
- Not a claim that CLI is better than MCP in principle. It's better *for this
  experiment, right now*, for the reasons above.
- Not a claim that LSP is the wrong shape for agents — that's a working
  intuition, not a result.

## Status

This is an open experiment. Results aren't in yet at a confidence level we'd
publish. The benchmark suite needs broader and larger scenarios, a
`control-guided` arm, and a better correctness rubric before we'd call any
number definitive. If you want to contribute, the benchmark harness
(`benchmarks/`) is the highest-leverage place to help.
