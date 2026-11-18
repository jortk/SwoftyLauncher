# GfxMaterialRes (.material): F1 25 and F1 23 layouts, F1 23 to F1 25 conversion

Lane: `assets`. Tool: `tools/gfx_material.py` (`parse25`/`write25`, `parse23`/`write23`, `parse_sampler`, `convert23to25`) with the per-shader catalog `tools/gfx_material_layouts.json`.

Evidence: file-format analysis only; no exe addresses. Every claim below holds on these corpora, checked by re-encoding each material from the parsed model and comparing bytes:

| corpus | materials (unique blobs) | byte-identical round trip |
|---|---|---|
| F1 25 `2025_asset_groups/character_package` (drivers, team_livery, kitpart_assets, `common_*.erp`; 1,075 ERPs) | 9,812 (2,926) | 9,812 |
| F1 25 `2025_asset_groups`, all 7,772 ERPs | 183,314 (38,226) | 183,314 |
| F1 23 `2023_asset_groups/character_package` (1,169 ERPs) | 10,616 (3,250) | 10,616 |
| F1 23 `2023_asset_groups`, all 5,535 ERPs | 82,790 (23,158) | 82,790 |

No F1 23 blob parses as F1 25 or the reverse, so `parse25` and `parse23` also tell the two formats apart. 75 F1 23 UI materials (`core_assets_ui.erp`, `main_front_end.erp`) hold a second zstd frame inside the frag; decompress it first. The resource is one frag. All integers are little-endian u32.

## Resource version

The ERP index entry of a material holds a u32 version right after the 16-byte type name (`unk` in `tools/erpw.py`). It is 2 on every stock F1 25 material and 1 on F1 23 materials. F1 25 parses both: `Asset_MaterialFactory_Parse` (0x143c0dc80, L1) reads the F1 25 layout when the version is 2 and the F1 23 layout for any other version. The F1 23 path copies each record's `size` from the value blob without a bounds check (memcpy call at 0x143c0e531). An F1 25 material labelled version 1 hands it record 8's `type` as a size: in More Driver Models' `head.material` (51 records) that is 0x40003 (float4), and the copy runs 256 KB past a 6 KB material, faulting at F1_25.exe+0x53784bb when the material sits at the end of committed memory. `tools/gfx_material.py convert-erp` and the Classic Seasons `tools/heads.py` write version 2 on F1 25 materials.

## F1 25 layout

```
0x00  u32 nparams
0x04  u32 ncbuffers          1 ("PerMaterial"), or 0 for a material with no parameters
0x08  u32 value_size         bytes of the value blob
0x0c  u32 off_shader         offsets 0x0c..0x1c are relative to base = 0x20 + 36 * nparams
0x10  u32 off_names
0x14  u32 off_textures       string list 1: texture records' paths
0x18  u32 off_heap_links     string list 2: heap-index records' texture paths
0x1c  u32 off_cbuffer_names
0x20  record[nparams], 9 x u32 each:
        +0x00 name        offset into names
        +0x04 kind        1 constant, 2 texture
        +0x08 type        constant: low16 3 float / 6 uint, high16 vector width (0 scalar, 2, 3, 4)
                          texture: 2 Texture2D (.tif/.dds/.video/...), 6 Texture2DArray (.texarray)
        +0x0c class       1 value, 3 bindless heap index (only with type 6)
        +0x10 size        constant: value bytes; texture: strlen(path) + 1
        +0x14 cbuffer     constant: 0 (PerMaterial); texture: 0xffffffff
        +0x18 gpu_offset  constant: byte offset inside PerMaterial as the shader reads it; texture: 0
        +0x1c at          constant: offset into the value blob; texture: offset into list 1
        +0x20 link        heap index: offset into list 2 (its texture); otherwise 0
base  value blob          constants back to back in record order (no padding)
      cbuffer table       ncbuffers x {u32 name (into cbuffer names), u32 size}; size == value_size
      shader path         C string, eaid://shader_package/<group>/<name>.nefx2
      names               C strings in record order
      list 1, list 2      C strings, deduplicated: a path seen before reuses its offset
      cbuffer names       "PerMaterial"
```

- Record order: constants first, then textures. Each group is sorted by name, case-insensitively, with `_` ranked below every digit and letter (`sort_key`): `me_SHADOW_*` falls between `me_normalBlend` and `me_skinmask`, and `weight_1_` comes before `weight_10_`. This holds for every material in both games. The value blob follows record order, so `at` grows by `size`; `gpu_offset` follows the shader's own packing (4-byte aligned, a float4 may sit at 8), and the constants' `gpu_offset + size` extents tile `[0, value_size)` exactly.
- Every texture slot `me_X` has a constant `me_XHeapIndex` (type 6, class 3, 4 bytes) whose `link` names the same path. The stored value is 0 in every character material; 699 speedtree and object_opaque heap indices store 1. The engine writes the SRV's descriptor-heap index there at load time.
- The parameter set is a function of the shader: across all 38,226 F1 25 materials, 278 of 285 shaders have exactly one layout (same names, types, classes, sizes, gpu offsets, PerMaterial size). The 7 exceptions are vehicle shaders where a few stale `common_fom.erp` materials lack the two trailing `me_gsaa*` constants.
- There are no sampler records and no GfxSampleState references. Samplers are static in the shader (the char_cloth_skinned nefx2 names `samplerClamp` and `g_shadowSampler`). GfxSampleState survives only for `.video.sampler` resources next to video textures (132 in the whole game).

## F1 23 layout

```
0x00  u32 nparams
0x04  u32 value_size
0x08  u32 off_names          relative to base = 0x10 + 16 * nparams
0x0c  u32 off_strings
0x10  record[nparams], 4 x u32 each:
        +0x00 name    offset into names (deduplicated)
        +0x04 kind    1 constant, 2 texture, 3 sampler
        +0x08 size    constant: value bytes; texture/sampler: strlen(path) + 1
        +0x0c at      constant: offset into the value blob; texture/sampler: offset into strings
base  value blob      constants in record order
      shader path     at base + value_size
      names
      strings         texture and sampler paths in one deduplicated pool, to EOF
```

- Constants carry no type, class or GPU offset. Order: constants sorted by `sort_key`, then textures sorted by `sort_key`, each texture `me_XTex` followed by its sampler `me_X`. The sampler's path is the texture path plus `.sampler`. This holds in all 23,158 materials.
- `GfxSampleState` (`<texture>.tif.sampler`, 32 bytes): `u32 filter, u32 address_u, u32 address_v, u32 address_w, f32 lod_bias, u32 max_anisotropy, u32 unk6, u32 unk7`. The field names are guesses from the values. Character samplers come in six variants: filter 1/6/7/8, address 1/1/1, bias 0.0, anisotropy 4 or 1, then 0, 1. The F1 25 video sampler is `7, 3,3,3, 0.0, 1, 0, 8`.

## F1 23 to F1 25 (`convert23to25`)

1. Look up the F1 23 shader path in the catalog. It holds the F1 25 layout per shader (record order, type, class, size, cbuffer, gpu_offset, PerMaterial size) and the most common F1 25 value of each parameter. A shader with no F1 25 material raises `KeyError`. 23,014 of the 23,158 unique F1 23 materials have a catalogued shader. The other 144 use 30 shaders, mostly track/vehicle (`object_opaque_flipbook`, `vehicle_generic_alpha`, ...). Every F1 23 character material has one.
2. Emit F1 25 records in catalog order:
   - constant: the F1 23 value with the same name and size;
   - `me_XHeapIndex`: value 0, linked to the F1 23 path of `me_X`;
   - texture `me_X`: the F1 23 path, typed 2 or 6 from the catalog.
   A name that F1 23 lacks takes the catalog default and is listed in `notes['defaulted']`.
3. F1 23 constants and textures that are not in the F1 25 layout are dropped (`notes['dropped']`). Sampler records are dropped, and the referenced GfxSampleState states are returned in `notes['samplers']` for inspection. F1 25 has nowhere to store them.
4. PerMaterial size comes from the catalog. The value blob, string pools and header are rebuilt by `write25`.

Validation:
- All 10,616 F1 23 character materials convert. Each output parses with `parse25`, and its layout equals the catalog layout of its shader.
- Only `skin_detail_delta_shift` (78 head/skin materials) defaults and drops anything. It defaults 11 constants new in F1 25 (`me_SSS*`, `me_*SpecLobe*`, `me_hair*`) and drops 9 F1 23 ones (`me_DualAnisoSpec*`, `me_translucency*`). Suit, glove, boot, helmet, visor and logo shaders map one to one, with HeapIndex constants as the only additions.
- 490 material names exist in both games' character packages. 475 convert byte-identically to the shipped F1 25 material. The other 15 differ only in retuned values or textures (e.g. `teamname_charname/livery_v2.material` `me_detailNormalScale`, `fabric_base_04_n` vs `fabric_base_02_n`).
- 364 `2023-<team>_<driver>` materials have a `2024-` F1 25 counterpart. All 364 use the same shader and get an identical layout. 103 are byte-identical once `2023-` is replaced by `2024-` in the paths, including `2023-mercedes_lewis_hamilton/materials/boots.material` (1,505 bytes in F1 23, 1,881 after conversion, the same as 2024's).

CLI: `python3 tools/gfx_material.py dump <erp> <name>`, `convert <f1_23.erp> <dir>`, `convert-erp <f1_23.erp> <out.erp> [--drop-samplers]`, `catalog <F1 25>/2025_asset_groups` (regenerates the catalog).

## Constant values

Stock values per vehicle and track shader, and the constants checked in a race: [`gfx_material_constants.md`](gfx_material_constants.md).

## Open
- No runtime test yet: a converted 2023 suit and helmet in F1 25.
- The exe side beyond the parser (HeapIndex patching, static sampler table) is not catalogued.

## Rust

`packages/formats/src/material.rs` has `Material::parse`/`to_bytes` (the same model as `parse25`/`write25`, canonical or refused), `link_texture(slot, path)` for the `me_X` + `me_XHeapIndex` pair and `set_floats`. All 748 materials of `flow_race.erp` re-encode to their own bytes. Texture, binding, clip and preview writers: `docs/reference/ui/art-resources.md`.
