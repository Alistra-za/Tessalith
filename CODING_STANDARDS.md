# Coding standards for reviewers

Judgement calls that no tool can check. Fmt, clippy, control bytes and duplicate note headings are checked by machines (`.githooks/pre-commit`, CI). Report a violation of a rule here as a finding, citing the rule.

- **A RED comes from real behaviour.** A failing test proves a fix only if it failed on the old behaviour, or a mutation of the new code makes it fail. A compile error against a function that doesn't exist yet proves nothing about behaviour.
- **Numbering schemes convert at the boundary.** An integer that has several numbering schemes (raw TXST index, 0-based NIF slot, 1-based `slotN`) states which one it holds at its declaration, and is converted once where it enters the code. Check every new read of such a value against its source: `txst_slot.slot` is a raw TX index, and TX02 is not slot 3.
- **Tests go through what gets written.** A test of a name, path or mapping checks the output that reaches disk or the record that reaches the game, not only the input name it started from.
- **A cache key covers everything the builder reads.** For each new input to a cached computation, find the key and confirm the input is in it. A new field read by the builder and missing from the key is a stale-cache bug.
- **A menu command does something in every state that shows it.** Hide or disable a command the current state can't act on. A click that silently changes nothing is a bug.
