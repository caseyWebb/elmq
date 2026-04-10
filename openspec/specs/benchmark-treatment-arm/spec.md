## ADDED Requirements

### Requirement: Treatment arm delivery mechanism

The `treatment` arm SHALL deliver elmq guidance to Claude by writing `benchmarks/elmq-guide.md` as `$work_dir/CLAUDE.md` at arm setup time (after the fixture copy, before the initial `git add -A` / `git commit` so the CLAUDE.md is part of the initial fixture state seen by `claude -p`). It SHALL NOT deliver the elmq guide via `--append-system-prompt-file benchmarks/elmq-guide.md`.

The rationale, empirically established by smoke testing: `--append-system-prompt-file` modifies the parent `claude -p` invocation's system prompt, but when the parent spawns a subagent via the `Task` tool (e.g. the `Explore` subagent), the subagent is a fresh Claude invocation with its own system-prompt construction and does not inherit the parent's appended file. `CLAUDE.md` is project memory loaded from the cwd on every Claude invocation in that directory, including spawned subagents.

The arm-independent `SYSTEM_PROMPT` (the project-describing prompt at `benchmarks/system-prompt.md` that tells Claude it is working on an Elm project) SHALL continue to be delivered via `--append-system-prompt-file "$SYSTEM_PROMPT"` on both arms. Only the elmq-guide-specific `--append-system-prompt-file` is removed, and only from the treatment arm.

The control arm SHALL NOT receive any CLAUDE.md in its workdir. The CLAUDE.md copy SHALL be gated on `$arm == "treatment"`. This preserves the control arm's byte-for-byte-identical invariant relative to its pre-change behavior.

#### Scenario: Treatment arm writes CLAUDE.md before initial git commit

- **WHEN** `benchmarks/run.sh` runs the `treatment` arm
- **THEN** after `cp -r "$FIXTURE_DIR" "$work_dir"` and before `git add -A` / `git commit`, the script SHALL execute `cp "$BENCH_DIR/elmq-guide.md" "$work_dir/CLAUDE.md"`
- **AND** the initial fixture commit SHALL include `CLAUDE.md` at the workdir root so the content is visible to `claude -p` from the first scenario onward

#### Scenario: Treatment arm does not pass the guide via `--append-system-prompt-file`

- **WHEN** `benchmarks/run.sh` constructs the `claude_base` command array for the `treatment` arm
- **THEN** the command array SHALL NOT contain `--append-system-prompt-file "$BENCH_DIR/elmq-guide.md"` or any equivalent reference to `elmq-guide.md` as a system-prompt file
- **AND** the command array SHALL contain exactly one `--append-system-prompt-file` entry: the one for `"$SYSTEM_PROMPT"` (the arm-independent, project-describing prompt), which appears in both arms' invocations

#### Scenario: Control arm workdir has no CLAUDE.md

- **WHEN** `benchmarks/run.sh` runs the `control` arm
- **THEN** `$work_dir/CLAUDE.md` SHALL NOT exist after the fixture is copied and the initial git commit is made

#### Scenario: Subagents inherit the guide under CLAUDE.md delivery

- **WHEN** the treatment arm's main agent spawns a subagent via the `Task` tool (e.g. the `Explore` subagent)
- **THEN** the subagent SHALL use `elmq list` / `elmq get` / `elmq refs` on `.elm` files in preference to `Read` / `Grep`
- **AND** the subagent SHALL NOT make any `Read` tool call targeting a file matching `*.elm`

### Requirement: Elmq guide file

The harness SHALL ship a markdown guide at `benchmarks/elmq-guide.md` (copied to `/bench/elmq-guide.md` in the Docker image) that describes the elmq CLI to Claude. Every subcommand name and flag referenced in the guide SHALL match the actual `src/cli.rs` definitions exactly.

The guide's scope SHALL be limited to describing elmq itself: what it does, what built-in tools it replaces on `.elm` files, how to invoke each subcommand (including a task → command decision table), and factual gotchas about elmq's runtime behavior. The guide SHALL NOT contain agent-behavior meta-rules — it SHALL NOT prescribe how the agent should plan its work, how the agent should relate to other tools like subagents, how the agent should manage general shell hygiene, or how the agent should batch unrelated tool calls. The guide SHALL NOT expose the benchmark's own metrics to the agent — in particular, it SHALL NOT frame its rules in terms of tool-call counts, cache-read costs, or any other measurement the harness is using to evaluate the guide's effect.

The guide SHALL frame `elmq list` and `elmq get` as **targeted** exploration tools — use them on files and declarations the agent needs to understand, not carte blanche on the whole project. The guide SHALL NOT use phrasing like "use [elmq list] every time" or any semantically equivalent instruction to call `elmq list` on every file. File discovery SHALL be directed to `find` / `Glob` first, then `elmq list` / `elmq get` on the files and declarations relevant to the task.

The guide SHALL include a task → command decision table mapping intents to the highest-level elmq subcommand for each intent (`mv`, `rename`, `move-decl`, `variant add`/`rm`, `refs`, `patch`, `set`, `import add`/`remove`, `expose`/`unexpose`). Each invocation in the table SHALL match the actual CLI flag/positional signature.

The decision table entry for `elmq move-decl` SHALL describe both the extract-into-new-module use case and the move-between-existing-modules use case in its intent column. The entry SHALL also note, in its command column, that `move-decl` creates the `<target>` file if it does not already exist.

The guide SHALL include a short worked example, placed after the decision table, demonstrating `elmq move-decl` invoked against a realistic source file producing a non-existent target file. The worked example SHALL be descriptive ("here is what this command does") rather than prescriptive.

The guide's "do not use" rule for search tools SHALL cover both `Grep` the built-in tool AND `grep` / `rg` invoked via `Bash` on `.elm` files.

The guide MAY include a short "Gotcha" section documenting factual runtime behavior the agent could otherwise get wrong — for example, that `elmq variant add` inserts new case branches as `Debug.todo "<VariantName>"`. "Gotcha" items SHALL be descriptions of what elmq commands actually do, not prescriptions about agent workflow.

#### Scenario: Guide file exists in the image

- **WHEN** `./benchmarks/build.sh` builds the `elmq-bench` image
- **THEN** `/bench/elmq-guide.md` SHALL exist in the resulting image, readable by the `bench` user

#### Scenario: Guide CLI references are accurate

- **WHEN** any subcommand name or flag appears in `benchmarks/elmq-guide.md`
- **THEN** the same subcommand and flag SHALL be present in `elmq --help` or the relevant `elmq <subcommand> --help` output

#### Scenario: Guide scope is limited to describing elmq

- **WHEN** a reader scans `benchmarks/elmq-guide.md`
- **THEN** the document SHALL contain only: a statement of what elmq is and what built-in tools it replaces on `.elm` files; a description of the reading subcommands (`list`, `get`, `refs`); a task → command decision table for the editing subcommands; an `elmq set` stdin/heredoc example; and factual gotchas about elmq behavior
- **AND** the document SHALL NOT contain sections titled or framed around "planning," "exploration strategy," "subagent trust," "tool-call budgets," "cost models," "cache efficiency," or equivalent
- **AND** the document SHALL NOT contain a numeric cap on reconnaissance call counts
