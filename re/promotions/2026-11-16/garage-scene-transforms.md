# Updating the transforms of a spawned scene

Proposed for review by the game lane owner, `re-career-teams`. The published
row at `0x1401e7aa0` has level L1 and an empty prototype. V7975 reviewed
that earlier claim. The new L2 prototype has independent static agreement
V10726 on the two Steam executables. [Independent findings](reviewer/garage-scene-batch.md)

```c
void Game_Scene_UpdateInstanceTransformsBatch(void *scene, uint8_t actor_update_flag);
```

## Data flow

`scene` is a live, instantiated scene. Its existing root matrix and scene
resource supply the inputs. Scene spawn writes the four world-matrix rows
immediately before the call at `0x1401a5993`. Scene merge supplies its
scene instance and mode 1 at `0x1401a6c75`.
The routine reads the resource and its object counts before checking the
individual instance pointers. Callers must supply an initialized scene.

The first two passes compose each present object's local transform with the
scene root and store the result in that object. The first pass also updates a
second transform derived from the new world transform. The third pass finds
each actor's instance-specific definition, using the resource definition when
that override is empty. It composes the local and scene matrices and sends
the transposed affine 3-by-4 result to the actor's transform method.

The second argument is an opaque byte flag. At entry the compiler saves EDX
in EBX. At the single actor dispatch it copies EBX into R8D. Native callers
that select mode 1 set only DL, leaving the upper bits unspecified. All matrix
operations take their operands from the scene and object definitions. Among
73 direct references in each executable, 69 supply 1 and four supply 0. The
two modes appear within the same caller at `0x14020add6` and `0x14020c5ec`.
The effect of the flag within each actor implementation remains unverified.

The last pass composes each present child scene's local transform with the
parent world transform. It writes the four new rows into the child and invokes
this same routine with mode 1, including when the parent was given mode 0.
The function has one return point, with no defined integer, pointer or
floating-point result across paths. The spawn and camera callers immediately
reuse the return register. The observable contract is an in-place update.

## Build comparison

The canonical-body decompile with angr 9.2.213 recovered 25 basic blocks.
Its pseudocode shows the four passes and recursive child update. Its
inferred signature and some SIMD store widths conflict with the machine
instructions. The native caller register assignments, actual vector stores
and callee exit paths provide the ABI and transform evidence above.

| Steam build | Executable SHA-256 | Code section | First garage call |
| --- | --- | --- | --- |
| 25251167 (KB) | `d873a1c8d2044cef2fd76c99c9d5a97649c5b815d055540995105f46619865ae` | `.xtls` | `0x14038be90` |
| 26566308 (v1.27) | `46009bd31f49d93a92d73cbd0b66131d2b37964a92da7293168d3c3a872d9389` | `.tls$` | `0x14038bed0` |

The published signature matches once per executable. Both Windows exception
tables delimit the same 1,802 bytes at `[0x1401e7aa0, 0x1401e81aa)`. The
entire bodies compare equal and have SHA-256
`5245d5ba86e6d6d320544d6a19ea6d27fd4a4c171b40c4428fd655154f739f97`.
The garage spawn path uses mode 1 for each of five scene updates in both builds.
These observations establish the static call contract. They leave the
rendered car and bone poses, the render-thread timing, and EA builds untested.

To repeat the body read, set `EXE` to each Steam executable and run
`python re/tools/disx.py 1401e7aa0 70a`. To locate both sets of native
callers, use `re/tools/sigverify.py --ref <base-exe> --port <v1.27-exe>
--build steam-26566308 --refs 1e7aa0` with `--on ref --section .xtls`
and then `--on port --section '.tls$'`. The output gives the displacement
address. Decode the instruction that contains it to distinguish direct calls
from tail jumps. The two sets each contain 71 calls and two tail jumps.
