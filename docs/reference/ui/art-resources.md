---
title: UI art resources
description: The resources behind a new piece of UI art in F1 25 — texture, material, material binding, animation clip, string table and editor preview — and the swofty-formats writers that build them.
---

# UI art resources

A sprite on a dial is five resources, and an animated one has a sixth. `swofty-formats` (`packages/formats`) reads and writes each of them; the HUD composer calls these writers, and they are the whole API an addon-side build tool needs.

## A sprite in full

```rust
use swofty_formats::{emb, material::Material, surface::{Surface, TextureResources}};

let tex = "eaid://ui_package/art/osd/textures/mine/dot";
let surface = Surface::from_rgba(&rgba, 64, 64, true, false)?;         // BC7 sRGB, one level
let texture = TextureResources::new(tex, &surface);
// texture.surface_name  = "<tex>.tif.image"   GfxSurfaceRes, fragments [header, pixels]
// texture.view_name     = "<tex>.tif"         GfxSRVResource, one fragment

let mut mat = Material::parse(&stock_material_bytes)?;                  // a stock dial quad material
mat.link_texture("me_diffuseTex", &texture.view_name)?;
mat.set_floats("me_colour", &[1.0, 1.0, 1.0, 1.0])?;
let material = mat.to_bytes();                                          // GfxMaterialRes

let binding = emb::write(&[("central_bg_fill".into(), material_path.into())]); // GfxMatBindRes
```

The four resources are added to the archive with the layout of a stock resource of the same type (`erp::Resource::like`). The scene's quad element then points at the binding through its `XrefMaterialBinding` param and the mesh slot name selects the material.

## Texture: `GfxSurfaceRes` and `GfxSRVResource`

```text
<name>.tif.image   GfxSurfaceRes, ERP type argument 9
  fragment 0   40 bytes: u32 2, 0x108, format, width, height, 1, levels, 1, 0, 0
  fragment 1   every mip level, largest first, each a whole number of 4x4 blocks
  fragment 2   optional 288 byte stream descriptor (the .tif.2025.mipmaps side file's path)
<name>.tif         GfxSRVResource, ERP type argument 2
  fragment 0   24 bytes: u32 5, format, 0, levels, 0, 0, then the surface's name and a NUL
```

Formats are the engine's numbers: 52/54 BC1 (linear/sRGB), 55/57 BC3, 65 BC5, 69 BC7, 70 BC7 sRGB. `Surface::from_rgba` pads sizes that are not multiples of 4 by repeating the edge texels and builds mips by a box filter with `(sum + n / 2) / n` rounding, the same as the native garage build. UI sprites are sRGB, one level, no descriptor. On `flow_race.erp`, the writer reproduces the stock bytes of 474 BC7 surface headers and 523 SRV resources.

## BC7 encoder

`bc7_enc::encode_rgba` writes mode 6 blocks with alpha weighting and a bleed of the opaque mean colour into transparent texels, so sprite edges do not fringe under bilinear sampling. `encode_logo` additionally switches a block to mode 5 (separate alpha) where that has less error. Both are bit-identical ports of `native/src/core/apply/content/bc7.hpp`: `packages/formats/tests/fixtures/bc7/gen.sh` compiles the C++ encoder and rewrites the golden files, and the Rust tests compare block for block, plus the native test fixtures the C++ and Python agree on.

## Material texture links

A texture slot `me_X` is two parameters: the texture record `me_X` (the path) and the heap index constant `me_XHeapIndex` whose link names the same path. `Material::link_texture("me_diffuseTex", path)` sets both. `Material::parse` refuses a material that does not re-encode to its own bytes, and all 748 materials of `flow_race.erp` do. The layout is in `re/db/subsystems/gfx_material.md`.

## Material binding: `GfxMatBindRes` (`.emb`)

```text
u32 n_slots, u32 n_unique_materials, u32 material_block_offset (from 0x0c)
n_slots x { u32 slot_name_offset, u32 material_offset }
slot names (NUL terminated), then material paths (NUL terminated), each once
```

`emb::parse` gives (slot, material) pairs and `emb::write` of them gives the stock bytes back.

## Animation clip and string table

A clip is `<scene>.idf/<element>?context=animation`, type `AnimClip`, two fragments: `temp`, an `ANIM` `!pkg` with one packet per channel, and `syst`, the key times and key values the packets point into. The scene's `ACStringTable` (`<scene>.idf?context=animstringtable&file=0`, an `ANST` `!pkg`) lists the element names; a clip's `name_index` is its element's index there.

```rust
use swofty_formats::animclip::{Channel, append_strings, build_clip};

let (table, first) = append_strings(&stock_table, &["zh_slide"])?;      // the clip's name_index
let slide = Channel::position(vec![0.0, 1.5], vec![[-1.0, 0.0, 0.0], [1.0, 0.0, 0.0]]);
let (temp, syst) = build_clip(&stock_clip_temp, "zh_slide", first as i32, &table_name, &[slide])?;
```

A channel is a position (interpolator 5, flags 0x80, three floats a key), a rotation (2, 0x84, a quaternion) or a scale (5, 0x80, three floats). The packet's `maxtime` is the last key time and `mintime` 0 unless the clip starts later. Key times are stored as they come, key values start 16 byte aligned, and `syst` ends where the last key ends. Stock clips also carry tangent keys (interpolator 3 and 4, 5 and 20 floats a key) and two-channel packets; `read_clip` refuses those. Of the stock clips the writer understands, all 66 in `flow_race.erp` rebuild byte for byte. A `UIXA` node in the screen drives a clip from a view-model fraction.

## Editor preview: `.uipre`

The HUD editor draws a dial scene with no template running: the scene's own state plus the setters of `<scene>.uipre`.

```rust
use swofty_formats::uipre::Uipre;

let mut pre = Uipre::parse(&stock_uipre)?;
pre.set_visible("fuel_icon_empty", false)?;     // hide a stock part in the editor
pre.set_text("speedo_number", "135")?;          // the text the editor shows
let bytes = pre.to_bytes()?;
```

A `UIPR` `!pkg` holds `Elements` (each an `Id`, the djb2 of the element's name, and `Setters`, each a `Key` and a typed `Value`) and the scene's `TargetId`. Setter key 4045306521 sets a label's text (`stri`), 839438211 an element's visibility (`bool`).

## Credits

The sprite, clip and preview layouts were first worked out by Project Halo's build tools. Uses work from Project Halo by nyannoying, <https://github.com/nyannoying1337/f1-25-project-halo>.
