# diagprint Git provenance v1

Schema:

```text
diagprint.forensics.git-provenance/v1
```

Git provenance v1 binds one immutable diagnostic-history run to exact Git
object identities without changing the diagnostic-history schema.

The headline consumer is:

```text
diagprint blame
```

Despite the name, this is not line-oriented `git blame` and it is not causal
attribution.

It is a forensic repository-context view.

## Record location

For history directory:

```text
.diagprint/history
```

run `000012` uses:

```text
.diagprint/history/git-provenance/run-000012.json
```

The sidecar is separate from history-v2.

History run files, chain computation, and `head.json` semantics remain
unchanged.

## Exact history binding

Every provenance record stores:

- history run index;
- exact history run digest;
- exact report digest;
- binding strength;
- Git commit object id;
- Git tree object id;
- ordered Git parent commit ids;
- provenance record digest.

The record digest is SHA-256 over deterministic compact JSON containing every
field except the record digest itself.

## Git object formats

Git provenance accepts full hexadecimal Git object ids using either:

- 40 hexadecimal characters;
- 64 hexadecimal characters.

This permits both SHA-1 and SHA-256 Git object formats.

## Binding strengths

### `captured_clean`

Produced by:

```text
diagprint scan ... --history <DIR> --git-provenance
```

diagprint requires the Git worktree to be clean before the scan.

After the scan completes, but before history is appended, diagprint verifies
again that:

- the worktree remains clean;
- HEAD is unchanged;
- the resolved Git tree is unchanged;
- the parent identities are unchanged.

Only then is the diagnostic-history run appended and its Git provenance record
written.

`captured_clean` therefore means the committed Git tree represented the clean
worktree surrounding that scan.

Generated history/capsule output is written after this verification.

Projects using `.diagprint/` inside the repository should normally add:

```gitignore
.diagprint/
```

or store diagnostic history outside the repository.

### `user_asserted`

Produced by:

```text
diagprint history git-bind <HISTORY> <RUN> <COMMIT>
```

This explicitly associates an existing immutable history run with a resolved
Git commit after the fact.

It is useful for historical backfill but is intentionally weaker evidence than
`captured_clean`.

The distinction is retained permanently in the record.

## Append-only behavior

Only one Git provenance record may exist for one history run.

Persisting the exact same valid record is idempotent.

Attempting to replace the record with a different binding, commit, tree, or
parent set is rejected.

## Privacy boundary

Persisted Git provenance contains no:

- diagnostic message;
- source path;
- source text;
- arbitrary diagnostic attributes;
- help;
- notes;
- cause chains;
- remediation payloads;
- author name;
- author email;
- remote repository URL;
- absolute repository path;
- branch name.

Commit author, timestamp, subject, and changed paths are read from the local Git
object database only when an investigation command requests them.

## Capsule anchoring

When all three are supplied:

```text
--history
--git-provenance
--capsule
```

capsule project provenance records:

- exact history run digest;
- Git provenance schema;
- Git provenance binding strength;
- Git provenance record digest;
- Git commit;
- Git tree.

The capsule manifest hashes the exact project provenance payload, providing an
additional independent artifact anchor for the Git/history association.

## `diagprint blame`

Usage:

```text
diagprint blame <HISTORY> <FINGERPRINT>
diagprint blame <HISTORY> <FINGERPRINT> --run <N>
diagprint blame <HISTORY> <FINGERPRINT> --repo <PATH>
```

Before displaying repository context, blame verifies:

1. diagnostic-history integrity;
2. fingerprint resolution;
3. Git provenance record digest;
4. exact history run digest association;
5. exact report digest association;
6. Git commit resolution;
7. Git tree identity;
8. Git parent identities.

If `--run` is omitted, diagprint prefers the newest provenance-bound run with a
meaningful forensic transition:

- first seen;
- changed;
- severity increased;
- resolved;
- reappeared.

If none of those has provenance, it falls back to the newest bound timeline
run.

## Repository diff

For an ordinary commit, `diagprint blame` compares the selected commit against
its first parent.

For a root commit, it displays the root diff.

For merge commits, this first-parent comparison is repository context rather
than a complete semantic explanation of the merge.

The output may include:

- files changed;
- insertions;
- deletions;
- binary-file count;
- changed path/status lines.

These paths are not claimed to be diagnostic source locations.

## Evidence versus causation

Git provenance can establish:

> This exact Git commit/tree was associated with this exact diagnostic-history
> run under the recorded binding strength.

It cannot by itself establish:

> This commit caused this diagnostic.

Therefore `diagprint blame` always reports:

```text
causation: NOT ESTABLISHED
```

Changed files are repository context only.

A later causal-graph layer may combine explicit diagnostic relationships,
source snapshots, remediation receipts, replay evidence, and provenance, but
Git temporal association alone is never promoted to causation.

## Compatibility

The meaning of:

```text
diagprint.forensics.git-provenance/v1
```

is immutable.

Backward-incompatible semantic changes require a new schema version.
