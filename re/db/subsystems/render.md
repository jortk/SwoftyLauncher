# render: D3D12/DXGI present path and the swapchain overlay

Owner: see `subsystems.tsv`. Consumer: `native/src/core/hud.cpp` (the overlay standard library, experimental `overlay` feature gate).

## Map
- The game renders with Direct3D 12 (`d3d12core.dll` + `dxgi.dll`); there is no exe-side present call site catalogued yet.
- The overlay reaches the present path through the stable COM vtable ABI, not through any `F1_25.exe` address, so it needs no KB VA and survives game updates. At arm time it creates a throwaway `D3D12CreateDevice` + `CreateDXGIFactory1` + `CreateSwapChainForHwnd` probe, reads three vtable slots, installs safetyhook inline hooks on the shared code they point at, then destroys the probe:
  - `IDXGISwapChain::Present` — vtable slot 8 (vt+0x40). Draws the frame, then calls the original.
  - `IDXGISwapChain::ResizeBuffers` — vtable slot 13 (vt+0x68). Releases the overlay's back-buffer references before the game resizes.
  - `ID3D12CommandQueue::ExecuteCommandLists` — vtable slot 10 (vt+0x50). Captures the game's `DIRECT` command queue (the one Present needs to submit the overlay's command list).

## Open
- The exe-side present call site (the render-loop function that calls `swapchain->Present`): decompile and catalogue as `Render_*` once observed in game, for a lower-overhead exe hook than the vtable probe.
- Text and image rendering: the first landing paints solid rects / outlines / lines only; a font atlas and textured-quad path remain.

## Render pipeline toward an offline preview (engine-re 2026-11-17, static)

Written by engine-re at user request (lane owner review requested). Static only (Steam d873a1c8 exe); nothing here is runtime-confirmed. Blind verification: round 1 V10803-V10816 (10 agree, 3 partial, 0 disagree); round 2 V10817-V10828 (12 agree).

## Lighting

- **Two sources for the sun.**
  - **Time-of-day curves.** Two consumers bind the `sky/*` curves from the TOD data under `Game_TimeOfDay_LockAll`, each through a 0x30-byte curve binding (`Game_TodData_FindParam`):
    - `Render_SkyState_BindTodCurves` 0x142e106f0 binds 54 keys.
    - `Render_SkyRenderer_BindTodCurves` 0x144779fd0 binds 47 keys.
    - Both rebind when the TimeOfDaySystem `curve_version` (+0xb438) moves (`Render_SkyRenderer_Update` 0x14477d170).
    - The sun direction is the pair `sky/sunHeightAngle` (default 45) and `sky/sunRotationAngle` (default 90).
    - The sun light is `sky/sunLightIntensity` (default 3), `sky/sunLightColour` and `sky/maxSunColour`.
    - The ambient light is `sky/ambientLightColour`, `sky/maxAmbientColour` and `sky/maxAmbientOvercastColour`.
    - Night: `sky/nightShadowIntensity` and `sky/nightSpecularIntensity`.
    - Fog and reflection colours come from `weather/atmospherics/*`.
  - **Real solar position.** `Game_TimeOfDay_ComputeSolarAngles` 0x1446175d0 computes elevation, azimuth, sunrise and sunset from date, clock, latitude and longitude.
    - `Sess_GameWeatherSystem_SetupSession` reads latitude from the track weather config +0x30, longitude from +0x34 and the year from +0x3c. Month and day come from the session settings.
    - `Render_SkyState_UpdateSolarSun` 0x142e15970 (owner `*(gws+0x1db0)`, sun block gws+0x1e40) evaluates three key times and writes the results into the TimeOfDaySystem from +0x267c on.
    - The longitude correction has no rounding (`(lon - (lon/15)*15) * 4`), so longitude has no effect. Only latitude, date and clock time move the sun.
    - Which DB keys feed config +0x30 / +0x34 is OPEN.
  - **HYPOTHESIS:** the curves are authored over a normalised day and stretched to the real sunrise/sunset. The runtime test in T158 will tell.
- **Per-world override: SunOverrideComponent** (game lane, `Game_SunOverrideComponent_Initialise` 0x14099e9c0).
  - It can override sun azimuth (+0xf34), height angle (+0xf38, clamped 0..pi), colour, intensity and diameter, plus explicit luminance in cd/m2, colour grading, the weather preset and "Apply SSAO to lightmap".
  - It carries the bake set as asset-group paths:
    - AO .bake (+0x160)
    - Radiance .bake (+0x280)
    - Light list .bake (+0x3a0)
    - DDGI radiance (+0x4c0)
    - DDGI config (+0xa60)
    - IBL SH prefix (+0x5e0)
    - IBL probe prefix (+0x820)
    - LUT texture (+0xb80)
  - The track DB (`Sess_Load_TrackDatabase`) names the same groups: aoBakeAssetGroup, radianceBakeAssetGroup, lightListAssetGroup, ddgiAssetGroup, iblProbes.
- **Light entity components.** Load slots near `Render_ModelComponent_Load`:
  - sun: `Render_SunLightComponent_Load` 0x144485de0 (shadow cascades, diameter, ibl* scales)
  - spot: `Render_SpotLightComponent_Load` 0x14448cf20 (cone, IES, shadows)
  - omni: `Render_OmniLightComponent_Load` 0x14447fa50
  - IBL box: `Render_IblLightComponent_Load` 0x14447ba00 and its field table `Render_IblLightComponent_RegisterProperties` 0x14447b540, with `diffuseWithLightmap` +0xdb and `ignoreIfRadianceMap` +0xda
  - volumetrics/ambient: `Render_VolumetricsComponent_Load` 0x144464e60 (global_ambient_light_map, world-space lightmap intensity)
  - Every light has `lightmap_direct`, meaning it is already in the bake. That is HYPOTHESIS: the consumer was not read.
- **Other lighting pieces.**
  - DDGI: `Render_DDGIVolumeConfig_Parse` 0x143be6f00 (has `ignoreLightmaps`).
  - Ambient/sun LUT compute: `Render_AmbientLut_Generate` 0x143ee9a00.
  - Exposure and tonemap rows already exist: `Render_Tonemap_*`, `Render_PostFx_InitTonemapper`.

### ToD key table (Render_SkyState_BindTodCurves, owner offsets, defaults)

| key | off | default |
|---|---|---|
| sky/sunHeightAngle | +0x780 | 45 |
| sky/sunRotationAngle | +0x750 | 90 |
| sky/sunLightIntensity | +0x6c0 | 3 |
| sky/sunLightColour (vec) | +0x670 | const 0x1457e7ea0 |
| sky/sunAngularDiameter | +0x6f0 | 0.533 |
| sky/maxSunColour (vec) | +0x50 | - |
| sky/ambientLightColour (vec) | +0x140 | const 0x145614f80 |
| sky/maxAmbientColour (vec) | +0x840 | 0 |
| sky/maxAmbientOvercastColour (vec) | +0x890 | 0 |
| sky/nightShadowIntensity | +0x240 | 1 |
| sky/nightSpecularIntensity | +0x720 | 1 |
| sky/sunSize / sunLum / sunDiscColour | +0x300 / +0x2d0 / +0xf0 | 3 / 1 / - |
| sky/glowSize / glowIntensity / maxSunGlowColour | +0x2a0 / +0x270 / +0xa0 | 8 / 0.6 / 0.9 |
| sky/moonRotationAngle / moonHeighAngle | +0x7b0 / +0x7e0 | 0 / 0 |
| weather/atmospherics/reflection_fade_colour, sky_fog_clear_colour, sky_fog_overcast_colour | +0x8e0 / +0x930 / +0x980 | - |
| weather/atmospherics/volumetric_fog_scale, rain_volumetric_fog_scale | +0xb80 / +0xbb0 | 0 |
| sky/planarReflectionIntensity | +0xbe0 | 0.15 |

The cloud keys sit at +0x190..+0x640 and +0x9d0..+0xb20 (see the function's decomp).

## Lightmaps

- **Every model component holds the track's lightmap location pair** (`Render_ModelComponent_Load` 0x14446a6d0):
  - +0xb8 = `eaid://ego_package/lightmap_locations`
  - +0xc0 = `eaid://ego_package/lightmap_locations_radiance`
- **LightMapLocRes** (`Render_LightMapLocResFactory_Create` 0x143c06290, object vtable 0x1458c2550, descriptor 0x146441c38):
  - The reader takes `(size-4)/32` records of 32 bytes from byte 4 on. The leading u32 count is not used.
  - Each record is a 16-byte instance uniqueid key and a 16-byte value, held in a hash map. A duplicate key keeps its first entry.
  - Value layout (file-format side, F1_TrackLab decoder): u32 atlas slice, f32 size, f32 u, f32 v.
  - The atlases are `baked_maps_ao` (BC4) and `baked_maps_radiance` (BC6H), 4 slices. Slice 0 is the planar terrain map. `Render_Texture_CreateFromResource` special-cases both.
- **Fallbacks.** `Game_Read_RendererGfxConfig` names `eaid://default_lightmap`, `default_lightmap_array_ao` and `default_lightmap_array_radiance`.
- **Round 2: the full chain, CPU to shader** (static exe + shader reflection, see "Shader side" below):
  1. `Render_ModelComponent_Load` copies the 16-byte instance uniqueid from the load descriptor (+0x98/+0xa0) to model+0xc8.
  2. `Render_ModelComponent_SendLightMapData` 0x14446e5f0 (called from slot 4 `Render_ModelComponent_OnResourcesReady` 0x14446b640 when a location resource has data) looks the id up in both maps (`Ne_HashMap_Find16` 0x143ed6e50) and posts render message 0x16 = `PROXY_MESSAGE_ID_SET_LIGHT_MAP_DATA` (0x20 bytes).
  3. `Render_ModelProxy_HandleMessage` 0x143d405b0 (vtable 0x1459b2720) stores it in the model proxy: +0x22 u16 AO slice, +0x24/+0x28/+0x2c AO size/u/v, +0x30 u32 radiance slice, +0x34/+0x38/+0x3c radiance size/u/v.
  4. The vertex shader reads the block as two float4 rows of `instanceBuffer` (t20, typed float4 buffer): row index = `SV_InstanceID * batchInstanceStride + batchStandardOffset + 8` (AO, `instanceLightData`) and `+ 9` (radiance, `instanceLightData2`), and passes them unchanged (bitcast to uint4, nointerpolation) as `LIGHTDATA_AO` / `LIGHTDATA_RAD`.
  5. The pixel shader: `uv1 = TEXCOORD0.zw` (the single TEXCOORD0 float4 holds uv0 in .xy and the lightmap uv in .zw, passed as `UV0UV1`); `atlas_uv = uv1 * size + (u, v)`; AO slice = `LIGHTDATA_AO.x >> 16` (the u16 at proxy+0x22), radiance slice = `LIGHTDATA_RAD.x`.
     - AO: `g_aoMap` (Texture2DArray t28) .x, sampled only when size > 0, else 1. `g_scene.rtFeatureFlags` bits 0x4/0x40 can replace the baked AO by 1 (ray-traced AO path). The result multiplies the material AO.
     - Radiance: `g_radianceMap` (Texture2DArray t29) .rgb, clamped to at least 0.01 per channel, used only when size > 0, else 0.
  - **A missing instance entry gives size 0, so AO = 1 and radiance = 0.** No default array is used for it (answers the static half of T159).
  - A second component type (vtable 0x145a91010, `Render_ModelComponent2_SendLightMapData` 0x1444795c0) does the same with a primary and a fallback uniqueid (comp+0x88, +0x98) and message id 4.
  - Variants: in object_opaque and track_surface every variant reads `LIGHTDATA_AO`/`_RAD` except `object_opaque_tinted` (world-space light map only); the `lightmap_*` variants bind `g_aoMap` but mostly not `g_radianceMap`.

## Materials

- **Parse.** `Asset_MaterialFactory_Parse` 0x143c0dc80 (assets lane). File layout: `gfx_material.md`.
- **Bind layout.** `Render_Material_CountResourceSlots` 0x143c07890 (name kept; it builds the bind layout, it does not count):
  - Layout = constants (slot 0, mat+0x50), one static slot (slot 1) and one slot per texture record (kind 2).
  - Kind 1 (constants) and kind 3 (bindless heap index) get no slot.
  - The slot index is written back to record+0x20.
- **Resource types.** `Render_StaticInit_ResourceTypeDescriptors` 0x143f13130 maps every render resource type name (GfxModelRes, NeMaterialRes, IBLSHRes, DDGIRes, ...) to its descriptor.
- **Round 2, shader side** (reflection of `shader_package_2025/*.nefx2.sm51.erp`):
  - **Permutations are separate resources.** Each `<group>.nefx2.sm51.erp` holds one `GfxNeFxResource` per variant (object_opaque: 30, e.g. `object_opaque`, `object_opaque_forward`, `object_opaque_wsm`, `lightmap_object_opaque`, `radiance_object_opaque_forward`; track_surface: 17). The material's shader path (`eaid://shader_package/<group>/<name>.nefx2`) names the variant directly. Each variant is a `NEFX` v8 container with one entry per pass (`gbuffer_export_ps`, `main_forward_plus_ps`, planar reflection, `wetness_ps`, `mirage_ps`) plus its vertex shaders.
  - **Material constants are not a cbuffer.** They live in `g_materialBuffer` (ByteAddressBuffer t100) at `PerBatch.materialBufferOffsetBatch + gpu_offset`. The `gpu_offset` of each constant record in the material file (gfx_material.md) is exactly that byte offset.
  - **Bindless textures.** A kind-3 HeapIndex constant (`me_*HeapIndex`) is a u32 read from the material buffer, mapped through `g_bindlessResourcesLookupTable` (StructuredBuffer<uint> t101) to a descriptor-heap index and opened with `CreateHandleFromHeap`. Samplers come from the sampler heap at `g_scene.anisoLevel + 6` or fixed slots. The exe-side fill of the lookup table is still OPEN.
  - `PerBatch` (cb0, 32 bytes): batchInstanceStride, batchStandardOffset, batchUserDataOffset, batchUserData2, materialBufferOffsetBatch, 3 pads.
- `Render_Material_ResolvePassSlot` 0x1401406e0 looks like a texture view/slot resolver (flags 0x40/0x400/0x1000, 0x180-byte entries), not a shader permutation selector. HYPOTHESIS, not changed in the KB.

## LOD

- **Instance LOD and fade use screen size, not distance.** `Render_Draw_ComputeInstanceLodScale` 0x143d41770 returns the projected size of the bounding sphere:
  - perspective: angle^2 * view+0x68
  - ortho: 2 * the projected radius
- **Per-model LOD switch values** are read and checked for ascending order by `Render_ModelResource_Parse` 0x143c16f40 (GfxModelRes v2).
- **Per-pass bias:** `Render_GeometryPass_Parse` 0x143b5c740 (lodbias, lodControlLod/Threshold/Threshold2).
- **Per-entity bias:** the "lod bias" and "min area bias" keys of `Game_Parse_ModelRenderFlags` 0x1444725e0.
- **OPEN:** the compare of the size against the model's LOD values, and the graphics-quality scale.

## Wetness

- The road wetness map is the top-down 75 x 75 m wetness render around the player (`Game_WetMap_*`, flags `render to wetness map` in the Render component).
- `Render_Bind_WetnessSamplers` 0x1447192f0 belongs to the skidmark/track-mark layer. It binds that layer's own albedo, normal, normalIntensity, surface, depth and wetness textures, so it is not the road material's wetness input.
- Track-level drive values: `weather/track/wetness`, `puddles` and `absorbtion` (see session.md#wetness), the ConstantsTODBuffer / `g_trackConstants` buffer (`Render_Gfx_CreateTrackConstantsAndPuddleBuffers`) and the puddle index buffer.
- **Round 2: `g_trackConstants` (cb2, 96 bytes), from the track_surface reflection:**
  - dustColour (float4 +0), fp32_absorbtion +16, fp32_wetness +20, fp32_puddles +24, fp32_dryingLine +28, fp32_racingLine +32, fp32_skidsStrength +36, tiledTrackInfoMapScale +40, fp32_rubbering +44, fp32_marbles +48, heat haze (+52..+72), dynamicTrackInfoEnabled +76, and fp16 pairs of the same values at +80..+92.
  - **`wetness_ps` (track_surface, puddle mask pass):** `p = saturate(height.b - 1 + in.x * 1.97 * puddles * (1 - dryingLine * mask))`, output 1 where `p^32 > 0.5`. The `height` and `mask` textures come from the material's heap indices.
  - **`gbuffer_export_ps` (wet road, partly decoded):**
    - With `dynamicTrackInfoEnabled`, wetness becomes `w' = saturate(max(0.45 * saturate(w * 2.222), noise * 2 - 1 + w))`. The noise comes from the world-space dynamic track-info map (t13, scaled by tiledTrackInfoMapScale).
    - Puddle depth uses `puddles`, `dryingLine` and the material height, with `^8` and `^32` thresholds.
    - Albedo is darkened by `1 - absorbtion * wetMask`. Roughness is pulled toward about 0.63 or 0.13 in puddles.
    - Exact blend: OPEN (T160).

## Shader side: per-frame light and scene constants (round 2, shader reflection)

Source: the DXIL reflection in `shader_package_2025/{object_opaque,deferred_render,track_surface}.nefx2.sm51.erp`. The ERP was unpacked and disassembled with the game's own dxcompiler.dll. This is static data from the shipped files; nothing was captured at runtime.

- **`g_scene` (cb4, 5876 bytes, every draw):**
  - scene_view +0 and scene_view_prev +1264: view/projection matrices, cameraPosition +880, nearAndFarParams +896.
  - sky_shadow_consts +2528 (TiledLocation + SkyShadowCascade[]) and spot_shadow_consts +3504.
  - `m_lighting` +4816: light counts and list offsets, cubeMapNumMips, maxLightingValue, iblReflectionIntensity, iblSHGridCam, the planar reflection block, ddgiVolume, denoiser packing.
  - m_timeInformation +5104 and renderTargetDimensions +5120.
  - debugSettings +5136 and deferredRenderSwitches +5232 (enable point/spot/quad/sphere/IBL/sky lights, SH probes).
  - `worldSpaceLightMapData` +5264: scaleAndBias, illuminationLevel, dynamicObjectIllumination, nightLightIntensity, nightLightSpecularScale.
  - sceneGammaAdjustment +5296, textureGradientAdjustment +5300.
  - cloudData +5312: cloudTextureBlendFactor, rainfallLevel, blendIndex1/2.
  - `hdr` +5328: HDR output and paper white.
  - viewportInfo +5472 / +5632, iblGenerationFlags +5792, rtFeatureFlags +5808, rtaoScale +5864, anisoLevel +5868.
- **The sun is a sky light, not a cbuffer field.** `g_skyLights` (StructuredBuffer t77) holds an 80-byte SkyLight record:
  - direction (float3 +0), diffuseScale +12, colour (float3 +16), shadowBias +28, ambientColour (float3 +32)
  - cascades +44, shadowFilterQuality +48, shadowsEnabled +52, billboardShadowLean +56
  - nightSpecularScale +64, tanSunAngularRadius +68
  - These map onto the sun component keys (diameter -> tanSunAngularRadius, billboard_shadow_lean, shadow_cascades) and the ToD curves (sunLightColour x intensity, ambient colours, nightSpecularIntensity).
  - Count: `m_lighting.numShadowedSkyLights` / `numUnshadowedSkyLights`.
- **Exposure:** `g_sceneLuminanceAndExposure` is a one-float StructuredBuffer (t39) written on the GPU (eye adaptation). A preview can treat it as one exposure scalar per frame.
- **Ambient and sun LUTs:** `g_ambientLUT` and `g_sunLUT` (2D) come from `Render_AmbientLut_Generate`. IBL uses g_ibl_diffuse/specular cube arrays plus `g_ibConstants` (iblSHGridTransform, iblSHGridBounds, iblKeyframe, iblMips, iblSHNumKeys). DDGI uses `g_ddgi_probe_map`, and the SH probes `g_shProbeMap`.
- **Fog: `g_fogData`** (208 bytes):
  - Mie: albedoMie, scatterCoefficientMie, absorbCoefficientMie, henyeyGreensteinG.
  - Height fog: startHeight, heightRange, opticalDepthScale, fade range.
  - Volumetric LUT block: g_volumetricLUT 3D + g_volumetricTransmittance.
  - Rain-sheet fog.
- **Night shadow:** `g_nightShadowData` (2 viewports: viewProj + intensity).
- **Sun direction on the CPU** (exe, static; see Render_SkyState_Update / Render_SkyState_UpdateSolarSun):
  - The sky/sunHeightAngle and sunRotationAngle curve values are bound but do not drive the sun light. Their bindings only gate the update.
  - The direction is the solar position:
    - mode (a): real solar position per minute from latitude, date and clock;
    - mode (b): a normalised day, zenith = (1 - dayFraction) * pi, with the azimuth blended across the sunrise/noon/sunset key times.
  - Which mode a race session uses is T158.
  - Latitude, longitude and year come from `tracks.Track.latitude` / `longitude` / `weekend_year` (db_keys rows). Longitude has no effect.

## Next rounds (toward offline preview = game), by value (updated round 2)

Done statically: lightmap placement (formula above), material constant / bindless layout, the per-frame constant layouts, sun source and lat/lon keys.

1. **Light list fill (exe):** the CPU code that writes the SkyLight record (direction from the sun entity rotation, colour x intensity, ambient) into g_skyLights, to tie the ToD curves to the GPU values. Lead: the consumer of the sun light component (+0x34 intensity, +0xa0 rotation, colour setter 0x14448a5e0).
2. **Exposure / tonemap:** the tonemap.nefx2 constants and the eye-adaptation pass that writes g_sceneLuminanceAndExposure. Without them a preview cannot match brightness; the runtime value per ToD is a capture item (T158 can log it).
3. **Deferred lighting math:** the BRDF in deferred_render (sky light + IBL + AO + radiance), so the preview shades like the game. Static from the DXIL; long but deterministic.
4. **Bindless table fill and texture resolution** (exe): who fills g_bindlessResourcesLookupTable from the material texture records.
5. **Wet road blend** (T160) and the dynamic track-info maps.
6. **LOD compare and draw distance.**
7. **DDGI / SH probe sampling** for dynamic objects.

Runtime capture still needed: T158 (sun mode and curves in a race), T159 (now only to confirm AO 1 / radiance 0 for missing entries) and T160 (wet road values).

## Round 3: sun record, tonemap and exposure, deferred lighting, bindless, shadows, fog (engine-re 2026-11-17, static)

Static only: Steam d873a1c8 exe plus a read-only disassembly of `shader_package_2025/{tonemap,deferred_render,object_opaque}.nefx2.sm51.erp` with the game's own dxcompiler.dll. Blind verification V11400-V11410 (9 agree, 2 partial, 0 disagree; corrections applied). Runtime items: T240, T241.

### Sun: how the 80-byte SkyLight record is filled

- **Chain (CPU, complete statically):**
  1. `Render_SkyState_Update` 0x142e13f20 writes into the sun light component (`[obj+0x40]`, vtable `vt_Render_SunLightComponent` 0x145a918b0):
     - the rotation rows +0xa0/+0xb0/+0xc0 from the solar zenith/azimuth;
     - intensity +0x34 (sky/sunLightIntensity lerped by weather);
     - +0x70 = the evaluated sky/ambientLightColour;
     - the colour through `Render_SunLightComponent_SetColour` 0x14448a5e0.
  2. The component posts proxy message 2 (0x90 bytes) to its SkyLightProxy. The message holds three transposed rows of the world matrix, then the component bytes +0x30..+0x8f. It is posted by:
     - `Render_SunLightComponent_AddToWorld` 0x14448aaa0 (creates the proxy, descriptor 0x146543928);
     - `Render_SunLightComponent_OnTransform` 0x144489ce0;
     - `Render_SunLightComponent_SetColour` 0x14448a5e0.
  3. `Render_SkyLightProxy_HandleMessage` 0x143d2e5d0 writes the record. Its pool is created by `Render_LightModule_CreateProxy` 0x143d2f5c0, case 2.
- **Record = shader `SkyLight` (byte for byte):**

| record | source |
|---|---|
| +0x00 direction | msg+0x04/+0x14/+0x24 = element 1 of each transposed row = row +0xb0 of the component rotation |
| +0x0c diffuseScale | diffuse_scale (+0x80) |
| +0x10 colour | `pow(byte/255, 2.2)` of the packed colour RGB (+0x30..+0x32) x intensity (+0x34); colour_temperature > 0 converts Kelvin first |
| +0x1c shadowBias | shadow_bias (+0x3c) |
| +0x20 ambientColour | +0x70 (sky/ambientLightColour from SkyState_Update) |
| +0x2c cascades | shadow_cascades (+0x38) |
| +0x38 billboardShadowLean | billboard_shadow_lean (+0x84) |
| +0x40 nightSpecularScale | specular_scale (+0x88) |
| +0x44 tanSunAngularRadius | tan(diameter (+0x8c) / 2 in radians) |
| side array (0x18) | 0, cascade_1..cascade_4 (+0x44..+0x50), cascade_blend (+0x54) |
| side array (0xc) | iblColour, iblDiffuseScale, iblSpecularScale (+0x5c..+0x64) |

- **Shadowed vs unshadowed:** a record counts as shadowed when cascades != 0 and +0x34 (shadowsEnabled) != 0. Who sets +0x30 (shadowFilterQuality) and +0x34 on the record was not seen; the GPU upload of the pool to t77 was not traced.
- **Real sun vs normalised day (static side of T158):** the direction is whatever SkyState_Update writes into the rotation rows. That comes from `Render_SkyState_UpdateSolarSun` mode (a) or (b) (round 2). Which flag a race session sets stays T158.
- **Component field map:** `Render_SunLightComponent_RegisterProperties` 0x144489df0 (group "sky-light-properties").

### Tonemap and exposure

- **ToD keys** (bound in `Game_TimeOfDaySystem_FinaliseTimeOfDay` 0x1438bc050, offsets in its KB note):
  - postprocess/tonemap: enable, ev_mode_select, max_lum, min_lum, max_scene_lum, key_value, threshold_a/_b/_garage/_tunnel/_lerp, adaption_speed_dl/_ld, bottom_scale, lock_eye_adaptaion, histogram_max/min, camera_fstop/tmin/tmax
  - postprocess/lut: enable, tint, colour, contrast, brightness
  - bloom and god_rays
  - All defaults are 0, so the weather/ToD preset must supply them.
  - Debug names `tonemap.*` (0x143b94340 / 0x143ba8110) mirror the cbuffer.
  - Per-track override: none found besides the SunOverrideComponent (explicit luminance, colour grading LUT) from round 1.
- **cbuffer `toneMapSettings`** (1032 bytes):
  - colorXFormLUT 4x4 +0, lutPCurveLerp +80, lutPCurveTODLerp +84
  - toneMapMaxLum +88, MinLum +92, KeyValue +96, Threshold +100, AdaptationSpeedDL +104, AdaptationSpeedLD +108, LumScale +112, MaxSceneLum +116
  - toneMapTotalNumOfPixels +128, HistogramLow/HighPercent +132/+136
  - exposureMode +176, cameraShutterSpeed/FStop/ISO/Tmin/Tmax +180..+196
  - two filmic spline blocks (+272 HDR, +656 SDR), mainTargetFP16 +1028
  - The CPU fill is `Render_Tonemap_Execute` / `SetupEyeAdaptation` from a settings block. The exact field-by-field copy was not traced.
- **Eye adaptation (tonemap.nefx2, shaders `calculate_histogram_scanline` + `eye_adaptation`):**
  - **Histogram:**
    - 256 bins; `bin = saturate(log2(max(dot(rgb * E8, (0.2126, 0.7152, 0.0722)), 1)) * 0.06682) * 255`, so bins cover log2 luminance 0..15.
    - E8 = exposure buffer [8], and it is applied only when mainTargetFP16 = 0.
  - **Percentile bins:** lowBin and highBin are the bins where the cumulative count crosses `N * HistogramLowPercent` and `N * HistogramHighPercent`. Then `L = 2^(((lowBin + highBin) / 2) * 15/255)`.
  - **Adaptation:**
    - `A = A_prev + (L - A_prev) * (1 - exp(-|dt| * speed))`, with speed = AdaptationSpeedDL when the scene got brighter, else LD. dt = g_scene.m_timeInformation +12.
    - A = 0.3 when a value is not finite.
    - Then `Ac = clamp(A, MinLum, MaxLum)`.
  - **Exposure:** `E = (5/6) / (8 * Ac / 2^Threshold)`; toneMapThreshold works as an EV offset, and toneMapKeyValue is not read here.
  - **Buffer `g_sceneLuminanceAndExposure` (floats):** [0] A, [1] E, [2] 1/E_prev, [3] E, [4] L, [5]/[6] low/high bin, [8] = 1.2 * 8 * Ac / 2^Threshold. A second viewport variant uses [9..11].
- **Tonemap resolve (`tonemap_and_resolve`, plain variant):**
  1. `c = rgb * E[1] * (mainTargetFP16 ? 1 : E[2])`.
  2. `Y = length(c * (0.299, 0.587, 0.114))`, `x = max(Y / MaxSceneLum, 1e-6)`.
  3. `c *= SCurve1D(x) / (MaxSceneLum * x)` (luminance-preserving 1D curve).
  4. sRGB encode, then the 3D colour-grade LUT, then colorXFormLUT, then back to linear when `g_scene.hdr.hdr == 0`.
  - The S-curve texture is the bilinear blend (`blend_lut_cs`) of lutSCurve00/10/01/11 by (lutPCurveLerp, lutPCurveTODLerp).
- **Deferred output is pre-exposed:** the lit colour is multiplied by `E[3]` and clamped to `m_lighting.maxLightingValue` before fog (unless debug flag 0x200).

### Deferred lighting math (deferred_render `main_per_pixel`, variant with sky shadows, DDGI, SH)

- **GBuffer (written by object_opaque `gbuffer_export_ps`):**

| target | content |
|---|---|
| RT0 Albedo | rgb = albedo, wet-darkened `lerp(a, a^2, min(saturate(4*wetness)*(1-metal), 0.6))`; a = material flags byte |
| RT1 Normal | xyz = normal; w = byte (baked AO x material AO) * 127, +128 flag |
| RT2 Surface | x = roughness (specular AA, wet: r * (1 - 0.4 * wet)); y = flags; z = metalness; w = byte (reflectance * 32, +64 flag) |
| RT3 Surface2 (u32) | packed RGB float (f16 bits >> 2/8/17): baked lightmap radiance (max 0.01) + emissive x scale + extra term; bit0 = valid |

- **Material:** `diffuse = albedo * (1 - metal)`, `F0 = lerp(0.0278, albedo, metal)`, `alpha = max(roughness, 0.01415)^2`.
- **AO:** `ao = min(bakedAO, SSAO)`. Coloured multi-bounce `aoC = ao^(1 - k * diffuse)`, with k from the gbuffer flags. Specular occlusion uses `ao*bakedAO` (times `1 - r^2` when roughness-linked occlusion is on).
- **Ambient / baked:**
  - The radiance from RT3 is multiplied by the diffuse albedo and AO.
  - Dynamic objects use `g_worldSpaceLightMap` (top-down, `scaleAndBias`) x `dynamicObjectIllumination` x `illuminationLevel` instead.
  - IBL comes from the probe list (per-tile `g_ibLights`, cube arrays `g_ibl_diffuse` / `g_ibl_specular`, `g_ibl_brdf` split-sum `F0 * a + b`), DDGI (`g_ddgi_probe_map`), SH probes (`g_shProbeMap`) and planar/SS reflections.
- **Sun (sky light 0):**
  - With alpha = max(roughness, 0.01415)^2 and a = alpha^2: GGX `D = a / (pi * (NH^2 (a - 1) + 1)^2)`.
  - Height-correlated Smith visibility `1 / ((NV + sqrt(NV^2 (1 - a) + a)) (NL + sqrt(NL^2 (1 - a) + a)))`.
  - Schlick Fresnel with F90 = 0.9722: `F = F0 + (0.9722 - F0)(1 - VH)^5`.
  - `Lo = colour * ((diffuse/pi * diffuseScale * (1 - F) + D * Vis * F * specOcc) * NL * shadow + k * diffuse * (shadow * saturate(dot(viewDir, -L))^5 + 0.1 * (1 - NL)))`; the last term is a back-light/translucency term weighted by the gbuffer-coded k.
  - Shadow: cascades from `g_scene.sky_shadow_consts`: 4 SkyShadowCascade entries of 208 bytes at +2560, 4-tap compare filtering, blend via cascadeBlendScale/Bias, plus the screen-space mask `g_skyShadowMaskTex` and whole-world shadow.
- **Final:** `out = (direct + ambient) * E[3]`, clamped by maxLightingValue, then fog and rain sheet (below).

### Bindless lookup table

- **Allocation:** every texture/buffer view allocates an id from the device bit allocator (device+0x1cc0, capacity 0x40000). Allocators: `Render_Gfx2Device_AllocViewDescriptor_A/B` 0x143a0e340/0x143a0e5b0 and `Render_Gfx2Buffer_Construct`.
  - The id is stored at tex+0x160 + view*4.
  - The view's descriptor-heap index goes into the u32 table device+0x1d28[id], and dirty byte +0x1ca8 is set.
- **Upload:** the render-graph node "BindlessLookupBuffer" (`Render_Bindless_AddLookupUploadPass` 0x143e815f0, execute `Render_Bindless_UploadLookupTable` 0x143f0c3b0) copies the table up to the highest id into `g_bindlessResourcesLookupTable` (t101) when dirty.
- **Shader side:** `heap = table[me_*HeapIndex]`.
- **OPEN:** the CPU code that writes the texture's bindless id into the material buffer constant (material -> tex+0x160).

### Shadow cascades

- **Settings source:** hardware_settings_config.xml `<shadows>`, read by `Render_Shadow_ReadHardwareSettings` 0x143dc39f0: skyShadowMapSize, skyShadowCascadeCount, skyShadowLowLod, worldShadowMapSize, skyShadowMap32Bits, spotShadowMapSize, spotShadowSampling.
  - A local config example has size 512, 4 cascades, world 1024; the values come from the graphics preset.
- **Per-sun settings:** shadow_cascades, cascade_1..4 split distances, cascade_blend, shadow_bias, filter quality.
- **Debug/override UI:** 0x140198750 ("Override Shadow Cascades", Start/Near/Mid/Far/Furthest, FOV scales); per-camera 0x1401e44a0 (cascades, cascade_start_distance, cascade_fov_scale_0/1).
- **Shader:** at most 4 cascades in g_scene; resolution / recipResolution / kernelRadius / lightWidth / sunArea in sky_shadow_consts.

### Fog / atmosphere

- **Inputs per track/weather:** the weather/atmospherics ToD curves (see the 0x1438bc050 note):
  - albedo, scatter/absorb coefficient distances, HG g (default 0.775), near plane/fade (100), optical depth scale (0.25), distance exp divisor/power, rain variants;
  - clear/rainy lerps, plus `volumetric_fog_scale` / `rain_volumetric_fog_scale` and the sky fog colours from round 1.
- **Shader (deferred_render, end of `main_per_pixel`):**
  1. `d = max(volume term, saturate((dist * invDistExpDivisor)^distExpPower) * dist)`.
  2. `od = sunLUT.a < 0 ? -|sunLUT.a| * d * opticalDepthScale : 0`, and `T = (1 + max(od, -4)/4)^4`.
  3. `fade = saturate(d * recipFullFadeRange + minusNearDivFullFadeRange)`.
  4. `inscatter = ambientLUT.a * (1 - T) * (fade * sunLUT.rgb + ambientLUT.rgb)`.
  5. `out = lit * T * volTransmittance + inscatter * (dist > 5000 ? volTransmittance : 1) + volumetricLUT.rgb`.
  6. Rain sheet: `out = out * rainA + rainSheetColour * rain` beyond the rain-sheet depth.
  - g_sunLUT / g_ambientLUT are indexed by (dot(sunDir, viewDir), worldY * worldPosYToLUTVMul + Add); they come from `Render_AmbientLut_Generate`.
- **OPEN:** the CPU fill of g_fogData from the curves (FogProxy / FogVolumeCBuffer 0x143caa850 not read) -> T241.

### Offline preview = game: updated plan

- **Reproducible offline now (static):**
  - lightmap placement and baked AO/radiance;
  - the GBuffer packing;
  - the sun record (direction from the sun rotation, colour = gamma-2.2 colour x intensity, angular radius) and the sun BRDF (GGX + height-correlated Smith + Schlick 0.972 + Lambert);
  - the multi-bounce AO;
  - eye adaptation and exposure formulas, the tonemap resolve order (exposure -> 1D S-curve -> sRGB -> 3D LUT -> matrix);
  - the analytic fog formula, cascade count and sizes from the hardware settings, and the bindless indirection.
- **Still needed from data or runtime:**
  - the ToD curve values per weather preset (they live in the weather/ToD data files: decode those offline next);
  - the S-curve and LUT textures (shipped assets: next);
  - the g_sunLUT / g_ambientLUT contents (generated at runtime: T241 or re-implement `Render_AmbientLut_Generate`);
  - g_fogData values (T241);
  - the exposure and the selected threshold (T240);
  - IBL probe cubes and SH (shipped bake assets);
  - the sun mode (T158) and the wet blend (T160).
- **Next static rounds:**
  1. Decode the weather/ToD curve files so the preview evaluates the same keys.
  2. The CPU fill of g_fogData and toneMapSettings (Render_Tonemap_Execute settings block).
  3. `Render_AmbientLut_Generate` math.
  4. The material -> bindless id write.
  5. The IBL probe selection (g_ibLights fill, IBLProxy).

## Round 4: ToD curve files, LUTs, IBL, bindless (engine-re 2026-11-17, static)

Static only: Steam d873a1c8 exe plus read-only game data (track ERPs, `common_all_tracks.erp`, and `ego_package` ERPs read out of `game.dat` with the NeFS header of the installed exe). Runtime items: T242, T243.

### Time-of-day curve files

- **Where they live.** Exactly four TOD files exist, shared by every track and every front-end world (13 identical copies, e.g. `environment_package/tracks/common_all_tracks.erp` resources 167-170, plus `sequence_test.todconfig`). No track ships its own TOD data.

| file | category clock | scenarios | what it holds |
|---|---|---|---|
| `sequence_data.timeofday.tod` | 0 TimeOfDay | `_master`, `timeofday_debug` | hour curves: sun/ambient/sky colours, fog, tonemap thresholds, `curve_lerp_tod`, `ibl_keyframe` |
| `sequence_data.weather.tod` | 1 Weather (per dataset) | `_master`, 16 `weather_<From>-<To>_a`, `weather_custom`, `scenario_full_transition` | clouds, `IBLReflectionIntensity`, `threshold_lerp`, `curve_lerp`, wetness/rain |
| `sequence_data.camera.tod` | 2 Camera | `_master`, 44 `camera_<type>` | bloom, dof, god rays, lens, motion blur, tonemap per camera type |
| `sequence_data.grading.tod` | 3 Grading | `_master`, 8 `grading_*` | lut tint/colour/contrast/brightness, vignette |

- **File format (TODD v2)**, parsed by `Game_TodData_ParseTodd` 0x14460d750:
  - `SLST`: u32 count, then per scenario u32 djb2(name), u32 len, name, u8 flag. The last flagged scenario becomes the clock fallback (+0x1dc).
  - `TWIN`: u32 count, then per window u32 djb2(`scenario/window`), u32 len, name, f32 start, f32 end, f32 rate, u32. Windows are 0..24 (timeofday, grading, camera) or 0..5 (weather).
  - `PATH`: u32 count, then 16-byte entries {djb2(`scenario/param`), djb2(scenario), djb2(param), u32}, sorted by the first hash.
  - `DATA`: no count, one 12-byte entry per PATH entry: u32 packed, f32 t_first, f32 t_last. Packed bits 0-5 = type (0 scalar, 2 rgb, 3 rgba, 4 bool), 0x40 = animated, bits 12-21 = key count, bits 22+ = float count.
  - Fragment 1 (`syst`) holds the key floats in record order: per key t followed by the value (1, 3 or 4 floats). The float counts sum exactly to the fragment size in all four files.
  - Times are the fraction of the window (timeofday: x 24 h; weather: fraction of the state or transition).
  - Parameter names are djb2 of the full path (`sky/sunHeightAngle`); about 75 % resolve against exe strings. The rest has no string in the exe, so no code binds them.
- **Evaluation:**
  - `Game_TimeOfDaySystem_FinaliseTimeOfDay` 0x1438bc050 binds each key category by category in the order 0 TimeOfDay, 1 Weather (active dataset clock), 2 Camera, 3 Grading; the first category whose CURRENT scenario has the key wins, otherwise the code default.
  - Binding (`Game_TodCurveBinding_Init` 0x142ff65e0) and re-binding on a version change (`Game_TodCurveBinding_Resolve` 0x14029c3f0, falls back to the clock fallback scenario).
  - Sampling (`Game_TodCurve_Sample` 0x14029c560): clamp t to [t_first, t_last], piecewise LINEAR between keys, constant when one key; no wrap. Vector keys go through the colour variant 0x14460f1d0.
  - x = clamp01((t_a + t_b) * rate / length), x 24 for 24 h clocks.
  - Current scenario per category: weather from the stitch (`weather_<From>-<To>_a`), camera `camera_<type>` from `Game_TimeOfDaySystem_UpdateCameraScenario` 0x144620a40. Every clock starts on `_master` (`Game_TimeOfDaySystem_Construct` 0x144618560). Whether the TimeOfDay category runs `_master` (constants) or `timeofday_debug` (hour curves) in a race is OPEN -> T242.
- **Offline evaluator recipe:** parse the four files; for a (time, weather state, camera type) take the scenario per category, look up each key in the category order above (scenario first, `_master` as fallback), sample linearly.
- **Stock values seen:**
  - `_master`: max_lum 0.2, min_lum 0.1, max_scene_lum 3, key_value 0.18, adaption_speed_dl 0.02, sunHeightAngle 55, sunRotationAngle 319.5.
  - `timeofday_debug`: threshold_a 6.0 at 10:18 falling to 1.30 at 24:00; `ibl_keyframe` 0 at 09:00 to 5 at 24:00; `curve_lerp_tod` 1 at 10:47 to 0 at 13:02.
  - weather: `threshold_lerp` / `curve_lerp` 0 clear, 1 overcast.

### LUTs and post splines (ego_package, in game.dat)

- **Colour grading 3D LUTs:** `ego_package/colour_grading/colourgrades.erp` holds apex, broadcast, cinematic, grid_reveal, podium and standard `.cube.tex.image`.
  - Format: 33^3 R11G11B10_FLOAT (first texels 0, 1/32, ...), stored as a 1089 x 33 strip (surface header format 0x4a), 143748 bytes.
  - Selection: by name from the camera clip results (`Game_CameraActorSyncJob_ApplyClipResults`, `colour_grading/%s.tex.image`) and the grading scenario; `standard.cube` is the descriptor default.
- **Track default post-process (`ego_package/postprocess/track_default/`):**
  - `3d_lut_32x32_n.out.tga` / `neutral_lut.tga`: 32^3 neutral LUT as a 1024 x 32 strip, 1 byte per pixel block-compressed (format 0x45).
  - `default.psplines` and `neutral.psplines` (PostSplinesRes): u32 count, then per curve u32 name length, name, u32 channels, u32 samples, channels x samples f32.
  - Curves: Tonemapping (1 x 8192), LUT (4 x 256), Bloom (1 x 512), BloomTint/StreakTint/fe tints, SSAO_FOV_RAD, and DayClearLUT, DayOvercastLUT, NightClearLUT, NightOvercastLUT (4 x 256 each). The last four are the lutSCurve00/10/01/11 inputs blended by lutPCurveLerp (`postprocess/curves/curve_lerp`, weather) and lutPCurveTODLerp (`curve_lerp_tod`, time).
  - `effects.todpp` (TimeOfDayPP) is a small TODD file for effect_0 / flashback / terminal_damage_stop.
  - Loaded by `Render_PostProcess_LoadResources` 0x143bc2d30 as `eaid://ego_package/postprocess.psplines`; the id-to-file mapping is in T242.
  - Per track: no stock track ERP ships LUT or psplines resources. The only per-world hook is the SunOverrideComponent "LUT texture" path `asset_groups/ego_package/postprocess/track_default/<name>` (round 1).

### Ambient: IBL, SH, DDGI per track

- `tracks/<t>/probes/key_<year>.erp`:
  - 20 keys x {diffuse, specular} BC6H cube arrays of 14 probes (diffuse 16^2 1 mip, specular 64^2 5 mips).
  - `keys_<year>.diffuse_sh.sh`: 20 x 14 x 9 float4 SH.
  - `config_diffuse/specular_<year>.ibl`: XML numProbes, numKeys, size, probe name to slot. Parsed by `Render_IBLConfig_Parse` 0x143c01350.
- `probes_garage_NN.erp`: IBLSH `.shprobes` grids (magic SHPR, GRID).
- `config.ddgi.erp`: DDGI XML config only (cascades, counts 16x12x16, spacing 6 m, hysteresis 0.97). The DDGI probes are traced at runtime, nothing is baked.
- **Key selection:** `Game_GameWeatherSystem_UpdateFrame` 0x1438b9ff0 turns `ibl_keyframe` into two neighbouring keys plus a fraction, mirrored after the turn time, plus the same pair offset by +0x1f2c for the other weather set, weighted by the dataset weather curve. `IBLReflectionIntensity` (1 clear, 0.4 storm) is a specular scale. The values per session are in T243.

### Fog and tonemap CPU fill (static side of T240/T241)

- **Fog:** `Game_GameWeatherSystem_EvalFogSettings` 0x1438b17e0 writes the fog settings block from the fog curves.
  - albedo, scatter distance lerped clear to rain by the dataset rain weight, absorb distance, HG g, near plane/fade, optical depth scale x rain lerp, 1/exp divisor, exp power, override colour.
  - The copy of that block into the g_fogData cbuffer on the render side is not traced.
- **Tonemap:** `Render_Tonemap_ComputeCurveParams` 0x143bca600 is the full toneMapSettings fill from a per-view 0x2e8-byte settings record (field map in its KB note).
  - Threshold = lerp(threshold_a, threshold_b, threshold_lerp).
  - Adaptation speeds x 100; ISO 100 and shutter 0.01 are fixed.
  - Who copies the curve values into the record is not traced; the field order matches the key order.

### Bindless material ids

- `Render_Material_ResolveBindlessIds` 0x143c0a100 (callback of `Render_Material_CountResourceSlots`) writes the texture's bindless id `[tex+0x50]+0x160+view*4` (buffers `[res+0x18]+0x160`) into every heap-index parameter record (+0x2c) and marks the material dirty.
- That record value is what g_materialBuffer carries as `me_*HeapIndex`.

### Offline preview = game: plan after round 4

- **An offline renderer now has:**
  - all ToD/weather/camera/grading curve data with an exact parser and linear evaluator, plus the layering rule;
  - post splines (S-curve, 4 LUT curves) and the colour-grade cube LUTs from game.dat;
  - the toneMapSettings and fog block formulas;
  - per-track IBL key cubes and SH with the key-selection rule;
  - the lightmaps, sun, BRDF, AO and fog formulas from rounds 1-3;
  - material bindless ids.
- **Still missing:**
  - the active TimeOfDay scenario in a race and the psplines id mapping (T242);
  - the IBL key and weight values per session (T243);
  - the per-probe IBL selection on the GPU (g_ibLights fill, IBLProxy);
  - the ambient/sun LUT generation math (`Render_AmbientLut_Generate`);
  - the render-side copy of the fog block into g_fogData and of the curves into the 0x2e8 tonemap record;
  - DDGI (runtime ray tracing; approximate with IBL + SH offline);
  - exposure in a real frame (T240);
  - the sun mode (T158) and the wet blend (T160).

## Inbox
- 2026-10-19 14:03 ui-re: new 'hud' lane holds the HUD view-model/logic side (what widgets publish + their setToOpen gates); render still owns the D3D present/draw path. Hud_Open_TyrePerformance (0x14152b5f0) publishes to view_model_data.hud_* registry nodes. HUD VM vtables cluster near 0x1456ed290 if you need draw-time addresses.
- 2026-10-28 22:57 re-import: (wx rounds 1-3, static, verified by wx-verify): render flags +0x32 live on the per-entity Render component (type 0x146cb7b70, vtable 0x145a90b10, parser 0x1444725e0); the Model component (type 0x146cb7da0, vtable 0x145a908a0) holds +0xb0 Render, +0x80 render instance (current world matrix inst+0x10..+0x3f, previous +0x40..+0x6f, generator mask +0x70), +0xd8 model path. Per-model setter Render_ModelComponent_ApplyRenderProps 0x14446ea70 -> 0x14446b9c0 (KB fix: not LitPlanarReflectionProxy). rainMask (0x20) = screen-space depth from the main camera that clips falling rain only; wetness map (0x40) = top-down ortho 75 x 75 m around the player, 256x256 (0x14478c050 / 0x14478e440); dry/wet meaning lives in material shaders. Quality handler 0x14015aa00 field map in game notes. | findings from PR #471 by vogtmarcel07-cloud
- 2026-10-28 09:45 engine-re: 2026-10-28 engine-re (wx rounds 1-3, static, verified by wx-verify): render flags +0x32 live on the per-entity Render component (type 0x146cb7b70, vtable 0x145a90b10, parser 0x1444725e0); the Model component (type 0x146cb7da0, vtable 0x145a908a0) holds +0xb0 Render, +0x80 render instance (current world matrix inst+0x10..+0x3f, previous +0x40..+0x6f, generator mask +0x70), +0xd8 model path. Per-model setter Render_ModelComponent_ApplyRenderProps 0x14446ea70 -> 0x14446b9c0 (KB fix: not LitPlanarReflectionProxy). rainMask (0x20) = screen-space depth from the main camera that clips falling rain only; wetness map (0x40) = top-down ortho 75 x 75 m around the player, 256x256 (0x14478c050 / 0x14478e440); dry/wet meaning lives in material shaders. Quality handler 0x14015aa00 field map in game notes.
- engine-re finding for Render_ModelComponent_Load 0x14446a6d0: Model component (type 0x146cb7da0, vtable 0x145a908a0, 0x118 bytes) slot 3: model+0xd8 = char* 'model' path (14446a7ec-800 via 0x143fa2de0), +0xe0 'mat binding'; +0xa8 entity; +0xb0 = Ne_Entity_FindComponent(entity,0x146cb7b70) Render component (14446a7d9/7e5); resource key '<model>?context=render?render' ids +0x68/+0x70, resource +0x78, lightmaps +0xb8/+0xc0 (wx-verify claim 5) | L2 2026-10-28: user OK for engine-re to raise render rows (render lane owner overlay); xref = caller or vtable slot pointer
- engine-re finding for Render_Apply_InstanceRenderProperties 0x14446b9c0: CORRECTION 2026-10-28 (wx): NOT LitPlanarReflectionProxy (that is type 0x146cb60a0, vtable 0x145a90870, factory 0x145a90880); this is generic Model component code. data=Render+0x18; byte [data+0x1a]=Render+0x32 tested 0x20 -> mask 0x146cb66f0 (rainMask), 0x40 -> 0x146cb6570 (wetnessMapGenerator), 0x80 -> 0x146cb65b0 (mirageGenerator) (14446d313/d393/d413), vpor/vpandn into render instance +0x70 (128-bit generator mask); +0x33 0x40 rainBeads 0x146cb6710; +0x34 0x10 -> service vtbl[+0x20](handle,12,0) (broadcast-only marker). Then cmpxchg inst+0x128 0->2, queue {0x143d3ce20,inst}, inst+0x12c|=0x20. Verified by wx-verify claims 1,2 | L2 2026-10-28: user OK for engine-re to raise render rows (render lane owner overlay); xref = caller or vtable slot pointer
- engine-re finding for Render_ModelComponent_ApplyRenderProps 0x14446ea70: Per-model render-property setter (wx 2026-10-28, static): rdx=[model+0xb0] (Render component found at load), add rdx,0x18, tail jmp 0x14446b9c0 (14446ea87/93/a0, only direct ref to 0x14446b9c0); called from AddToScene 0x14446e7b0 when model+0x10e set; without a Render component applies default masks staticGenerator+lowLODShadows. Rain-blocker recipe: rp=Ne_Entity_FindComponent(entity,0x146cb7b70); rp+0x32|=0x20 (rainMask) or &=~0xC0 (no wetness/mirage); call this(model) if model+0x80. Notes work/weather/findings_gfx.md | L2 2026-10-28: user OK for engine-re to raise render rows (render lane owner overlay); xref = caller or vtable slot pointer
- engine-re finding for Render_ModelComponent_OnTransform 0x144470870: Model secondary iface vtbl[1]: inst=[rcx+0x68] (=model+0x80); writes CURRENT world matrix as 3 transposed rows at inst+0x10/+0x20/+0x30 (14447092d-937); copies old to inst+0x40..+0x6f only if [inst+0x13c]&8; skips if counter +0x13a>=3 and unchanged; sets inst+0x12c|=1 (wx-verify claim 3) | L2 2026-10-28: user OK for engine-re to raise render rows (render lane owner overlay); xref = caller or vtable slot pointer
- 2026-10-29 11:48 engine-re: engine-re findings 2026-10-29 for the render lane owner (static, blind-verified v4-verify S2): camera 0x150 bytes: +0x70 projection 4x4 row-major (v' = v*M), +0xb0 view, +0xf0 inverse view, eye +0x120, near +0x2c, far +0x30; 0x143be2070(cam, view) writes +0xb0 and inverse +0xf0; Render_View_SetFromCamera 0x143be0360(V, cam) copies to V+0x10 (proj V+0x80, view V+0xc0), different projection branch when [0x14618caa0]+0x161; called per viewport at 0x1401583d0 in 0x140157d90 (unique site pattern E8 ?? ?? ?? ?? 42 83 BC BE F4 0A 00 00 00). Used by vmknolle race-start for a world-to-screen overlay.
- engine-re finding for Render_OnWeatherQualityChanged 0x14015aa00: Log channel WEATHER: 'GameRendererModule::onWeatherQualityChanged' | 'GameRendererModule::onWeatherQualityChanged: rainBeadsEnable %d' | 'GameRendererModule::onWeatherQualityChanged: rainSplashesEnable %d' | 'GameRendererModule::onWeatherQualityChanged: rainHighEnable %d' | 'GameRendererModule::onWeatherQualityChanged: heatHazeE
- engine-re finding for Render_Evaluate_CarDamageLevels 0x142f17170: damage level per part group for the renderer (cases 0..10: wheels, front wing, sidepods/mirrors, floor/bargeboards, rear wing = mean of rearwing_bl/br/top, diffuser).
- engine-re finding for Render_Particles_CreateCarEffect 0x143af5620: named from its log messages (channel PARTICLES): 'USED!! | Car start %u | Car stop %u | Couldn't create effect '%s'!'
- engine-re finding for Render_Particles_ApplySpawns 0x143b05a50: named from its log messages (channel PARTICLES): 'perform %d spawns and %d destroys and %d tints | trying to create an effect %s in a slot %04x where we have one already, ignoring (maybe we skipped a delete in replays?)'
- engine-re finding for Render_GroundCover_Construct 0x143ce0630: named from its log messages (channel GROUND_COVER): 'Constructed NeGroundCoverSystem | Enhanced ground cover is enabled (ultra quality is needed).'
- engine-re finding for Render_GroundCover_LoadCascadeModel 0x144467200: named from its log messages (channel GROUND_COVER): 'Ground cover cascade %d loading model: %s' lb22-verify partial
- engine-re finding for Render_Particles_OnVehicleEvent 0x14463da30: named from its log messages (channel PARTICLES): 'tyre burst | LOOK OUT - veh index not same as array index, should be OK, but may need testing %d %d | - | unknown surface '%s' | I expected a null handle but got %p | KILL EFFECTS at start of fast rew' lb22-verify partial
- engine-re finding for Render_Particles_SelectScrapeEffect 0x144704740: named from its log messages (channel PARTICLES_C): 'mat '%s' effect name '%s' scrape '%s' priority %d'
- engine-re finding for Render_Skidmarks_Init 0x144711290: trackinfo parser for skid marks and the wetness section: SKIDMARKS, SKIDMARKS_BLEED, TYRE_RESIDUE, SKIDMARKS_CONFIG, WETNESS (wet_range, duration_no_rain, duration_full_rain, puddle_range), CAR_MARKS_ENABLE. The car marks are what dries the racing line in the wetness map.
- engine-re finding for Render_Bind_WetnessSamplers 0x1447192f0: binds six named texture/sampler pairs of a pass, among them wetnessSamplerTex / wetnessSampler (the wetness map the surface shaders read); keeps a width/height ratio of the first texture in 0x145faa648.
- engine-re finding for Render_Skidmarks_Update 0x14471f650: Log channel SKIDMARKS: 'Something has gone wrong during the mark update code.'
- engine-re finding for Render_PuddleBuffer_Readback 0x14479f9c0: reads the GPU puddle index list back (count <= 0x10000, 'invalid puddle data count') and marks a 64 KB byte map per slot ([[+0x40]+0x40]+0x60 -> +8 + slot*0x10000, 256 x 256 cells) where puddles are.
- 2026-11-02 00:26 placed-record: placed-record: 0x1439b9390/0x1439b98c0/0x1439f4150 and site_Render_Gfx2Heap_RecordIndex raised at L1 from re/decomp (heap object layout in the Create notes; frame start 0x1439f4150 releases records 0..count-1 at 0x1439f5172). Owner: please review to L2 and add an independent agree. 0x143a06ea0 (GetImmediateCommandList) indexes lists by the +0x1490/+0x1494 counters.
- 2026-11-05 14:01 sponsor-studio: sponsor-studio: added at L1 with static L2 evidence (decomp read, callers listed, data flow in re/research/sponsors.md); please raise after verification: Render_CustomisationDecalsSet_OnProxyMessage 0x143c772f0.
- 2026-11-14 02:27 engine-re: engine-re 2026-11-14 weather to L2 (static, for the render owner): Game_TrackInformationSystem_Update 0x144795cf0 writes the PREVIOUS frame track wetness ws+0x3f0a38 (ws = *g_Game_WeatherSurfaceState) to [[ws+0xd08]+0x48]+0xa5b7e4 (0x144795f48) before it re-evaluates weather/track/wet_delta, and a temperature-derived value to +0xa5b7ec (0x144796052); rain intensity reaches the rain renderer as [TimeOfDayOwner+0x2c8]+0x75ca4 (0x14479c47e). Data flow and struct layout: kb:subsystems/session.md#wetness / #weather. Render rows (Render_Bind_WetnessSamplers 0x1447192f0, Render_Gfx_CreateTrackConstantsAndPuddleBuffers 0x1447889a0) were not touched.
- 2026-11-17 04:46 rr-bindings: Binding request (rr-bindings, 2026-11-17; doc:re/research/weather_strategy_bindings.md#functions): please review for L2 and export. (1) Render_SceneView_BuildViewParams 0x143be0360 (L1, sig render.scene_view_build_view_params): rcx (rsi) = view params block it fills, rdx (rdi) = source view; projection at +0x70 (0x143be0902), world-to-view +0xb0.. (0x143be0815/0x143be082d), camera-to-world +0xf0.. (0x143be0855/0x143be086d/0x143be08ad), second pass 0x143be1629..0x143be1684; 27 callers. (2) site_Render_MainViewParamsCall 0x1401583d0 (L1, sig render.main_view_params_call): the main 3D view's call to it, in 0x140157d90-0x140158a63. Realistic Rain draws its puddle pass from this camera (inline hook, return address = site+5); both moved +0x25d0 / +0 on Steam 1.0.145.
- 2026-11-17 14:16 engine-re: engine-re 2026-11-17 render-pipeline round 1 (static, blind-verified V10803-V10816): new sections Lighting / Lightmaps / Materials / LOD / Wetness / Next rounds above this inbox; 15 new render rows (6 at L2 forced), 3 raised to L2 with corrected notes (0x143c07890 builds the material bind layout, 0x143d41770 sphere screen size, 0x1447192f0 skidmark layer samplers). Lane owner review requested. Runtime questions: T158 (sun curves vs solar position), T159 (lightmap location lookup).
- 2026-11-17 engine-re: render-pipeline round 3 (static, blind-verified V11400-V11410): section "Round 3" above the inbox (SkyLight record fill chain via SkyLightProxy, tonemap/eye adaptation formulas, deferred BRDF and GBuffer packing, bindless table fill, shadow settings, fog formula). 2 new render rows at L2 (forced), 8 L1 rows, 3 globals. Lane owner review requested. Runtime: T240 (exposure), T241 (fog/SkyLight values).
