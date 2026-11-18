# Third-party notices

## Geode docs theme

`docs/.vitepress/theme/styles/flash.css` is adapted from the stylesheets in `flash-template/` of the Geode docs, <https://github.com/geode-sdk/docs>. Its selectors are changed to fit VitePress and the peach theme is the default.

```
MIT License

Copyright (c) 2022 Geode SDK developers

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## JSONTestSuite

`native/test/json/suite/` holds the `y_`, `n_` and `i_` parsing cases of JSONTestSuite at commit 1ef36fa, <https://github.com/nst/JSONTestSuite>, unchanged. The `swofty::json` unit test runs them.

```
MIT License

Copyright (c) 2016 Nicolas Seriot

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## Geode website

The homepage in `docs/.vitepress/theme/components/Landing.vue` and `docs/.vitepress/theme/styles/landing.css` follows the look of <https://geode-sdk.org/>. It is written from scratch and contains no code or CSS from the Geode website.

## NefsEdit

`packages/formats/src/nefs.rs` reads the NeFS 1.6 archive layout that NefsEdit documents, <https://github.com/victorbush/ego.nefsedit>. It is written from scratch and contains no NefsEdit code.

## Simplified Suite

The built-in Team Simplified addon, `addons/simplified`, reads the `.serps` package format of the Simplified Suite by Team Simplified, <https://github.com/TeamSimplified/simplified-suite>. Its `mod.serps` reader (`src/serps.rs`) and its package root, content root and entry path checks (`src/package.rs`) are ported from `ModManifest.cs`, `ModImport.cs` and `ModStaging.cs` of the Simplified Launcher, with Team Simplified's permission. The texture array layout and slice merge behind the `erp.merge-slices` operation (`packages/formats/src/texarray.rs`, used by `packages/core/src/erpbuild.rs`) are ported from `src/texarray.rs` of the Team Simplified addon for Swofty Launcher, which ports them from `merge_resources.py` of the Simplified Launcher and `LogoBaker.cs` and `LogoSource.cs` of the Simplified Team Editor. The Simplified Suite is licensed under the GNU General Public License version 3, which section 13 lets this AGPL-3.0-or-later program combine with.

Based on Simplified Suite by Team Simplified - https://github.com/TeamSimplified/simplified-suite

```
Simplified Suite
Copyright (C) 2026 Team Simplified

This program is free software: you can redistribute it and/or modify it under
the terms of the GNU General Public License as published by the Free Software
Foundation, version 3 of the License. It is distributed WITHOUT ANY WARRANTY;
see LICENSE for the full terms.

Under section 7(b) of the GNU General Public License version 3, the following
attribution requirement applies to this work and to every work based on it:

> Any program or library that contains, copies, ports or is based on code from
> Simplified Suite must credit it, by preserving this notice in its source and
> by stating, in its credits, About screen or documentation - wherever the
> program shows its own Appropriate Legal Notices:
>
> **Based on Simplified Suite by Team Simplified - https://github.com/TeamSimplified/simplified-suite**
```

## bcdec and bcdec_rs

The native Live image service also uses `native/third_party/bcdec/bcdec.h`, pinned to commit `80859ed3b7afb1c527a2a99d70c61457bea72d0c`, under the bcdec MIT licence reproduced below and in `native/third_party/bcdec/LICENSE.txt`.

Swofty UI's BC7 texture decoder, `tools/swofty-ui/static/bc7.js`, is a JavaScript port of bcdec_rs 0.2.0 from <https://github.com/ScanMountGoat/image_dds>, which is itself a port of bcdec by Sergii Kudlai, <https://github.com/iOrange/bcdec>. bcdec_rs is MIT-licensed. bcdec is available under the MIT licence or the Unlicense; Swofty UI uses it under the MIT licence.

```
MIT License

Copyright (c) 2023 SMG

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

```
MIT License

Copyright (c) 2022 Sergii Kudlai

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## Swofty UI executable

`bin/swofty-ui` and `bin/swofty-ui.exe` in the release archives are built with PyInstaller, <https://pyinstaller.org/>, and bundle:

| Component | Licence |
|---|---|
| The CPython 3.14 runtime and standard library, <https://www.python.org/> | Python Software Foundation License Version 2, <https://docs.python.org/3/license.html> |
| The PyInstaller bootloader | GNU General Public License version 2 or later with the PyInstaller bootloader exception, <https://github.com/pyinstaller/pyinstaller/blob/develop/COPYING.txt> |
| The libraries CPython links, such as zstd, zlib, libffi and OpenSSL | Their own licences, listed in the CPython licence document above |

Python is Copyright (c) 2001 Python Software Foundation; All Rights Reserved.

## DirectX Shader Compiler

Releases include `dxc/`: dxc, dxcompiler and dxil built by `.github/workflows/dxc.yml` from <https://github.com/microsoft/DirectXShaderCompiler> at the tag in `tools/dxc/version`. They compile addons' post-process passes. `dxc/LICENSE.txt` (University of Illinois/NCSA Open Source License, with MIT-licensed parts) and `dxc/ThirdPartyNotices.txt` are copied from that tag.

## Discord logo

`apps/launcher/assets/discord.svg` is the Discord logo from Simple Icons, <https://simpleicons.org/>, released under CC0 1.0. Discord and the Discord logo are trademarks of Discord Inc.

## ADDON SETTINGS icons

`packages/core/src/ui/icons/addon.svg`, drawn into `packages/core/src/ui/icons.bc7`, is the `package` icon from Lucide, <https://lucide.dev/>, as gpui-component ships it:

```
ISC License

Copyright (c) 2026 Lucide Icons and Contributors

Permission to use, copy, modify, and/or distribute this software for any
purpose with or without fee is hereby granted, provided that the above
copyright notice and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
```

`simplified.svg` is Team Simplified's mark, from Simplified for Swofty. `swoftymod.svg` and `track.svg` are the launcher's own `apps/launcher/assets/icon.svg` and `track.svg`.

## Project Halo

Parts of the std lib's HUD and live state are ported from, or follow, Project Halo by nyannoying, <https://github.com/nyannoying1337/f1-25-project-halo>, with its author's permission. Project Halo is under the Project Halo Attribution License 1.0, below.

Uses work from Project Halo by nyannoying: <https://github.com/nyannoying1337/f1-25-project-halo>

Runtime headers, `packages/sdk/include/swofty/`, mirrored in `templates/addon/native/include/swofty/`. They port the generic parts of Halo's HUD plugin: leaf publishing, easing and holds, gap and speed text, the frame loop's cadences and cost meter, the validation log and the timing tower model with its leaf contract.

- `hud_leaves.hpp` from `native/src/leaves.hpp` and `tower_leaves.cpp`
- `hud_format.hpp` and `timing.hpp` from `native/src/tower_model.cpp` and `hud_model.cpp`
- `ease.hpp` from `native/src/hud_model.cpp` and `gmeter.cpp`
- `hud_frame.hpp` from `native/src/halo_hud.cpp`
- `log_csv.hpp` from the validation log of `native/src/diag_log.cpp`

Formats, `packages/formats/src/`. `surface.rs`, `emb.rs`, `animclip.rs`, `uipre.rs` and the texture link part of `material.rs` follow the file layouts and build steps of Halo's `tools/native/resources.py`, `emb.py` and `halo_native.py`. The code is written for the launcher; the layouts, setter keys and clip packet fields are Halo's measurements of the game.

HUD composer, `packages/core/src/hud/`. `scene.rs`, `template.rs`, `editor.rs`, `textures.rs` and `mod.rs` are ported in parts from `tools/native/halo_native.py`, `build_native.py` and `resources.py`: the element templates, the placement maths and text model, the clip keys, the binding and decision rules, the editor preview and the checks against the stock files. Their file headers carry the notice.

Live state, `native/src/core/live/`. `live_corners_data.hpp` is the official corner table of Halo (`native/src/halo_corners.hpp`), moved with the namespace changed. `live_turns.hpp` adapts the track-name matching and turn lookup of `native/src/halo_track.cpp`. The object layouts and read rules in the other files follow the in-process readers of `game_state_f1_25.cpp`, `tower_reader.cpp` and `tower_state.hpp`.

Documentation. These pages carry the credit line: `docs/reference/ops/hud.md`, `docs/reference/ui/art-resources.md`, `docs/reference/std/hud-leaves.md`, `hud-format.md`, `ease.md`, `hud-frame.md`, `timing.md`, `diag-csv.md`, `live.md`, `docs/make/hud-element.md` and `docs/make/halo-migration.md`.

Formats and game facts, such as offsets, file layouts, setter keys and measured constants, are not covered by the licence.

```
Project Halo Attribution License 1.0
SPDX-License-Identifier: LicenseRef-Project-Halo-Attribution-1.0

Copyright (c) 2026 nyannoying

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to use,
copy, modify, merge, publish, distribute, sublicense, and/or sell copies of
the Software, subject to all of the following conditions:

1. The above copyright notice and this license shall be included in all copies
   or substantial portions of the Software.

2. Every publicly released mod, project, or other work incorporating a
   substantial portion of the Software, in source or compiled form, must
   provide the following visible attribution, including the project link:

   "Uses work from Project Halo by nyannoying"
   https://github.com/nyannoying1337/f1-25-project-halo

   The attribution must appear in the work's public description, README,
   documentation, or credits. It must be readily accessible to users and must
   accompany redistributed copies in a readable credits or documentation file.
   If the chosen location supports hyperlinks, the project link must be clickable.
   An equivalent translation is permitted if the author, project name and link
   are preserved. Private use does not require public attribution.

3. Attribution must not imply endorsement by nyannoying. Modified versions must
   be identified as modified and must not be presented as the original release.
   Sublicensing may not remove the attribution and notice obligations applying
   to the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
THE SOFTWARE.

Scope: this license applies to original Project Halo code and documentation in
this release. Third-party files, including the Swofty SDK, retain their own
licenses. The official turn numbers in native/src/halo_corners.hpp come from F1
live-timing circuit data and are not covered by this license. The game resources
the addon's archives are built from belong to their owners. No rights to game
assets or trademarks are granted by this license. This is a custom attribution
license, not the MIT License or an OSI-approved standard license.
```

## Fonts

Swofty UI uses the system's fonts. When the system has none, as in a Windows compatibility layer without fonts installed, it falls back to DejaVu Sans Regular and Bold, which are embedded in the launcher (`apps/launcher/assets/fonts`). The fonts are under the Bitstream Vera licence with DejaVu's public domain changes:

```
Fonts are (c) Bitstream (see below). DejaVu changes are in public domain.
Glyphs imported from Arev fonts are (c) Tavmjong Bah (see below)


Bitstream Vera Fonts Copyright
------------------------------

Copyright (c) 2003 by Bitstream, Inc. All Rights Reserved. Bitstream Vera is
a trademark of Bitstream, Inc.

Permission is hereby granted, free of charge, to any person obtaining a copy
of the fonts accompanying this license ("Fonts") and associated
documentation files (the "Font Software"), to reproduce and distribute the
Font Software, including without limitation the rights to use, copy, merge,
publish, distribute, and/or sell copies of the Font Software, and to permit
persons to whom the Font Software is furnished to do so, subject to the
following conditions:

The above copyright and trademark notices and this permission notice shall
be included in all copies of one or more of the Font Software typefaces.

The Font Software may be modified, altered, or added to, and in particular
the designs of glyphs or characters in the Fonts may be modified and
additional glyphs or characters may be added to the Fonts, only if the fonts
are renamed to names not containing either the words "Bitstream" or the word
"Vera".

This License becomes null and void to the extent applicable to Fonts or Font
Software that has been modified and is distributed under the "Bitstream
Vera" names.

The Font Software may be sold as part of a larger software package but no
copy of one or more of the Font Software typefaces may be sold by itself.

THE FONT SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
OR IMPLIED, INCLUDING BUT NOT LIMITED TO ANY WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT OF COPYRIGHT, PATENT,
TRADEMARK, OR OTHER RIGHT. IN NO EVENT SHALL BITSTREAM OR THE GNOME
FOUNDATION BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, INCLUDING
ANY GENERAL, SPECIAL, INDIRECT, INCIDENTAL, OR CONSEQUENTIAL DAMAGES,
WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF
THE USE OR INABILITY TO USE THE FONT SOFTWARE OR FROM OTHER DEALINGS IN THE
FONT SOFTWARE.

Except as contained in this notice, the names of Gnome, the Gnome
Foundation, and Bitstream Inc., shall not be used in advertising or
otherwise to promote the sale, use or other dealings in this Font Software
without prior written authorization from the Gnome Foundation or Bitstream
Inc., respectively. For further information, contact: fonts at gnome dot
org.

Arev Fonts Copyright
------------------------------

Copyright (c) 2006 by Tavmjong Bah. All Rights Reserved.

Permission is hereby granted, free of charge, to any person obtaining
a copy of the fonts accompanying this license ("Fonts") and
associated documentation files (the "Font Software"), to reproduce
and distribute the modifications to the Bitstream Vera Font Software,
including without limitation the rights to use, copy, merge, publish,
distribute, and/or sell copies of the Font Software, and to permit
persons to whom the Font Software is furnished to do so, subject to
the following conditions:

The above copyright and trademark notices and this permission notice
shall be included in all copies of one or more of the Font Software
typefaces.

The Font Software may be modified, altered, or added to, and in
particular the designs of glyphs or characters in the Fonts may be
modified and additional glyphs or characters may be added to the
Fonts, only if the fonts are renamed to names not containing either
the words "Tavmjong Bah" or the word "Arev".

This License becomes null and void to the extent applicable to Fonts
or Font Software that has been modified and is distributed under the 
"Tavmjong Bah Arev" names.

The Font Software may be sold as part of a larger software package but
no copy of one or more of the Font Software typefaces may be sold by
itself.

THE FONT SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO ANY WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT
OF COPYRIGHT, PATENT, TRADEMARK, OR OTHER RIGHT. IN NO EVENT SHALL
TAVMJONG BAH BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY,
INCLUDING ANY GENERAL, SPECIAL, INDIRECT, INCIDENTAL, OR CONSEQUENTIAL
DAMAGES, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
FROM, OUT OF THE USE OR INABILITY TO USE THE FONT SOFTWARE OR FROM
OTHER DEALINGS IN THE FONT SOFTWARE.

Except as contained in this notice, the name of Tavmjong Bah shall not
be used in advertising or otherwise to promote the sale, use or other
dealings in this Font Software without prior written authorization
from Tavmjong Bah. For further information, contact: tavmjong @ free
. fr.

TeX Gyre DJV Math
-----------------
Fonts are (c) Bitstream (see below). DejaVu changes are in public domain.

Math extensions done by B. Jackowski, P. Strzelczyk and P. Pianowski
(on behalf of TeX users groups) are in public domain.

Letters imported from Euler Fraktur from AMSfonts are (c) American
Mathematical Society (see below).
Bitstream Vera Fonts Copyright
Copyright (c) 2003 by Bitstream, Inc. All Rights Reserved. Bitstream Vera
is a trademark of Bitstream, Inc.

Permission is hereby granted, free of charge, to any person obtaining a copy
of the fonts accompanying this license (“Fonts”) and associated
documentation
files (the “Font Software”), to reproduce and distribute the Font Software,
including without limitation the rights to use, copy, merge, publish,
distribute,
and/or sell copies of the Font Software, and to permit persons  to whom
the Font Software is furnished to do so, subject to the following
conditions:

The above copyright and trademark notices and this permission notice
shall be
included in all copies of one or more of the Font Software typefaces.

The Font Software may be modified, altered, or added to, and in particular
the designs of glyphs or characters in the Fonts may be modified and
additional
glyphs or characters may be added to the Fonts, only if the fonts are
renamed
to names not containing either the words “Bitstream” or the word “Vera”.

This License becomes null and void to the extent applicable to Fonts or
Font Software
that has been modified and is distributed under the “Bitstream Vera”
names.

The Font Software may be sold as part of a larger software package but
no copy
of one or more of the Font Software typefaces may be sold by itself.

THE FONT SOFTWARE IS PROVIDED “AS IS”, WITHOUT WARRANTY OF ANY KIND, EXPRESS
OR IMPLIED, INCLUDING BUT NOT LIMITED TO ANY WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT OF COPYRIGHT, PATENT,
TRADEMARK, OR OTHER RIGHT. IN NO EVENT SHALL BITSTREAM OR THE GNOME
FOUNDATION
BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, INCLUDING ANY GENERAL,
SPECIAL, INDIRECT, INCIDENTAL, OR CONSEQUENTIAL DAMAGES, WHETHER IN AN
ACTION
OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF THE USE OR
INABILITY TO USE
THE FONT SOFTWARE OR FROM OTHER DEALINGS IN THE FONT SOFTWARE.
Except as contained in this notice, the names of GNOME, the GNOME
Foundation,
and Bitstream Inc., shall not be used in advertising or otherwise to promote
the sale, use or other dealings in this Font Software without prior written
authorization from the GNOME Foundation or Bitstream Inc., respectively.
For further information, contact: fonts at gnome dot org.

AMSFonts (v. 2.2) copyright

The PostScript Type 1 implementation of the AMSFonts produced by and
previously distributed by Blue Sky Research and Y&Y, Inc. are now freely
available for general use. This has been accomplished through the
cooperation
of a consortium of scientific publishers with Blue Sky Research and Y&Y.
Members of this consortium include:

Elsevier Science IBM Corporation Society for Industrial and Applied
Mathematics (SIAM) Springer-Verlag American Mathematical Society (AMS)

In order to assure the authenticity of these fonts, copyright will be
held by
the American Mathematical Society. This is not meant to restrict in any way
the legitimate use of the fonts, such as (but not limited to) electronic
distribution of documents containing these fonts, inclusion of these fonts
into other public domain or commercial font collections or computer
applications, use of the outline data to create derivative fonts and/or
faces, etc. However, the AMS does require that the AMS copyright notice be
removed from any derivative versions of the fonts which have been altered in
any way. In addition, to ensure the fidelity of TeX documents using Computer
Modern fonts, Professor Donald Knuth, creator of the Computer Modern faces,
has requested that any alterations which yield different font metrics be
given a different name.

$Id$
```

The site loads Open Sans, Poppins, Quicksand, Source Code Pro and Varela Round from Google Fonts. They use the SIL Open Font License 1.1.
