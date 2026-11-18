# Independent verification of the garage scene transform update

Verifier `garage-scene-review`, 2026-11-16. Verdict `agree` for the L2 function contract at `0x1401e7aa0`. The canonical and installed Steam executables were read independently. The original promotion write-up was withheld until the verifier had derived the call sequence and data flow.

| Image (SHA-256 prefix) | Batch start | Garage owner start | Garage call sites in slot order |
| --- | --- | --- | --- |
| Canonical `d873a1c8d2044cef` | `0x1401e7aa0` | `0x140388cf0` | `0x14038be90`, `0x14038c07e`, `0x14038c1d1`, `0x14038c324`, `0x14038c477` |
| Installed `46009bd31f49d93a` | `0x1401e7aa0` | `0x140388d30` | `0x14038bed0`, `0x14038c0be`, `0x14038c211`, `0x14038c364`, `0x14038c4b7` |

## Data flow

The native calling contract is `void Game_Scene_UpdateInstanceTransformsBatch(void *scene, uint8_t actor_update_flag)`. The first argument is the spawned instance. Its existing 4-by-4 matrix occupies four aligned rows at `+0x20,+0x30,+0x40,+0x50`. Counts in its descriptor at `+0xa0` govern four loops which skip null elements.

1. Objects in the instance's `+0x68` list receive composed matrices at `+0x4e0` and `+0x520`. The second matrix includes the object's own transform at `+0xd0`. The `+0x70` list receives a composed matrix at `+0x1a0`.
2. For each populated entry in the `+0x78` list, the code selects the entry's descriptor at `+0x538` or the indexed fallback. It composes the transform, rearranges it into three 16-byte rows on the stack and calls vtable slot `+0x70` at `0x1401e7fdb`. RCX is the entry, RDX points to those rows and R8D receives the saved second argument. The incoming flag has no data flow into floating-point arithmetic. The interpretation of its two values inside the virtual implementation is open.
3. The `+0x98` list contains child scenes. Four stores beginning at `0x1401e8167` replace each child's root matrix. The recursive call at `0x1401e817d` sets DL to one regardless of the original flag.
4. All 71 direct near calls in the main code section of each build were checked at decoded instruction boundaries. In each build 67 set DL to one and four clear EDX. Upper argument bits are consequently unspecified at the DL-only callers. In particular, the five garage paths test slots `+0xc8,+0xd0,+0xd8,+0xe0,+0xf0` for null, compose the descriptor's matrix at `+0x140` with the garage placement matrix at `+0x1c0`, store the four rows in the instance, and pass that same instance in RCX with DL set to one. None of the five uses a return value. The common epilogue establishes no return value.

## Reproduction

Decode the function from the PE exception directory boundaries `[0x1401e7aa0,0x1401e81aa)`. Both images give 1,802 bytes with SHA-256 `5245d5ba86e6d6d320544d6a19ea6d27fd4a4c171b40c4428fd655154f739f97`. The registered signature has one match per main code section (`.xtls` and `.tls$`), resolving to this function start. The stock `re/tools/xref.py batch=1e70a0` with `EXE` set to the canonical image reports 73 relative references, including operand offsets. The direct-call counts above use decoded E8 instructions only.

GNU objdump can show the batch with `-d -Mintel --start-address=0x1401e7aa0 --stop-address=0x1401e81aa`. For the garage, decode each owner from the start given in the table through its exclusive end (`0x14038c9b8` and `0x14038c9f8`). Inspect the five listed sites. Both register transfers into the actor call and the operand widths are visible directly.

The verifier also re-decompiled both bodies with angr 9.2.213 using the Win64 calling convention, yielding 25 CFG blocks per image. Its C printer recovers the two inputs and four loops but guesses a return type from residual RAX and reports width errors for SIMD stores. Native calls, stores and the sole epilogue settle those details. The independent canonical decompilation is retained locally at `re/decomp/1401e7aa0-garage-scene-review.c`; the repository's Ghidra project was unavailable on this workstation. Evidence covers the static contract on these two Steam images.
