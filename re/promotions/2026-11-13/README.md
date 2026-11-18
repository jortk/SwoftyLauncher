# Promotions 2026-11-13: hidden modes / engine-setting readers to L2

Agent `vmknolle-hidden`. 55 rows raised from L1 to L2, listed in `applied.tsv`.

## How

1. **Decompile:** Ghidra 12.1 headless (`Decomp.java`, project on build d873a1c8), output in `re/decomp/<addr9>.c`.
2. **Analyse:** four analysis agents, each with a batch of the 79 L1 setting readers, read every decompile. They proposed a name, a proto, xrefs and notes, and judged whether the role was clear from the code alone. Their output went into the rows while they were still L1.
3. **Verify blind:** four separate verifiers (`verify-hm-0` .. `verify-hm-3`) got only addr, name, proto and subsystem. They re-derived each role from the decompile and disassembly, checking callers, vtable slots and the class strings in front of the vtables.
4. **Results:** 62 rows checked. 55 agree and are raised to L2. 7 partial and stay L1, with `CORRECTION(V…)` in their notes (wrong arity, or a class name the vtable string does not support).
5. **Signatures:** 53 of the 55 have a unique `func_start` signature (`sigcheck.make`); `sigcheck.py --only` passes 53/53. Two have no unique pattern and get no binding.

## Owner review

| owner | rows |
|---|---|
| `re-career-teams` (game) | 40 |
| `ui-idf` (ui) | 5 |
| unowned (session 9, core 1) | 10 |

The owned rows were raised with `kb.py --force`, as in the 2026-11-12 promotions. Owners, please review `applied.tsv` and demote with a `verify` row where you disagree.

## Also in this change

13 L1 rows from the first pass were removed before promotion. Their only "reference" to the setting string was a coincidental disp32 inside an unrelated instruction. The scan now checks every hit against an instruction boundary.

## Limits

Static only. No row was observed in a running game.
