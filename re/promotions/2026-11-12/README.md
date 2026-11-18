# My Team promotions (2026-11-12)

Raises 723 My Team rows from #995 to L2. Every one has a verified `func_start` signature (sigcheck on Steam d873a1c8)
and an `agree` row from a blind verifier who is not the raising agent. No other row changes.

Two rounds:
1. 40 rows that already had signatures and agree rows from #995 (verify-mt-a..d).
2. The remaining 1242 rows: `sigcheck.make` found a unique func_start pattern for 915, eight blind verifiers
   (verify-mt3-0..7) judged those 915 (685 agree, 189 partial, 41 disagree). Agree + unique signature + xref
   evidence = 683 rows raised (one agree row lost its signature because the pattern also matched 0x20 earlier
   and stays L1; one had no xref token). Partial and disagree rows stay L1; their verify rows carry the
   corrections for the lane owner. 873 new signatures are added (agree + partial rows).

| owner | lane | rows |
|---|---|---|
| career-re | career | 435 |
| ui-idf | ui | 129 |
| ui-re | vm | 79 |
| re-career-teams | game | 45 |
| - (unowned) | core | 29 |
| - (unowned) | gamedb | 6 |

`applied.tsv`: owner, addr, name, subsystem, signature id, verifier, agree row (V-number), decision.

The 688 rows in owned lanes were raised with `kb.py --force` (agent `vmknolle-myteam`), so they need the lane
owner's review: drop single rows or reject the PR if an owner disagrees. Demote with
`py -3 re/tools/kb.py --agent <owner> set functions <addr> level=L1`.

Bindings: `bindgen.py` exports 3414 bindings instead of 2691 on main (+723, all experimental). Generated
headers only gain entries (new `ui.hpp`); the 4 bindings that main's sigcheck already drops were kept as they are.

All findings are static (decompile + disassembly), nothing is runtime-tested. Several agree verdicts on large UI
builders rest on strings and callees rather than a full read of the body.
