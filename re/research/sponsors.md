# Sponsors, decals and the My Team livery

The engine side of F1 25's sponsors: the car decal system and its editor, the My Team decal catalogue, career sponsor
records and discovery, the per-decal income, engine supplier history, the team badge emblem, decal rendering and the
teamwear screens. It is the research behind Sponsor Studio and its Sponsor Base addon (JonnieDesigns), recorded on
the KB build (`F1_25.exe` sha256 `d873a1c8…`, pe_timestamp 1789118616). For the database side, see
[Adding a sponsor](../../docs/reference/data/sponsors.md).

**Levels** follow PROTOCOL §1:

- **L1** is static inference.
- **L2** means the function was decompiled and read, with its callers and data flow written down. Decompiles are cited as
  `re/decomp/<addr>.c`; regenerate them with `re/tools/decomp`, because none is shipped here.
- **L3** means seen in a real game session: the contributor's passive trace addons, the Sponsor Base's own logs, or the
  owner's in-game checks. The `T###` numbers below belong to the contributor's private test lane, not to `re/tests/`.
  The logs are available on request.

`re/kb_sponsors.sh` adds the new rows at L1 and leaves raising them to the lane owners. **Nothing here is
independently verified** (PROTOCOL §5), so no behaviour hook is cleared under §2 on the strength of this file. The
Sponsor Base ships behaviour hooks on these sites anyway, as a third-party addon whose owner authorised them; that
was the owner's decision and isn't a KB clearance.

Each table's last column says whether the row is new, already in the KB, or corrects a KB row.

## The decal system and its editor list {#decal-system}

`game::GameDecalCustomisationSystem` is a registered system. Its type key is
`k_Game_GameDecalCustomisationSystemDesc` (0x146064d10), found by `lea key; call Ne_RobinHoodMap_U64_FindIndex` on
`[[g_Game_Manager]+0x30]` at 16 sites. The vtable `0x1456e3b50` was seen in game. One object lives for the whole
session.

**The editor list.** `+0x178`/`+0x180` hold 16-byte `{node *, refcount *}` entries of `DecalEditorNode`s (0x70 bytes):

| offset | field |
|---|---|
| `+0x10` | item key `(DecalSet.id << 16) \| 9` |
| `+0x18` | RGBA colour slot (`0xffffffff`) |
| `+0x20` | uniqueId |
| `+0x28` | area (`ProjectionArea.id`) |
| `+0x30` | translation |
| `+0x40` | rotation |
| `+0x50` | scale |
| `+0x68` | the mirrored partner's uniqueId |

The layout was seen in T013.

**Load and store.**

- **Copy:** `0x14142bd50(system, &key)` loads a saved livery into the list. The key is a vehicle customisation stat id:
  byte 0 is the kind (below 3), and bits 16–31 index `[[system+0x258]+0x388+kind*0x10]`. Kind 0 holds the 69 database
  rows (AI drivers) and kind 1 the player's liveries. An unknown key makes it read through a null record, so check the
  key before calling.
- **Writeback:** `0x14142d800(system, flag)` stores the list back. The editor calls it with `flag 0` after every edit.
- **Add, remove, update:** `0x14142dc70(system, template)` adds a node, and the game creates the mirrored partner itself.
  `0x14142c330(system, &uid)` removes one, and `0x14142c6f0(system, &uid, node)` replaces one's placement.

| address | name | level | role | KB |
|---|---|---|---|---|
| `0x14030ffd0` | `Ne_RobinHoodMap_U64_FindIndex` | L1 | Registry lookup used as `lea key 0x146064d10; call 0x14030ffd0(map,&key); entry == [map+0x20] -> missing; value = entry+8` at 16 sites for GameDecalCustomisationSystem, and 92 probes for playerCustomisationInfoSystem (0x146064e78) on [[g_Game_Manager]+0x30]. | in KB L1 |
| `0x140fe5900` | `Game_CareerOnlineManagerComponent_CleanSaveSlotData` | L1 | Caller of badge getter 0x141443700 passing game mode and save slot. | in KB L1 |
| `0x141106ea0` | `Ne_Vector_PushBack_Ptr` | L1 | Ordinary vector append used by the area-node collection 0x14ac8bfb0. | new |
| `0x141108480` | `Ne_Vector_PushBack_Elem16` | L1 | Append used by the add path 0x14142dc70. | in KB L2 |
| `0x1413fac70` | `Game_DecalNode_Serialize` | L1 | Decal editor node serialiser (node vtable 0x1456e3240 slot at 0x1456e3258): writes all 32 bits of the +0x18 colour slot to saves. | new |
| `0x1413fbb70` | `Game_DecalNode_Deserialize` | L1 | Decal node deserialiser (vtable slot at 0x1456e3260): restores the +0x18 colour slot. | new |
| `0x14141efe0` | `Game_CustomisationDecals_LoadConfig` | L1 | Decal system load: reads ProjectionArea rows into 0x108-byte area records at system+0x98..+0xa0 (field handles: +0xc id, +0x30 reserved_slots_count, +0x48 mirrored_up_inversed, +0x50 mirrored_right_inversed, +0x58 decal_origin_point, +0x60 default_xz_scale, +0x68 min_xz_scale, +0x70 max_xz_scale, +0x78 default_y_scale, +0x80 input_up, +0x88 input_right, +0x90 top_left_corner, +0x98 bottom_right_corner, +0xa0.. camera fields; values read at use: float record +8, vector record +0x10 x / +0x14 y). reserved_slots_count read at 0x14141f835; matching_mirrored_side read nearby. Also a preset reader of livery_finish_%d labels (resolved to the finish enum via 0x1424fa150/0x141438290). The inherited Gdb_Get_FieldInt prototype is unsuitable for its string fields. | in KB L2, adds detail |
| `0x141427e80` | `Game_CustomisationDecals_CreateServiceForEditorNode` | L1 | Refresh of current editor decals (rendering) after a list change; called after 0x14142bd50 copy. The Sponsor Base DLL calls it as refresh(system, 0, 0) after adding nodes, and the car updates. | in KB L1 |
| `0x141428060` | `Game_DecalGeneration_CreateService` | L1 | Dispatches decal generation/reload work; not a requirement-binding reader. | in KB L1 |
| `0x14142a160` | `Game_CustomisationDecals_EnumCompatibleAreas` | L1 | Enumerates compatible projection AREA definitions (not node ordering) for the slot view builder 0x141ff8b10. | new |
| `0x14142a6d0` | `Game_CustomisationDecals_HasNoFreeSlot` | L1 | Called as 0x14142a6d0(system, container+0x80, 0x141ff7610(screen)); the menu builder enables Duplicate only when it returns false, else shows lng_decal_editor_no_free_slots. Role inferred from that call site only. | new |
| `0x14142a8b0` | `Game_CustomisationDecals_CollectAreaNodes` | L1 | Thunk; body 0x14ac8bfb0 (.data1) scans editor nodes system+0x178/+0x180, filters node.area (+0x28) and appends matches via 0x141106ea0 in SOURCE LIST ORDER (no sort by uniqueId). | new |
| `0x14142aa90` | `Game_CustomisationDecals_GetAreaCentre` | L1 | Only function found reading both area corners directly: (top_left+bottom_right)/2 as the area centre for a transform (likely editor camera focus). No movement clamp against the corners was found. | new |
| `0x14142bd50` | `Game_CustomisationDecals_SetEditorVehicleCustomisation` | L2 | copy(system, const u32 *key): saved livery -> editor list. Resolves the livery with 0x14246dcb0([system+0x258], key) (key = vehicle customisation stat id: byte0 kind <3, bits16-31 index); an unknown key returns 0 and copy then reads 0+0xe8 (crash), so keys must be validated first. Reloads only when key != [system+0x1a8] and the [system+0x2b0] map entry for byte [system+0x1ac] is type 5; copies livery +0xe8/+0xf0 nodes (item identity, uniqueId, area, transforms) into editor list system+0x178/+0x180 and sets +0x1a8 = key, then 0x141427e80 refreshes. No call to the placement-JSON parser 0x1414384e0 and no unconditional required-sponsor insertion on this path. | in KB L1, adds detail |
| `0x14142c330` | `Game_CustomisationDecals_RemoveEditorDecalNode` | L1 | remove(system, const u64 *uniqueId): removes the node and also its linked partner (node+0x68 = partner uniqueId). Only 3 callers. | in KB L1, adds detail |
| `0x14142c6f0` | `Game_GameDecalCustomisationSystem_ApplyEditorDecalState` | L1 | update(system, const u64 *uniqueId, const Node *src): overwrites item, area and transform of the node with that uniqueId in place, then its partner. Only 4 callers: 0x14142ce50 (catalogue swap commit), 0x14142e050 (flip), 0x14142e7f0, 0x141ff05f0 (decal editor screen incl. type carousel). | in KB L1 |
| `0x14142c860` | `Game_DecalCustomisation_SyncLinkedDecalNode` | L1 | Copies node data (including the +0x18 colour slot) to the mirror partner. | in KB L1 |
| `0x14142ce50` | `Game_CustomisationDecals_CommitCatalogueSwap` | L1 | Catalogue swap commit: replaces an existing node in place through 0x14142c6f0 (no remove+add). | new |
| `0x14142d630` | `Game_CustomisationDecals_StepEditorNodeScale` | L1 | (system, &uniqueId, float *step): editor scale step; node scale x (+0x50) and z (+0x58) each = clamp(\|s\|+step, area min_xz_scale, max_xz_scale) keeping the sign (flip preserved); y untouched. So min/max_xz_scale are the editor's hard scale limits. | new |
| `0x14142d800` | `Game_CustomisationDecals_WriteBackEditorDecals` | L2 | writeback(system, u8 flag): editor list (system+0x178/+0x180) -> 0x142477900 (store into the selected vehicle customisation), then 0x14142daa0; flag != 0 also posts via system+0x278 and clears dirty byte +0x168; then sets +0x1a8 = 0x3fff0000 and copy(old key) reloads. NOT a clear-all. Editor callers pass flag 0 after each edit. Within it the call site 0x14142da2a leads to the driver-customisation decal copy 0x141401110 (T015). | new |
| `0x14142daa0` | `Game_CustomisationDecals_WriteBackStep` | L1 | Called by writeback 0x14142d800 after the store call 0x142477900; role not traced. | new |
| `0x14142dc70` | `Game_CustomisationDecals_AddEditorDecalNode` | L1 | add(system, const Node *tmpl0x70): copies the supplied 0x70 node, allocates uniqueId above the system counter (+0x1a0 per T013 hypothesis) / current list maximum (observed max+1), appends via 0x141108480, then guarded matching-area partner handling: one add on left_nose created the right_nose partner itself. Identified by string 'addEditorDecalNode()' 0x1456e3de8 referenced at 0x14142dce9. Template fields observed: +0x10 item key, +0x18 -1, +0x20 uid -1, +0x28 area, +0x30 translation, +0x50 scale, +0x60 0, +0x68 -1. | in KB L1, adds detail |
| `0x14142e050` | `Game_GameDecalCustomisationSystem_FlipEditorDecalNode` | L1 | Modifies artwork transform axes via 0x14142c6f0; it does NOT create or lock an opposite-side mirrored decal. | in KB L1 |
| `0x14142e220` | `Game_CustomisationDecals_MirrorEditorDecalNode` | L1 | Target of decal action 3 (mirror) in dispatcher 0x141ffdef0. An earlier note called it 'a separate duplicate path'; the later call-graph section maps it to mirror. | new |
| `0x14142e7f0` | `Game_DecalCustomisation_SwapAssetGroup` | L1 | Update/state machine of the registered GameDecalCustomisationSystem with per-vehicle 'services' (log 'Service for id %I64d is deactivated, removing from GameDecalCustomisationSystem'); one of the 4 callers of 0x14142c6f0. | in KB L1, adds detail |
| `0x1414346c0` | `Game_AsyncFileLoadOp_DoJob` | L1 | Asynchronous raw file allocRead/wait; NOT the JSON parser. | in KB L1 |
| `0x141438290` | `Game_Livery_ParseFinishType_2` | L1 | Second copy of the finish label -> enum chain used for presets (livery_finish_%d). | new |
| `0x1414384e0` | `Game_Livery_ParseDecalsJson` | L1 | Decal placement JSON reader: root 'decals' array; per node uniqueId, mirrorLinkedDecalUniqueId, id, projectionAreaId, translation, rotationAngle, scale (integer reads for id/projectionAreaId; no sponsor or reserved flag). Builds node item key as ((id << 16) + 9) plus a middle byte from a temporary; stores uniqueId at node+0x20, projectionAreaId at +0x28. Loads via allocRead, reports JSON parse errors. T013 later showed 'id' = customisation.custom_decal_sets...DecalSet.id (type 9 is not UnlockableItemKey type 9 eAvatarPose). | in KB L1, adds detail |
| `0x141458e00` | `Game_Decal_VectorAssign16` | L1 | Receives the parsed placement list from 0x1424fa3a0. | in KB L1 |
| `0x141a20850` | `Game_Customisation_RemoveTeamCustomisations` | L2 | Passes one entrants pointer to badge remove 0x142470be0 and vehicle-customisation remove 0x1424773f0; also a caller of badge getter 0x141443700. | new |
| `0x14246dcb0` | `Ent_Get_VehicleCustomisation_Thunk` | L2 | (store=[decal system+0x258], u32 key): key byte0 kind (<3), bits16-31 index into list [store+0x388+kind*0x10] (+8 begin, +0x10 end, 16-byte entries, record pointer at +8); unknown key returns 0. Kind 0 = the 69 DB vehicle_customisation rows (AI drivers); kind 1 = player liveries (observed index 0x10 F1 World, 0x11 My Team in the owner's save). Also used by 0x14142bd50 to select the vehicle customisation list. | new |
| `0x1424773f0` | `Ent_EntrantsSystem_RemoveVehicleCustomisation` | L2 | Called from 0x141a20850 (same object as badge remove): badges (+0x3b8) and vehicle liveries (+0x388) share one entrants system. | in KB L1 |
| `0x142477900` | `Ent_EntrantsSystem_ReplaceVehicleCustomisation` | L1 | Store call of decal writeback 0x14142d800 (editor list -> saved vehicle customisation). | in KB L1 |
| `0x142492ab0` | `Game_Read_LegacySlotDecalLists` | L1 | Reads legacy decal_colour_list into +0x18 of legacy entries sharing the node prefix {vtable, 0x145613520, key, colour}, packing R \| G<<8 \| B<<16 \| A<<24 (= bake shader packedAlbedoColour layout). | new |
| `0x1424fa150` | `Game_Livery_ParseFinishType` | L1 | Livery finish label -> enum (lower-case FNV-1a64 compare chain: eSatin 1, eMatte 2, eMetallic 3, else 0) used by the preset reader 0x1424fa3a0; duplicated inline chain also in 0x141438290. | new |
| `0x1424fa3a0` | `Game_Customisation_ReadDecalPlacementPreset` | L1 | Finds a customisation.decal_customisation_system.decal_placement_files.DecalPlacementFile row by label, resolves the cached path by its id, calls the JSON reader 0x1414384e0 (E8 at 0x1424fb4d8) and passes a successful list to 0x141458e00. Content is cached by GameDecalFileMananger (async). Also resolves preset livery_finish_%d labels to the finish enum (0x1424fa150/0x141438290). DB consumers of decal_placement_file: randomisation_livery_designs LiveryDesign rows and entrants.vehicles.vehicle_customisations rows. | new |
| `0x1424ff270` | `Game_Read_LegacyProfileDecalList` | L1 | Legacy profile decal list reader (decal_name_list / single_decal_position_list / decal_colour_list / decal_node_version / decal_list under driver_customisation keys); drops decal_colour_list. | new |
| `0x14313dbd0` | `Game_NetSerialisableVehicleDecalNodeList_CreateDecalNodeList` | L1 | Network decal node list reader; the network packet carries the per-node colour (+0xc in the packet). | in KB L1, adds detail |
| `0x1445a2bd0` | `Gdb_Find_RowByLabel` | L1 | Used by 0x141114160 to look decal names up in decal_sets/DecalSet. | in KB L2 |
| `0x1445a2cf0` | `Gdb_Get_FieldInt` | L1 | Absent key returns &info[8]; inherited prototype is unsuitable for some string fields (do not infer return types from the name). | in KB L2 |

Data, vtables and code sites:

| address | name | kind | level | role | KB |
|---|---|---|---|---|---|
| `0x14141f835` | `site_Game_DecalAreas_ReadReservedSlots` | code site | L1 | reserved_slots_count read; handle stored at area record +0x30 (local KB xref 0x14141f829). | doc only |
| `0x14142dce9` | `site_Game_AddEditorDecalNode_LogString` | code site | L1 | Reference to string 0x1456e3de8 'addEditorDecalNode()'. | doc only |
| `0x141fed3de` | `ret_Game_DecalEditor_WritebackAfterPlace` | code site | L3 | Return address of writeback after each placement (containing function not stated). | doc only |
| `0x141ff694a` | `ret_Game_DecalEditor_WritebackOnExit` | code site | L3 | Return address of writeback on editor exit (containing function not stated). | doc only |
| `0x1424fb4d8` | `site_Game_ReadPreset_CallParseDecalsJson` | code site | L1 | Direct E8 call to 0x1414384e0. | doc only |
| `0x142ef58f6` | `crash_Game_DecalEditor_EmptyRowList` | code site | L3 | Fault reading 0xb8 (rdx=rdi=0xb0) reached from 0x141fec530+0x114 when the editor row list screen+0x490..0x498 was empty. Fault address written as 142ef58b0+0x46 in the contributor's notes; absolute = 0x142ef58f6. | doc only |
| `0x14537db28` | `crash_Game_ShutdownAccessViolation` | code site | L3 | Pre-existing shutdown AV on launcher Stop (identical in ~29 sessions incl. tracer-free ones); unrelated to sponsor work. | doc only |
| `0x145613520` | `k_Game_DecalNodePrefixPtr` | data | L1 | Second qword of the shared node prefix {vtable, 0x145613520, key, colour} seen in legacy entries; meaning unknown. | new |
| `0x1456e3240` | `vt_Game_DecalEditorNode` | vtable | L1 | Vtable of the 0x70-byte decal editor node; slots at 0x1456e3258 (Serialize 0x1413fac70) and 0x1456e3260 (Deserialize 0x1413fbb70). | new |
| `0x1456e3b30` | `s_GameDecalCustomisationSystem` | string | L1 | Type name of the decal system descriptor 0x146064d10. | doc only |
| `0x1456e3b50` | `vt_Game_GameDecalCustomisationSystem` | vtable | L3 | Vtable of the decal editor system object; value observed in game via the Sponsor Base DLL ('vtable +0x56e3b50', one object for the whole session, stable). prepare_signatures checks it is outside .xtls with slot 0 in .xtls. | new |
| `0x1456e3de8` | `s_addEditorDecalNode` | string | L1 | 'addEditorDecalNode()' string, referenced at 0x14142dce9 inside 0x14142dc70. | doc only |
| `0x1456e4ef8` | `s_playerCustomisationInfoSystem` | string | L1 | Type name of descriptor 0x146064e78. | doc only |
| `0x1457647e8` | `s_DecalAction_Edit` | string | L1 | Menu label (edit) used by 0x141ff79b0. | doc only |
| `0x1457647f7` | `s_DecalAction_Delete` | string | L1 | Menu label (delete). | doc only |
| `0x145764808` | `s_DecalAction_Swap` | string | L1 | Menu label (swap). | doc only |
| `0x145764817` | `s_DecalAction_Mirror` | string | L1 | Menu label (mirror). | doc only |
| `0x145764828` | `s_DecalAction_Duplicate` | string | L1 | Menu label (duplicate). | doc only |
| `0x1457649bc` | `s_lng_decal_editor_no_free_slots` | string | L1 | Message loaded into r15 from 0x141ff7ddd on (Duplicate/Mirror disabled). | doc only |
| `0x145764a01` | `s_lng_decal_editor_reserved_swap` | string | L1 | Disabled-swap message for reserved slots. | doc only |
| `0x145764a20` | `s_lng_decal_editor_reserved_delete` | string | L1 | Disabled-delete message for reserved slots. | doc only |
| `0x145766728` | `vt_Game_CustomisationMyTeamOverviewVM_InitSlot` | vtable slot | L2 | Vtable slot holding 0x1420527d0 (CustomisationMyTeamOverviewViewModel init). | doc only |
| `0x1457f1070` | `vt_Game_UnlockMethodMyTeamSponsorReward` | vtable | L1 | UnlockMethodMyTeamSponsorReward marker on sponsor unlock items (type 0x146080a50); all methods return constants (no logic). | new |
| `0x146064cf0` | `k_Game_CustomPaintFinishesSystemDesc` | data | L1 | CustomPaintFinishesSystem type descriptor (name hash 0x7d59a59a); same registration pattern as the decal system. | new |
| `0x146064d10` | `k_Game_GameDecalCustomisationSystemDesc` | data | L2 | Type descriptor / registry key of game::GameDecalCustomisationSystem (name 0x1456e3b30, hash 0xc9ec67cf, parent 0x146cb8538), written in static init 0x1414751e0; 16 lookup sites `lea key; call 0x14030ffd0` on [[g_Game_Manager]+0x30]. | new |
| `0x146080a50` | `k_Game_UnlockMethodMyTeamSponsorRewardType` | data | L1 | Type of the UnlockMethodMyTeamSponsorReward marker. | new |
| `0x146cb8538` | `k_Game_SystemDescBase` | data | L1 | Common parent descriptor of GameDecalCustomisationSystem and playerCustomisationInfoSystem. | new |
| `0x1565f32c9` | `k_Ui_TeamHqContextValueObf` | data | L1 | Static obfuscated constant: [0x1565f32c9] ^ 0x48241c8b resolves to 2 (team_hq) in 0x141643760. | new |

Fields (proposed struct files in `re/structs/` hold the ones with concrete types):

| struct | offset | type | name | level | meaning |
|---|---|---|---|---|---|
| DecalEditorNode | `0x0` | void ** | `vtable` | L1 | 0x1456e3240. |
| DecalEditorNode | `0x8` | void * | `prefix_ptr` | L1 | 0x145613520 in legacy entries sharing the node prefix; meaning unknown. |
| DecalEditorNode | `0x10` | uint32_t | `item_key` | L3 | (customisation.custom_decal_sets.decal_sets.DecalSet.id << 16) \| 9 (595 monomic_n, 301 euterpe_n, 661 techpace_n). Type 9 here is NOT UnlockableItemKey type 9 (eAvatarPose). |
| DecalEditorNode | `0x18` | uint32_t | `colour` | L3 | Per-decal RGBA8 colour slot (R low byte), default 0xffffffff on every observed node; serialised, networked, copied to partner and on swaps; nothing in the shipped game sets it. Whether rendering uses it = T016 (pending). |
| DecalEditorNode | `0x20` | uint64_t | `unique_id` | L3 | uniqueId; -1 in an add template, game allocates max+1. |
| DecalEditorNode | `0x28` | uint32_t | `projection_area_id` | L3 | ProjectionArea.id (2025 ids 0..20 with gaps, 2026 ids 21..38; id 0 is a real area). |
| DecalEditorNode | `0x30` | float[3] | `translation` | L3 | Translation (JSON translation[x,y,z,1]). |
| DecalEditorNode | `0x40` | float | `rotation_angle` | L3 | rotationAngle (captured by the Sponsor Base DLL, e.g. +-0.088). |
| DecalEditorNode | `0x50` | float[3] | `scale` | L3 | Scale x/y/z; x (+0x50) and z (+0x58) clamped by the editor scale step, sign = flip. |
| DecalEditorNode | `0x60` | uint64_t? | `unknown_60` | L3 | 0 in every add template. |
| DecalEditorNode | `0x68` | uint64_t | `partner_unique_id` | L3 | Mirror partner uniqueId (-1 none); remove/update also act on the partner. |
| EntrantsSystem (ENTRANTS_SYSTEM customisation store) | `0x298` | list[3] | `driver_customisations` | L1 | [+0x298+kind*0x10]: +8 begin, +0x10 end, 16-byte entries, record at +8 (+0x290 pair filled by 0x142466a10). |
| EntrantsSystem (ENTRANTS_SYSTEM customisation store) | `0x388` | list[3] | `vehicle_customisations` | L2 | Same list layout; kind 0 = DB rows (AI), kind 1 = player liveries. |
| EntrantsSystem (ENTRANTS_SYSTEM customisation store) | `0x3b8` | list[3] | `badges` | L2 | Same list layout; [X+0x280]+0x3c8 = kind-1 badge list. |
| GameDecalCustomisationSystem | `0x0` | void ** | `vtable` | L3 | 0x1456e3b50 (observed in game). |
| GameDecalCustomisationSystem | `0x98` | ProjectionAreaRecord * | `areas_begin` | L1 | Area records, 0x108 stride (..+0xa0 end). |
| GameDecalCustomisationSystem | `0xa0` | ProjectionAreaRecord * | `areas_end` | L1 |  |
| GameDecalCustomisationSystem | `0x168` | uint8_t | `dirty` | L2 | Editor dirty byte; cleared by writeback when flag != 0. |
| GameDecalCustomisationSystem | `0x178` | DecalNodeRef * | `editor_nodes_begin` | L2 | Editor node list, 16-byte entries {node*, refcount*}; same layout as saved livery lists. |
| GameDecalCustomisationSystem | `0x180` | DecalNodeRef * | `editor_nodes_end` | L2 |  |
| GameDecalCustomisationSystem | `0x1a0` | uint64_t | `uid_counter` | L1 | Counter used when allocating uniqueId (T013 hypothesis; observed allocation = max+1). |
| GameDecalCustomisationSystem | `0x1a8` | uint32_t | `editor_livery_key` | L2 | Key of the livery loaded in the editor (vehicle customisation stat id); 0x3fff0000 = none (set by writeback before reload). Runtime: 0x110001 in My Team, 0x100001 F1 World. |
| GameDecalCustomisationSystem | `0x1ac` | uint8_t | `service_key` | L2 | Byte used as key into the +0x2b0 map; copy reloads only if that entry is type 5. |
| GameDecalCustomisationSystem | `0x258` | EntrantsSystem * | `entrants` | L2 | Vehicle customisation store (lists at +0x388); same object as playerCustomisationInfoSystem+0x280 is an open check. |
| GameDecalCustomisationSystem | `0x278` | void * | `post_target` | L2 | Writeback with flag != 0 posts via this. |
| GameDecalCustomisationSystem | `0x2b0` | map | `services` | L2 | Service map keyed by +0x1ac. |
| GameDecalCustomisationSystem | `0x418` | ? | `mode` | L1 | Mentioned as 'mode +0x418' in the career-load analysis; meaning not traced. |
| PlayerCustomisationInfoSystem | `0x120` | list | `assigned_badge_customisation_head` | L2 | List nodes {+0 next, +8 prev, +0x10 game_mode_stat_id, +0x20 tree head, +0x28 tree count}; tree nodes {+0 left, +8 parent, +0x10 right, +0x19 isnil, +0x20 save_slot_id u64, +0x28 badge key u32}. |
| PlayerCustomisationInfoSystem | `0x128` | uint64_t | `assigned_badge_customisation_count` | L2 |  |
| PlayerCustomisationInfoSystem | `0x1b0` | ? | `standard_driver` | L2 | Save name. |
| PlayerCustomisationInfoSystem | `0x1b4` | ? | `standard_driver_customisation` | L2 | Save name. |
| PlayerCustomisationInfoSystem | `0x1b8` | ? | `standard_badge` | L2 | Save name. |
| PlayerCustomisationInfoSystem | `0x1d0` | hash map | `standard_vehicle_customisations` | L2 | Save name. |
| PlayerCustomisationInfoSystem | `0x280` | EntrantsSystem * | `entrants` | L2 | Badge record store (L2 for the badge-deleted use). |
| ProjectionAreaRecord | `0xc` | GdbFieldHandle | `id` | L1 | Area id handle. |
| ProjectionAreaRecord | `0x30` | GdbFieldHandle | `reserved_slots_count` | L1 | Field handle of the ProjectionArea row field; values read at use (float: record+8; vector: +0x10 x, +0x14 y). |
| ProjectionAreaRecord | `0x48` | GdbFieldHandle | `mirrored_up_inversed` | L1 | Field handle of the ProjectionArea row field; values read at use (float: record+8; vector: +0x10 x, +0x14 y). |
| ProjectionAreaRecord | `0x50` | GdbFieldHandle | `mirrored_right_inversed` | L1 | Field handle of the ProjectionArea row field; values read at use (float: record+8; vector: +0x10 x, +0x14 y). |
| ProjectionAreaRecord | `0x58` | GdbFieldHandle | `decal_origin_point` | L1 | Field handle of the ProjectionArea row field; values read at use (float: record+8; vector: +0x10 x, +0x14 y). |
| ProjectionAreaRecord | `0x60` | GdbFieldHandle | `default_xz_scale` | L1 | Field handle of the ProjectionArea row field; values read at use (float: record+8; vector: +0x10 x, +0x14 y). |
| ProjectionAreaRecord | `0x68` | GdbFieldHandle | `min_xz_scale` | L1 | Field handle of the ProjectionArea row field; values read at use (float: record+8; vector: +0x10 x, +0x14 y). |
| ProjectionAreaRecord | `0x70` | GdbFieldHandle | `max_xz_scale` | L1 | Field handle of the ProjectionArea row field; values read at use (float: record+8; vector: +0x10 x, +0x14 y). |
| ProjectionAreaRecord | `0x78` | GdbFieldHandle | `default_y_scale` | L1 | Field handle of the ProjectionArea row field; values read at use (float: record+8; vector: +0x10 x, +0x14 y). |
| ProjectionAreaRecord | `0x80` | GdbFieldHandle | `input_up` | L1 | Field handle of the ProjectionArea row field; values read at use (float: record+8; vector: +0x10 x, +0x14 y). |
| ProjectionAreaRecord | `0x88` | GdbFieldHandle | `input_right` | L1 | Field handle of the ProjectionArea row field; values read at use (float: record+8; vector: +0x10 x, +0x14 y). |
| ProjectionAreaRecord | `0x90` | GdbFieldHandle | `top_left_corner` | L1 | Field handle of the ProjectionArea row field; values read at use (float: record+8; vector: +0x10 x, +0x14 y). |
| ProjectionAreaRecord | `0x98` | GdbFieldHandle | `bottom_right_corner` | L1 | Field handle of the ProjectionArea row field; values read at use (float: record+8; vector: +0x10 x, +0x14 y). |
| ProjectionAreaRecord | `0xa0` | GdbFieldHandle | `camera_fields` | L1 | Field handle of the ProjectionArea row field; values read at use (float: record+8; vector: +0x10 x, +0x14 y). |
| VehicleCustomisation | `0x78` | uint32_t | `bound_sponsor_ref?` | L1 | -1 = not title-bound; compared with the active commercial record +0xdc by 0x141ff7610. Meaning (livery-bound sponsor reference) is a candidate. |
| VehicleCustomisation | `0xe8` | DecalNodeRef * | `decal_nodes_begin` | L2 | Saved decal node list (16-byte entries, same node layout); counted by income 0x141104190 branch B. |
| VehicleCustomisation | `0xf0` | DecalNodeRef * | `decal_nodes_end` | L2 |  |

## The decal editor screen and its slot menu {#decal-editor}

The slot action menu `0x141ff79b0` adds its entries through `0x141855ba0(list, id, label, callback, ?,
enabled, message)`. Four checks decide whether an action is greyed:

- **Swap and Delete:** greyed when the slot index is below the area's reserved count. That count is ProjectionArea
  `reserved_slots_count`, filled by the current title sponsor. The comparisons are `site_Game_DecalMenu_SwapReservedCheck`
  (ecx = slot index, r12d = count) and `…DeleteReservedCheck` (r15d). Raising the count just past a slot greys the action
  with the title sponsor's own message; the owner saw this in game.
- **Duplicate:** enabled when `!0x14142a6d0(system, container+0x80, 0x141ff7610(screen))`, the "no free slot" test.
- **Mirror:** goes through `0x141ffdd40(screen, container)`. The game's disabled branch is
  `site_Game_DecalMenu_MirrorDisabledBranch`.

The stack holds the container at `[rsp+0x78]` and the slot element, with its node at `+0x250`, at `[rsp+0x80]`.

Title-bound liveries get virtual title placeholders before the real nodes (`0x141ffb850`, `0x141ffd450`). Raising the
database reserved count alone only adds more placeholders.

| address | name | level | role | KB |
|---|---|---|---|---|
| `0x1413ffe80` | `Game_Customisation_ConstructWithTimestamp` | L1 | Used by 0x1416cb9b0 to 'optionally reconstruct preset customisation' (our wording, vague). | in KB L1 |
| `0x141643760` | `Ui_Get_CustomisationContext` | L1 | Customisation context accessor: registry path ui_menu_data.customisation.context; djb2 0xd7d4dc84 = team_hq; static XOR constant [0x1565f32c9] ^ 0x48241c8b resolves to 2. Values on this build: home_screen 1, team_hq 2, my_team_setup 3, driver_setup 4. Build-specific static mapping. | new |
| `0x1416cb9b0` | `Game_Customisation_ResolveVehicleCustomisation` | L1 | Resolves the vehicle customisation via global 0x146066be0, optionally reconstructs a preset customisation via 0x1413ffe80, and resolves that item's key through its type registry. | new |
| `0x141855ba0` | `Ui_Menu_AddEntry` | L1 | Menu entry add (list, id, label, callback, ?, enabled, message); prototype inferred from the call sites in 0x141ff79b0 only. | new |
| `0x141fec530` | `Game_DecalEditor_OnBindingMessage` | L1 | Editor VM binding handler that reads end+0xb0 of the 0xc0-stride row list screen+0x490..+0x498 without an emptiness check. Crash: thread VM, fault at 0x142ef58b0+0x46 reading 0xb8 from 0x141fec530+0x114 (note says 'Vm_Dispatch_BindingMessage', upstream 0x1442c00e0, on the stack) with rdx=rdi=0xb0 i.e. empty list. | new |
| `0x141fec9a0` | `Game_DecalEditor_CommitEdit` | L2 | Editor caller of writeback 0x14142d800 with flag 0 after an edit; other role not traced. | new |
| `0x141ff0070` | `Game_DecalEditor_ClampPlacementScale` | L1 | Reads area min/max scale (likely clamp on place/swap). | new |
| `0x141ff05f0` | `Game_DecalEditor_Screen_Update` | L1 | Decal editor screen incl. the type carousel; caller of update 0x14142c6f0. | new |
| `0x141ff5950` | `Game_DecalEditor_SetDirty` | L1 | Source of the editor dirty byte used with writeback; delete_all_decals / delete_all_decals_mirror strings nearby are confirmation/message identifiers, not proven separate command handlers. | new |
| `0x141ff7530` | `Game_DecalEditor_IsReservedSlot` | L1 | Matches the selected area by id and tests the selected slot index against that area's reserved_slots_count; assigns no sponsor identity. | new |
| `0x141ff7610` | `Game_DecalEditor_IsTitleBoundLivery` | L1 | Calls 0x1416cb9b0 with VM+0x428 (vehicle customisation); false when vehicle customisation +0x78 == -1; in modes 2/3 true when it equals the active commercial record +0xdc (or no record found). Earlier note ('checks selected decal-associated ID') was corrected: not a lookup of the selected decal node; +0x78 meaning (livery-bound sponsor reference) remains a candidate. | new |
| `0x141ff78c0` | `Game_DecalEditor_GetReservedSlotDefaultDecal` | L1 | Default decal for an empty reserved slot: reads current-team records through VM+0x588 collection +0x298/+0x2a0 and team key +0x648, iterates 0x160 records testing flags +0xd8/+0x158, returns the first eligible default item via 0x141107420 and the customisation registry (g_FeTile_CustomisationSystem). Does not pick a brand by area or slot index; default generation stays coupled to current commercial records. | new |
| `0x141ff79b0` | `Game_DecalEditor_BuildSlotActionMenu` | L2 | Occupied-decal action menu: entries via 0x141855ba0(list, id, label, callback, ?, enabled, message). Labels edit 0x1457647e8 (id 0, always enabled), delete 0x1457647f7 (id 1), swap 0x145764808 (id 2), mirror 0x145764817 (id 3), duplicate 0x145764828 (id 4). In team_hq mode 2 swap/delete require slot_index >= area reserved count (sites 0x141ff80b0 / 0x141ff81eb: ecx index, r12d resp. r15d reserved), else messages lng_decal_editor_reserved_swap/_delete (0x145764a01/0x145764a20). Duplicate enabled = !0x14142a6d0(system, container+0x80, 0x141ff7610(screen)), else lng_decal_editor_no_free_slots (r15 = 0x1457649bc from 0x141ff7ddd on). Mirror: at 0x141ff7ee1 calls 0x141ffdd40(rcx=screen rsi, rdx=container rbx); al==0 -> disabled entry at 0x141ff7f7b with message r15; partner linked -> disabled lng_decal_editor_decal_already_linked. Stack: [rsp+0x78] container (set at 0x141ff70a0), [rsp+0x80] element whose +0x250 is the node (0x141ff72f0); rsi = screen, screen+0x290 = editor system. Guards do not call 0x141ff7610. (L1 for the earlier reserved-guard part, L2 for the 0.9.6 decompile+asm part.) | new |
| `0x141ff8790` | `Game_DecalEditor_OnDeleteAllCallback` | L1 | Callback reaching delete-all 0x141ffdaa0; calls writeback 0x14142d800 (flag 0) twice. | new |
| `0x141ff8b10` | `Game_DecalEditor_BuildSlotViews` | L1 | Enumerates compatible area definitions through 0x14142a160 then calls 0x141ffb850 per area. | new |
| `0x141ffa120` | `Game_DecalEditor_AddSlotRow` | L1 | Appends one slot row (real node handle or zero for virtual rows). | new |
| `0x141ffb850` | `Game_DecalEditor_BuildAreaSlotView` | L1 | Per-area slot list: in team_hq (mode 2) AND 0x141ff7610 true it searches the active commercial record and creates reserved_slots_count VIRTUAL title rows via 0x141ffd450 (zero node handle) before the real node rows (0x141ffa120 with node handle); otherwise real nodes start at slot index 0. Reserved label at slot+0x1a8 for index < reserved; lng_reserved_slot_baked_livery labels virtual rows only for baked title liveries. Raising reserved_slots_count here expands title placeholders, it does not reserve another brand. | new |
| `0x141ffd450` | `Game_DecalEditor_AddVirtualTitleSlot` | L1 | Creates a virtual (title) slot row: passes a zero shared-node handle to 0x141ffa120. | new |
| `0x141ffdaa0` | `Game_DecalEditor_DeleteAllInArea` | L1 | Delete-all: iterates the selected area's slots; in mode 2 (team_hq) calls remove 0x14142c330 only when slot index >= reserved count. Guard reads only the context mode (0x141643760 == 2) and the index; it does not call 0x141ff7610. | new |
| `0x141ffdd40` | `Game_DecalEditor_CanMirror` | L1 | Mirror availability test (rcx screen, rdx container); al == 0 -> Mirror disabled. | new |
| `0x141ffdef0` | `Game_DecalEditor_DispatchSlotAction` | L1 | Action dispatcher: 0 edit -> route to_decal_editor, 1 delete -> confirm, 2 swap -> route to_decal_catalogue, 3 mirror -> 0x14142e220, 4 duplicate (its cleanup calls remove 0x14142c330). Calls writeback (flag 0). | new |
| `0x141ffea20` | `Game_DecalEditor_OnDeleteConfirmed` | L1 | Delete-confirm callback (action 1) -> remove 0x14142c330; calls writeback (flag 0). | new |

Data, vtables and code sites:

| address | name | kind | level | role | KB |
|---|---|---|---|---|---|
| `0x141ff70a0` | `site_Game_DecalMenu_StoreContainer` | code site | L2 | Sets [rsp+0x78] = container used by the menu checks. NOTE: 0x141ff70a0 is below the function start 0x141ff79b0 in the contributor's notes; if it is a separate function it is the provider of the container, not an offset in the menu builder. | doc only |
| `0x141ff72f0` | `site_Game_DecalMenu_StoreElement` | code site | L2 | Source of [rsp+0x80] = slot element with node at +0x250. Same caveat as 0x141ff70a0 (address below the menu builder start). | doc only |
| `0x141ff7dd8` | `site_Game_DecalMenu_DuplicateFreeSlotCheck` | code site | L2 | mov edx,eax; xor dl,1: al = result of 0x14142a6d0 'no free slot'; al = 1 greys Duplicate with lng_decal_editor_no_free_slots. | new |
| `0x141ff7ddd` | `site_Game_DecalMenu_LoadNoFreeSlotsMsg` | code site | L2 | From here on r15 = 0x1457649bc (lng_decal_editor_no_free_slots). | doc only |
| `0x141ff7ee1` | `site_Game_DecalMenu_MirrorCheck` | code site | L2 | mov rcx,rsi; mov rdx,rbx before call 0x141ffdd40(screen, container). Game's disabled-Mirror branch at 0x141ff7f7b (menu+0x5cb, bytes 488b8e30050000 = mov rcx,[rsi+0x530]). | new |
| `0x141ff80b0` | `site_Game_DecalMenu_SwapReservedCheck` | code site | L2 | cmp ebx,2; setne al: mode test for the Swap guard; ecx = slot index, r12d = area reserved count, [rsp+0x80] = slot element (+0x250 node). Raising r12d past the index greys Swap with lng_decal_editor_reserved_swap. | new |
| `0x141ff81eb` | `site_Game_DecalMenu_DeleteReservedCheck` | code site | L2 | cmp ebx,2; setne al: mode test for the Delete guard; ecx = slot index, r15d = reserved count, [rsp+0x80] = slot. | new |

Fields (proposed struct files in `re/structs/` hold the ones with concrete types):

| struct | offset | type | name | level | meaning |
|---|---|---|---|---|---|
| DecalEditorScreen / VM (rsi in 0x141ff79b0) | `0x290` | GameDecalCustomisationSystem * | `editor_system` | L2 | Editor system. |
| DecalEditorScreen / VM (rsi in 0x141ff79b0) | `0x3f8` | ? | `carousel_ctx` | L1 | Passed to 0x1416d2790 by 0x141feea20. |
| DecalEditorScreen / VM (rsi in 0x141ff79b0) | `0x428` | ? | `vehicle_customisation_ctx` | L1 | VM+0x428 passed to 0x1416cb9b0 by 0x141ff7610 (same object as the screen not confirmed). |
| DecalEditorScreen / VM (rsi in 0x141ff79b0) | `0x490` | Row * | `rows_begin` | L1 | 0xc0-stride row list (filled by 1416d2790 as param_1[0x13..0x14]); empty list crashes 0x141fec530. |
| DecalEditorScreen / VM (rsi in 0x141ff79b0) | `0x498` | Row * | `rows_end` | L1 |  |
| DecalEditorScreen / VM (rsi in 0x141ff79b0) | `0x588` | ? | `sponsors_collection` | L1 | VM+0x588 collection whose +0x298/+0x2a0 are team records (0x141ff78c0). |
| DecalEditorScreen / VM (rsi in 0x141ff79b0) | `0x648` | uint32_t | `team_key` | L1 | VM+0x648 (0x141ff78c0). |
| DecalEditorSlotElement | `0x250` | DecalEditorNode * | `node` | L2 | Element at [rsp+0x80] in the menu builder. |
| DecalEditorSlotRow | `0x1a8` | ? | `reserved_label` | L1 | Reserved label for index < reserved (0x141ffb850). |

## The My Team decal catalogue and type carousel {#catalogue}

The My Team decal catalogue screen builds its tiles in `0x141fe5a10`, not in the editor's decal-type carousel
`0x1416d2790`. In the career branch (`Game_Get_CustomisationMode_Body` returns 2) it works like this:

- it walks `CareerSponsorsComponent+0x500`;
- for each sponsor it looks for a decal item whose `+0x38` equals the sponsor's group id at `+0xdc`;
- a match becomes a tile through `0x141fe9600`; no match takes the natural no-tile path `0x141fe6d70`;
- the entry builder `0x141feb380` sets the lock state at entry `+0x18`, which is whether the player team has a record for
  the sponsor.

A tile's texture is `*(*(item+0x30)+0x10)`: the sponsor's `DecalGroup.texture_name`, as seen in game. At
`site_Game_DecalCatalogue_TileSearch`, `…DesignEntry` and `site_Game_DecalTypeCarousel_Entry`, `rax` is the decal type
record and `[rax+0xc8]` its item. Pointing `rax` at a zeroed block skips the entry: the owner saw engine-supplier
sponsors filtered this way.

| address | name | level | role | KB |
|---|---|---|---|---|
| `0x1416d2790` | `Game_DecalEditor_BuildDecalTypeCarousel` | L1 | Decal type carousel (bindings decal_types/selected_decal_type): obtains CareerSponsorsComponent in team_hq/my_team_setup (reads +0x298..+0x2a0, 0x88 stride), walks a decal tree and builds one VM row per entry (texture/show_texture/locked/loyalty_required) from provider 0x1416d4910; keeps only entries whose item +0x40 byte / +0x42 u16 equal the key at 0x146066bf0 (sponsor-category decals). locked/loyalty_required come only from unreached loyalty levels (loyalty_required = level index + 1); a decal only in default_decals is unlocked. Tree loop site 0x1416d2f83 (rax entry, [rax+0xc8] item, *(*(item+0x30)+0x10) texture); rsi = 0 takes skip path 0x1416d40e0 (in-order tree successor via rbp); shown-entry counter only advances for built rows. | new |
| `0x1416d4910` | `Game_DecalEditor_DecalTypeProvider_Thunk` | L1 | 5-byte thunk into .data1 body 0x14b69e440, which calls 0x1417be2a0. | new |
| `0x1417be2a0` | `Game_Customisation_FilterUnlockTree` | L1 | Generic unlock/filter logic over customisation trees; attaches the career sponsor component for team_hq/my_team_setup; additional provider/tree logic. Sponsor-group identity filter not established. | new |
| `0x1417d74c0` | `Game_Init_DecalCategoryKey` | L1 | Static init setting the key at 0x146066bf0 to kind 0x14, id 0xffff (all sponsor-category decals). | new |
| `0x141fe5a10` | `Game_DecalCatalogueVM_BuildTiles` | L2 | My Team decal catalogue screen VM (CustomisationCarDecalCatalogueViewModel) builds its tiles here, not in carousel 0x1416d2790. Career branch (thunk 0x14b52b6e0() == 2): CareerSponsorsComponent+0x500..+0x508 sponsor list -> sorted 32-byte entries -> per sponsor (+0xdc group id) search of the decal tree (param_1+0x340) for an item with +0x38 == group id; match -> tile via 0x141fe9600 + push 0x142149410; no match -> 0x141fe6d70, no tile. Sites 0x141fe6e8e (tile search, mov rbx,[rax+0xc8]) and 0x141fe637a (non-career per-design branch, mov rbp,[rax+0xc8]). Entry builder 0x141feb380; tile loyalty_level = top-down scan. | new |
| `0x141fe8920` | `Game_DecalCatalogueVM_SetTileTexture` | L1 | Sets the row texture from item+0x30 -> +0x10 (texture name). | new |
| `0x141fe9600` | `Game_DecalCatalogueVM_BuildTile` | L1 | Catalogue grid row/tile: texture/placed_count/is_selected/locked/loyalty_level; stores the item found for the sponsor at row+0x108. | new |
| `0x141feb380` | `Game_DecalCatalogueVM_BuildEntry` | L1 | Entry builder: entry+0x18 = 'player team has a record with this sponsor_id' (walks the team's record vector) = tile lock state ('discover first'). | new |
| `0x141feea20` | `Game_DecalCatalogue_PlaceOrSwap` | L1 | Decal catalogue: global 0x146066c08 == -1 means new placement (add 0x14142dc70, return 0x141fef4df), otherwise swap of that uniqueId committed through 0x14142ce50 -> 0x14142c6f0 (in place). Fills the editor's 0xc0-stride row list screen+0x490..+0x498 via 0x1416d2790(screen+0x3f8, ..., 0x146066bf0). Also reads min/max scale (likely clamp on place/swap). | new |
| `0x142149410` | `Game_DecalCatalogueVM_PushTile` | L1 | Tile push after 0x141fe9600. | new |
| `0x14b52b6e0` | `Game_Get_CustomisationMode_Body` | L1 | Thunk returning 2 in the career branch of 0x141fe5a10 / 0x1420527d0. | new |

Data, vtables and code sites:

| address | name | kind | level | role | KB |
|---|---|---|---|---|---|
| `0x1416d2f83` | `site_Game_DecalTypeCarousel_Entry` | code site | L1 | mov rsi,[rax+0xc8]: rax = decal type record, rsi = item, *(*(item+0x30)+0x10) = texture name; rax -> zero block makes rsi 0 and takes skip path 0x1416d40e0. | new |
| `0x1416d40e0` | `site_Game_DecalTypeCarousel_SkipEntry` | code site | L1 | Skip path: in-order tree successor via rbp; taken when the item pointer is 0. | doc only |
| `0x141fe637a` | `site_Game_DecalCatalogue_DesignEntry` | code site | L2 | mov rbp,[rax+0xc8]: non-career per-design branch, same roles. | new |
| `0x141fe6d70` | `site_Game_DecalCatalogue_NoTile` | code site | L1 | No-match path (no tile built for the sponsor). | doc only |
| `0x141fe6e8e` | `site_Game_DecalCatalogue_TileSearch` | code site | L2 | mov rbx,[rax+0xc8]: sponsor tile search in the career branch; with all designs skipped no tile is built (0x141fe6d70). | new |
| `0x141fef4df` | `ret_Game_DecalCatalogue_AfterAdd` | code site | L3 | Return address of the add 0x14142dc70 call from the catalogue (new placement). | doc only |
| `0x146066bf0` | `g_FeTile_SponsorDecalCategoryKey` | data | L1 | Key (kind 0x14, id 0xffff) set at startup by 0x1417d74c0; 0x1416d2790 keeps only entries whose item +0x40 byte / +0x42 u16 equal it. | new |
| `0x146066c08` | `g_FeTile_DecalCatalogueSwapUid` | data | L1 | Catalogue selection: -1 = new placement (add), otherwise the uniqueId being swapped. | new |

Fields (proposed struct files in `re/structs/` hold the ones with concrete types):

| struct | offset | type | name | level | meaning |
|---|---|---|---|---|---|
| CustomisationItem (decal) | `0x30` | Names * | `names` | L1 | *(item+0x30)+0x10 = texture name char* (tile texture). |
| CustomisationItem (decal) | `0x38` | uint32_t | `group_id` | L2 | Sponsor decal group id matched against sponsor +0xdc. |
| CustomisationItem (decal) | `0x40` | uint8_t | `category_kind` | L1 | With +0x42 u16 compared to key 0x146066bf0. |
| CustomisationItem (decal) | `0x42` | uint16_t | `category_id` | L1 |  |
| DecalCatalogueEntry (0x141feb380) | `0x18` | bool | `has_team_record` | L1 | Lock state shown on the tile. |
| DecalCatalogueEntry / item | `0xc8` | Item * | `item` | L2 | [entry+0xc8] item at all three catalogue sites. |
| DecalCatalogueTileRow | `0x108` | Item * | `item` | L1 | Item found for the sponsor. |

## Career sponsor records, income and discovery {#career-sponsors}

A My Team sponsor is **discovered** when the player team has a record for it. Everything else (catalogue lock, income,
loyalty) follows from that record.

**CareerSponsorsComponent** (vtable `0x1456be490`; the type name string `CareerSponsorsComponent` ends just before
it; written by the constructor `0x1410f3750`):

| offset | what | evidence |
|---|---|---|
| `+0x298`/`+0x2a0` | team list, 0x88-byte `SponsorTeamRecord`s (team key `+8`, records `+0x18`/`+0x20`) | already in the KB (L2, 141106870); also read by 1410f43f0, 141feb380, 141100a80 |
| `+0x3b0` | object whose `+0x2b0` u32 is the game clock the discovered day is computed from (`(t >> 5) * 0x5b05b06 >> 32`) | 1410f43f0 |
| `+0x390`, `+0x398`, `+0x3c8`, `+0x3e0`, `+0x3c0` | passed through to the record builder 1410f5590 (goal setup 14111a560 gets `+0x3c8`; `+0x3c0` is walked at `+0x2c8` per team, 0x30 stride, for a contract link when the definition's `+0xd8` is 0) | 1410f43f0, 1410f5590 |
| `+0x4d8` | `initial_secondary_sponsors`: list object copied by the loader 14110e4d0 (decoded strings); begin `+0x4e0`, end `+0x4e8`, 0x28-byte entries with the code at `+0x10` | 14110e4d0 (Gdb field read and copy), 141100a80 |
| `+0x500`/`+0x508` | sponsor definitions, pointers; definition `+0x10` code (char *), `+0x28` u16 sponsor id, `+0xd8` flag (already in the KB as `SponsorEntry.slot`) | 1410f43f0, 141100a80, catalogue 141fe5a10 |
| `+0x520`/`+0x528` | second definition list; entries with `+0x110 == 0` are discovered for every new My Team career | 141100a80 |

**Sponsor record** (0x160 bytes; a copy of the 0x150 definition plus state). Field names from the save writer
`0x1410ffcc0` and reader `0x1410fd5f0`:

| offset | save name |
|---|---|
| `+0x28` | `sponsor_id` (u16) |
| `+0x30`, `+0x50`, `+0x70` | `teamSponsorGoals`, `…Easier`, `…Harder` |
| `+0x114` | `loyalty_xp` (in the KB) |
| `+0x118` | `contract_provider` |
| `+0x128` | `seen` (u8) |
| `+0x12a` | `discovered_day` (u16) |
| `+0x12c` | `level_up_notification_day` (u16) |
| `+0x130`, `+0x140` | `active_contract`, `next_contract` (ref-counted pointer pairs) |

<a id="discover"></a>**The discover routine** `0x1410f43f0(CareerSponsorsComponent *c, int16_t sponsor_id, uint32_t team_key)`
(proposed `Career_Sponsors_Discover`):

1. finds the team in `c+0x298` (matches byte 0 and bits 16–31 of the key);
2. finds the definition in `c+0x500` whose `+0x28` is the id, and returns if there is none;
3. returns if the team already has a record with that id. Calling it twice is harmless;
4. computes the day from `[c+0x3b0]+0x2b0` and calls `0x1410f5590` (`Career_Sponsors_Add`, already in the KB from its
   log string `Sponsor %s added with stat id %d`) with a copy of the definition. That builds the record with
   `seen = 0` and `discovered_day = day`, sets up its goals and pushes it with `0x1410fe630`, the same push the save
   reader uses;
5. if `0x1410f4270(c, team)` is true (My Team mode and the team's flag `0x10` at `+0x4b`: the player's team), posts a
   discovered-count message (vtable `0x1456c55e0`, the two list sizes) on `g_Game_MessageBus`.

Callers:

- `0x141100a80`, slot `+0xe8` of the component's vtable: when `[c+0x270]` vtable `+0xf8` returns 0 (a new career),
  for each player team it discovers every starting-list entry found by name in `c+0x500`, then every `c+0x520`
  definition with `+0x110 == 0`, then marks all records seen;
- `0x140fc7e10` (vtable `0x1456b1f60`, the My Team sponsor unlock reward): tail-jumps to it with
  `[this+0x20]` component, `[this+0x18]` team;
- `0x1423f6230`, a UI command.

**Correction to the KB:** `0x1410f85c0`, named `Career_Sponsors_Remove` from its log strings, is slot `+0x18` of the
same vtable: the component's save **reader**. It reads every team and record (record reader `0x1410fd5f0`), pushes
each record with `0x1410fe630`, and runs `0x1410f7b10` on records whose sponsor is still defined in `+0x500`. A cleanup pass in the
same function then drops records (log `Sponsor %s removed with stat id %d`). When a
team is left without a record that has `+0xd8 == 0` and `+0x158 != 0`, it assigns a new active title sponsor (log
`Active title sponsor removed … during cleanup`). The "removed" strings that named the row come from that pass. Proposed name
`Career_SponsorsComponent_Load`.

Why it matters: the starting list only reaches careers created after it changed. A plugin can call the routine once
per loaded My Team career for starting sponsors the player team lacks. Sponsor Base 1.2.0 does this; it's checked in a
harness, not yet in game.

| address | name | level | role | KB |
|---|---|---|---|---|
| `0x140739cf0` | `Game_Analytics_PlayerHasUnlockedSecondarySponsorWith` | L1 | Telemetry side effect of discovery (f1.proto.SecondarySponsorDiscovered), not the discover routine. | in KB L1 |
| `0x140ca0510` | `Career_Read_CareerDepartmentEventsConfig` | L1 | Department events reader: loops i = 1..3 snprintf('%s%d', 'outcome_modifier_', i), Gdb_Get_FieldInt on the outcome row and dereferences the result; an absent key returns &info[8] (0x08000f000800000e) -> AV at 0x140ca413b. Stock outcome_modifier_1..3 exist with empty strings. | in KB L1, adds detail |
| `0x140fc7e10` | `Career_MyTeamSponsorUnlockReward_Apply` | L2 | MyTeamSponsorUnlockReward (vtable 0x1456b1f60) method: tail jump to 0x1410f43f0 with [this+0x20] component and [this+0x18] team. | new |
| `0x1410f3750` | `Career_CareerSponsorsComponent_Ctor` | L2 | CareerSponsorsComponent constructor; writes vtable 0x1456be490 (the only rip ref of the vtable resolves to this function). | new |
| `0x1410f4270` | `Career_Sponsors_IsPlayerMyTeam` | L2 | My Team mode + team flag 0x10 at +0x4b (player team) check gating the discovered-count message. | new |
| `0x1410f43f0` | `Career_Sponsors_Discover` | L2 | void discover(CareerSponsorsComponent *c, short sponsor_id, uint team_key): team entry from c+0x298 (0x88 stride, key +8; match on byte0 + hi16), sponsor definition from c+0x500 pointer list (def +0x10 code char*, +0x28 id); returns if either is missing or the team already has a record with that id. Else day = ([c+0x3b0]+0x2b0)/day and 0x1410f5590(team, [c+0x390], [c+0x398], [c+0x3c8], [c+0x3e0], [c+0x3c0], copy of def, day) builds the record, pushed with 0x1410fe630. If 0x1410f4270 (My Team mode + team flag 0x10 at +0x4b = player team) it posts DiscoveredSecondarySponsorCount (vtable 0x1456c55e0) on g_Game_MessageBus. | new |
| `0x1410f5590` | `Career_Sponsors_Add` | L2 | Builds a team sponsor record: (team, [c+0x390], [c+0x398], [c+0x3c8], [c+0x3e0], [c+0x3c0], def copy, day); log 'Sponsor %s added with stat id %d'; seen +0x128 = 0, discovered_day +0x12a = day; goals via 0x14111a560; contract link via 0x1410f8230 when def+0xd8 == 0. | in KB L1, adds detail |
| `0x1410f5b90` | `Career_Sponsors_GetDecalsWithLocked` | L1 | Same available-decal list as 0x141107420 plus a 'locked' list of unreached levels. | new |
| `0x1410f8230` | `Career_Sponsors_LinkContract` | L1 | Contract link for a new record when def+0xd8 == 0; role from the call site only. | new |
| `0x1410f85c0` | `Career_Sponsors_Remove` | L2 | CareerSponsorsComponent save reader: slot +0x18 of vtable 0x1456be490 (0x1456be4a8). Reads every team and sponsor record (record fields via 0x1410fd5f0), pushes each record with 0x1410fe630, runs 0x1410f7b10 on records whose sponsor is still defined in +0x500, then a cleanup pass drops records (log "Sponsor %s removed with stat id %d") and assigns a new active title sponsor when a team has none left (+0xd8 == 0, +0x158 != 0). | in KB L1, correction |
| `0x1410fd5f0` | `Career_Sponsors_ReadRecordFields` | L1 | Reader of the same record fields (discovered_day etc.). | new |
| `0x1410fe630` | `Career_Sponsors_PushRecord` | L2 | Pushes a built 0x160 record into the team's vector; the same push the save reader 0x1410f85c0 uses. | new |
| `0x1410ffcc0` | `Career_Sponsors_WriteSaveData` | L1 | Save writer naming the 0x160 record fields: +0x28 sponsor_id (u16), +0x114 loyalty_xp, +0x30/+0x50/+0x70 teamSponsorGoals/Easier/Harder, +0x12c level_up_notification_day, +0x118 contract_provider, +0x130 active_contract, +0x140 next_contract, +0x128 seen, +0x12a discovered_day. | new |
| `0x141100a80` | `Career_Sponsors_InitNewCareer` | L2 | CareerSponsorsComponent vtable 0x1456be490 slot +0xe8. When [c+0x270]->vf 0xf8 == 0 (new career): for the player team discovers every c+0x4e0 starting entry (0x28 stride, code +0x10) found by name in c+0x500, then sponsors of c+0x520 with +0x110 == 0, then marks all records seen. | new |
| `0x141103de0` | `Career_Sponsors_CountUnlockedSlots` | L1 | Unlocked slots per area at the current renown minus reserved. | new |
| `0x141104190` | `Career_Sponsors_CountPaidDecals` | L1 | Weekly per-decal income count (career.my_team_sponsors.weekly_income_per_decal_thresholds). Branch A (no per-decal data) sums 0x141103de0 = unlocked slots per area at renown minus reserved; branch B counts saved livery nodes (cust+0xe8..+0xf0, 16-byte entries) minus reserved per area. Not sponsor-specific: every placed decal pays, reserved (title) slots do not. Site 0x141104491 (rsi = count, r15 = livery). | new |
| `0x141106870` | `Career_Sponsors_GainLoyalty` | L1 | On XP gain claims EVERY unclaimed level with threshold <= XP (0x141118600 per level: rewards + livery unlock; the decal is never unlocked there, it is computed live), so same-XP levels unlock together. | in KB L1, adds detail |
| `0x141107420` | `Game_CarDecals_FilterListByLod` | L1 | For one team sponsor record (SponsorEntry): available decals = all default_decals (+0xb8..+0xc0, 0x10 stride) + the decal of every loyalty level (+0x98..+0xa0, 0x78 stride) whose xp_threshold <= loyalty_xp (+0x114); returns the first eligible item (team default item). No count limit. | in KB L1, correction |
| `0x14110e4d0` | `Team_Load_MyTeamSponsors` | L2 | Also the CareerSponsorsComponent config loader: initial_secondary_sponsors (Gdb field read at 0x14110f0fb) copied into the component's +0x4d8 object, begin/end +0x4e0/+0x4e8 (0x28 stride, code char* at +0x10). Used only when a career's sponsor state is created, so it does not apply retroactively. | in KB L1, adds detail |
| `0x141114160` | `Career_Load_SponsorDefinition` | L1 | Commercial sponsor data loader: sponsor_type, lump_sum, signing_bonus, sponsor_id, tier, core_belief, backstory, sponsor, loyalty, default_decals; resolves decal_group/sponsor and validates default/loyalty decal items. Each decal name is looked up in decal_sets (call 0x1445a2bd0, strings decal_sets/DecalSet); an unknown name yields a junk pointer that is dereferenced (empty names skipped, cmp byte [rbx],0). default_decals -> uncapped vector at SponsorEntry+0xb8..+0xc0 (0x10 stride); each loyalty level (0x78 stride, +0x98..+0xa0) has ONE decal ref (+0x10 id, +0x18 present), livery ref (+0x20), rewards (+0x38). core_belief: lower-cased FNV-1a64 compare: innovation 0x2701ccff37173a0a = 1, engagement 0xed299cdab455724e = 2, youthdevelopment 0xb3fec0c5ac4c4641 = 3, anything else (incl. 'Performance') = 0. Not a generic cosmetic DecalSet loader. | new |
| `0x141118600` | `Career_Sponsors_ClaimLoyaltyLevel` | L1 | Per-level claim: rewards + livery unlock (no decal unlock). | new |
| `0x14111a560` | `Career_Sponsors_BuildGoals` | L1 | Goal setup for a new sponsor record (called by 0x1410f5590); role from the call site only. | new |
| `0x1423f6230` | `Career_TeamHqVM_DiscoverSponsorCommand` | L2 | UI/VM command caller of discover 0x1410f43f0 (only the call is established). | new |

Data, vtables and code sites:

| address | name | kind | level | role | KB |
|---|---|---|---|---|---|
| `0x140ca413b` | `crash_Career_DeptEvents_MissingOutcomeModifier` | code site | L3 | AV reading -1 when outcome_modifier_N is absent (Gdb_Get_FieldInt not-found value dereferenced). | doc only |
| `0x141104491` | `site_Career_DecalIncome_Count` | code site | L1 | mov rdx,[rbx+0x298]; at this point rsi = decal count on the saved livery, r15 = livery (nodes +0xe8/+0xf0). | new |
| `0x14110f0fb` | `site_Team_LoadSponsors_InitialSecondary` | code site | L1 | Gdb field read of initial_secondary_sponsors. | doc only |
| `0x1411154c8` | `crash_Career_LoadSponsor_UnknownDecal` | code site | L3 | AV reading 0xffffffffffffffff: dereference of the result for an unknown decal name. | doc only |
| `0x1411a6df0` | `site_Career_SponsorDiscoveryMessages_StaticInit` | code site | L1 | Static-init chunk right after this address registers f1.proto.SecondarySponsorDiscovered / DiscoveredSecondarySponsorCountUpdatedMessage (side effects, not the discover routine). | doc only |
| `0x1414011ac` | `ret_Career_SyncDriverCustomisations_Replace` | code site | L3 | Caller site of replace 0x14246fde0. | doc only |
| `0x1456b1f60` | `vt_Career_MyTeamSponsorUnlockReward` | vtable | L2 | Unlock reward whose method 0x140fc7e10 tail-jumps to discover. | new |
| `0x1456bdab8` | `k_Career_CoreValueLabels` | data | L1 | Strings near here: 4-entry label table lng_core_value_performance_excellence / _innovation_and_tech / _engagement / _youth_development (cut 4th belief). | new |
| `0x1456be490` | `vt_Career_CareerSponsorsComponent` | vtable | L2 | Type name string right before it; written by ctor 0x1410f3750; slots hold 0x141100a80 (slot +0xe8) and the save reader 0x1410f85c0. | new |
| `0x1456c55e0` | `vt_Career_DiscoveredSecondarySponsorCountMessage` | vtable | L2 | Message posted by discover 0x1410f43f0 on g_Game_MessageBus for the player My Team. | new |

Fields (proposed struct files in `re/structs/` hold the ones with concrete types):

| struct | offset | type | name | level | meaning |
|---|---|---|---|---|---|
| CareerSponsorsComponent | `0x270` | void * | `career_state` | L2 | [c+0x270]->vf 0xf8 == 0 means new career (0x141100a80). |
| CareerSponsorsComponent | `0x298` | SponsorTeamRecord * | `teams_begin` | L2 | 0x88-stride team entries. |
| CareerSponsorsComponent | `0x2a0` | SponsorTeamRecord * | `teams_end` | L2 |  |
| CareerSponsorsComponent | `0x390` | void * | `add_arg_390` | L2 | Passed to 0x1410f5590. |
| CareerSponsorsComponent | `0x398` | void * | `add_arg_398` | L2 | Passed to 0x1410f5590. |
| CareerSponsorsComponent | `0x3b0` | void * | `clock` | L2 | Day = ([c+0x3b0]+0x2b0)/day in discover. |
| CareerSponsorsComponent | `0x3c0` | void * | `add_arg_3c0` | L2 | Passed to 0x1410f5590; [c+0x3c0]+0x2c8 must be readable (checked by the Sponsor Base DLL before calling). |
| CareerSponsorsComponent | `0x3c8` | void * | `add_arg_3c8` | L2 | Passed to 0x1410f5590. |
| CareerSponsorsComponent | `0x3e0` | void * | `add_arg_3e0` | L2 | Passed to 0x1410f5590. |
| CareerSponsorsComponent | `0x4d8` | list | `initial_secondary_sponsors` | L2 | Copied by 0x14110e4d0 (Gdb field, decoded strings). |
| CareerSponsorsComponent | `0x4e0` | StartingSponsor * | `starting_begin` | L2 | 0x28 stride, code char* at +0x10. |
| CareerSponsorsComponent | `0x4e8` | StartingSponsor * | `starting_end` | L2 |  |
| CareerSponsorsComponent | `0x500` | SponsorEntry ** | `sponsor_defs_begin` | L2 | Pointer list (8-byte stride) of sponsor definitions (def +0x10 code char*, +0x28 id); also read by catalogue 0x141fe5a10. |
| CareerSponsorsComponent | `0x508` | SponsorEntry ** | `sponsor_defs_end` | L2 |  |
| CareerSponsorsComponent | `0x520` | list | `sponsors_520` | L2 | Sponsors with +0x110 == 0 are discovered for a new career after the starting list. |
| SponsorEntry | `0x10` | const char * | `code` | L2 | Sponsor code C string (also on definitions in c+0x500). |
| SponsorEntry | `0x28` | uint16_t | `sponsor_id` | L1 | Career sponsor_id (save name). |
| SponsorEntry | `0x30` | ? | `teamSponsorGoals` | L1 | Save name. |
| SponsorEntry | `0x50` | ? | `teamSponsorGoalsEasier` | L1 | Save name. |
| SponsorEntry | `0x70` | ? | `teamSponsorGoalsHarder` | L1 | Save name. |
| SponsorEntry | `0x98` | SponsorLoyaltyLevel * | `levels_begin` | L1 | 0x78 stride. |
| SponsorEntry | `0xb8` | DecalSetRef * | `default_decals_begin` | L1 | Uncapped vector, 0x10 stride (loader 0x141114160; read by 0x141107420). |
| SponsorEntry | `0xc0` | DecalSetRef * | `default_decals_end` | L1 |  |
| SponsorEntry | `0xd8` | uint8_t | `slot` | L1 | Tested by 0x141ff78c0; def+0xd8 == 0 -> contract link in 0x1410f5590. |
| SponsorEntry | `0xdc` | uint32_t | `group_id` | L1 | Decal group id (catalogue searches item +0x38 == it); compared with vehicle customisation +0x78 in 0x141ff7610. |
| SponsorEntry | `0x110` | ? | `flag_110` | L2 | Entries of c+0x520 with +0x110 == 0 are discovered for a new career (struct of those entries not confirmed to be SponsorEntry). |
| SponsorEntry | `0x114` | int32_t | `loyalty_xp` | L1 | Save name loyalty_xp; compared with level thresholds. |
| SponsorEntry | `0x118` | ? | `contract_provider` | L1 | Save name. |
| SponsorEntry | `0x128` | uint8_t? | `seen` | L2 | 0 after discover (like a reward discovery). |
| SponsorEntry | `0x12a` | uint16_t? | `discovered_day` | L2 | Set to the current day by 0x1410f5590. |
| SponsorEntry | `0x12c` | ? | `level_up_notification_day` | L1 | Save name. |
| SponsorEntry | `0x130` | ? | `active_contract` | L1 | Save name. |
| SponsorEntry | `0x140` | ? | `next_contract` | L1 | Save name. |
| SponsorEntry | `0x158` | uint8_t | `active` | L1 | Tested by 0x141ff78c0. |
| SponsorLoyaltyLevel | `0x4` | int32_t | `xp_threshold` | L1 | Level unlocks when <= loyalty_xp; level index + 1 = loyalty_required shown in the carousel. |
| SponsorLoyaltyLevel | `0x10` | uint32_t | `decal_id` | L1 | ONE decal ref per level (a comma list is one unknown name -> loader crash). |
| SponsorLoyaltyLevel | `0x18` | uint8_t | `decal_present` | L1 |  |
| SponsorLoyaltyLevel | `0x20` | ref | `livery` | L1 | Livery ref. |
| SponsorLoyaltyLevel | `0x38` | list | `rewards` | L1 | Reward definitions (OneShot/Morale/Renown/ResourcePoints/RndDiscount/SponsorDiscovery/SponsorUnlock/Oxp...); none unlocks a decal. |
| SponsorTeamRecord | `0x8` | uint32_t | `team` | L2 | Team key; matched on byte0 + hi16 (mask 0xffff00ff). |
| SponsorTeamRecord | `0x18` | SponsorEntry * | `sponsors_begin` | L2 | 0x160 records. |
| SponsorTeamRecord | `0x20` | SponsorEntry * | `sponsors_end` | L2 |  |
| SponsorTeamRecord? | `0x4b` | uint8_t | `flags (0x10 = player team)` | L2 | Tested by 0x1410f4270; the object (team entry vs other) is not stated precisely in the contributor's notes. |

## Engine supplier history {#engine-suppliers}

`EngineSuppliersComponent` `+0xe30`/`+0xe38` is `engineSupplierHistory`: 0x28-byte team entries, with the
team key at `+0` and a history at `+0x10`/`+0x18` of `{u16 day, u8 engine stat id}`. The newest entry is the team's
current supplier, and reading it gave the right engine in game.

Don't look the component up from UI or editor code. Doing it inside the catalogue build on the VM thread caused a
softlock and then a crash with an empty row list (L1 cause). Look it up on the flow thread and cache it. The
`component("EngineSuppliersComponent")` lookup itself cost 13–30 ms per call.

| address | name | level | role | KB |
|---|---|---|---|---|
| `0x141223130` | `Career_EngineSuppliers_WriteSaveData` | L1 | EngineSuppliersComponent save writer; field names give engineSupplierHistory at +0xe30..+0xe38 (0x28-byte entries). | new |
| `0x141223190` | `Career_EngineSuppliers_ReadSaveData` | L1 | Reader using the same field names. | new |
| `0x1412972e0` | `Career_EngineSuppliersComponent_GetType` | L1 | GetType stub of EngineSuppliersComponent (vtable 0x1456cbd40). | new |

Data, vtables and code sites:

| address | name | kind | level | role | KB |
|---|---|---|---|---|---|
| `0x1456cbd40` | `vt_Career_EngineSuppliersComponent` | vtable | L1 | EngineSuppliersComponent vtable; GetType stub 0x1412972e0. | new |
| `0x146063140` | `k_Career_EngineSuppliersComponentDesc` | data | L2 | Resolved by the career-editor components helper; layout added in struct_fields (EngineSuppliersComponent +0xe30). | in KB |

Fields (proposed struct files in `re/structs/` hold the ones with concrete types):

| struct | offset | type | name | level | meaning |
|---|---|---|---|---|---|
| EngineSuppliersComponent | `0xe30` | EngineHistoryTeam * | `engineSupplierHistory_begin` | L3 | 0x28-byte entries; +0 team key (byte kind, +2 u16 teamStatId; compare masked 0xffff00ff), +0x10/+0x18 history vector of 4-byte {u16 day, u8 engineStatId at +2} (entrants.engines.stat_id); current = newest day. Runtime: the Sponsor Base DLL read 'team engine 33' and filtering confirmed by owner. |
| EngineSuppliersComponent | `0xe38` | EngineHistoryTeam * | `engineSupplierHistory_end` | L3 |  |

## The team badge and its emblem {#badge}

The team badge is held by `game::playerCustomisationInfoSystem`, a registered system:

- **Registration:** descriptor `0x146064e78`, vtable `0x1456e4f20` (slot 0 returns the descriptor), and a singleton
  at `0x146064cb8` that the constructor sets and the destructor clears.
- **Badge records:** in its entrants system `[X+0x280]`, list `+0x3b8+kind*0x10`, the same 16-byte entry layout as the
  vehicle store.
- **Record fields:** each record keeps its own key at `+0x160`. The emblem is `(BadgeEmblem.id << 16) | 8` at `+0xb0`,
  so `+0xb2` is the id: the database `id`, not the `statId`.
- **Replacing a badge:** `0x1424710e0` inserts a new record on every edit, so a record pointer is never stable.
- **The current badge key:** the front end keeps it in `0x146066c18`. The My Team overview's init `0x1420527d0` writes it
  together with the decal editor's livery key at `0x146066be0`. The badge editor's apply `0x142074fd0` reads it.

| address | name | level | role | KB |
|---|---|---|---|---|
| `0x140cddf70` | `Ent_EntrantsSystem_InsertBadgeRecord` | L2 | Inserts the new badge record for 0x1424710e0. | new |
| `0x1414399d0` | `Game_PlayerCustomisationInfoSystem_WriteSaveData` | L2 | Save writer naming fields: +0x1b0 standard_driver, +0x1b4 standard_driver_customisation, +0x1b8 standard_badge, +0x1d0.. standard_vehicle_customisations hash map, +0x120 list head / +0x128 count of assigned_badge_customisation (see struct). | new |
| `0x14143f390` | `Game_PlayerCustomisationInfoSystem_Ctor` | L2 | Constructor: sets vtable 0x1456e4f20 and stores the singleton at 0x146064cb8. | new |
| `0x141443700` | `Game_PlayerCustomisationInfoSystem_GetAssignedBadge` | L1 | Badge getter (X, out, game mode, save slot); .data1 thunk 0x14ac9f2e0 (obfuscated). Callers pass game mode [career+0x2a8] and save slot **([career-subobject]+0x280). | new |
| `0x141446bb0` | `Game_PlayerCustomisationInfoSystem_OnBadgeDeleted` | L2 | Badge-deleted handler: walks assigned_badge_customisation (list + tree) and reads [X+0x280]+0x3c8 (entrants kind-1 badge list) and record +0x160. | new |
| `0x1414559a0` | `Game_PlayerCustomisationInfoSystem_GetType` | L2 | vtable 0x1456e4f20 slot 0: `lea rax,[0x146064e78]; ret` (asserted by prepare_signatures.py). | new |
| `0x1414559b0` | `Game_PlayerCustomisationInfoSystem_Dtor` | L2 | Destructor: resets vtable, clears singleton 0x146064cb8. | new |
| `0x141468840` | `Game_Load_DefaultBadgeEmblem` | L2 | default_badge_emblem loader: finds the row in customisation.custom_player_badges.emblems.BadgeEmblem and builds (id << 16) \| 8 from field id (stat_id only if the table has no id key; missing row -> 0xffff0008). | new |
| `0x1414751e0` | `Game_StaticInit_CustomisationSystemDescriptors` | L2 | Static init writing the type descriptors of GameDecalCustomisationSystem 0x146064d10 and playerCustomisationInfoSystem 0x146064e78 (same pattern as CustomPaintFinishesSystem 0x146064cf0). | new |
| `0x141d77e70` | `Game_RenderBadgeViewModel_SetupBadgeData` | L2 | Passes BadgeInfo +0xa8 emblem object to the emblem item provider. | in KB L1, adds detail |
| `0x141fc5b80` | `Game_F1WorldBadgeSlotEditor_A` | L2 | F1 World badge slot editor; points 0x146066c18 at other badges (one of 17 functions with 25 refs). 0x141fc8ea0 and 0x141fce280 are its twins. | new |
| `0x1420527d0` | `Game_CustomisationMyTeamOverviewVM_Init` | L2 | CustomisationMyTeamOverviewViewModel init (vtable slot 0x145766728). Career branch (0x14b52b6e0() == 2): 0x146066c18 = getter(X, game mode, slot) and 0x146066be0 = assigned vehicle customisation (decal editor livery key) together. Mode-3 branch copies 0x14606d42c when +0x2e9 is 0. | new |
| `0x1420567a0` | `Game_MyTeamSetup_Complete` | L2 | Setup completion: assigns the setup badge (MyTeamSetupData 0x14606d340 +0xec = 0x14606d42c) or creates a default badge (0x1424717e0) into assigned_badge_customisation[gm][slot] and sets 0x146066c18; also reads record +0x160. | new |
| `0x142063ad0` | `Game_CustomisationMyTeamOverviewVM_CarLiveriesTab` | L1 | Overview car_liveries tab from which the team_hq decal editor is entered. | new |
| `0x142074fd0` | `Game_BadgeEditor_Apply` | L2 | Badge editor apply: reads 0x146066c18, looks up the record, replaces it under the SAME key (0x1424710e0); reads BadgeInfo +0x98/+0xa0/+0x18..+0x90. Runs from 0x14207bd20 on the confirm message and on one value-change path. | new |
| `0x14207bd20` | `Game_BadgeEditor_OnConfirm` | L2 | Confirm message handler: calls 0x142074fd0, then +0x16d8 = 1, exit. | new |
| `0x14246d820` | `Ent_Get_Badge_Thunk` | L2 | Thunk -> 0x14caf0800(entrants, &key): kind = key byte0 (<3), index = key >> 16, list [entrants+0x3b8+kind*0x10] (+8 begin, +0x10 end, 16-byte entries, record [entry+8]); log 'Badge with stat id %s not found'. | new |
| `0x142470be0` | `Ent_EntrantsSystem_RemoveBadge` | L2 | Called from 0x141a20850 with the same entrants object as vehicle-customisation remove. | in KB L1 |
| `0x1424710e0` | `Ent_EntrantsSystem_ReplaceBadge` | L2 | (entrants, key, info): kind 0 refused ('preloaded'); equal record -> 4 (no-op); otherwise removes the old entry and inserts a NEW record (0x140cddf70) with +0x160 = key, so the record pointer changes on every edit. | in KB L1, adds detail |
| `0x1424717e0` | `Ent_EntrantsSystem_AddBadge` | L2 | Creates the default badge in My Team setup completion 0x1420567a0. | in KB L1 |
| `0x14248c9e0` | `Game_Write_BadgeInfoJson` | L2 | Badge writer: writes u16 +0xb2 as emblem_id. | new |
| `0x14248d820` | `Game_Read_BadgeInfoJson` | L2 | Badge JSON reader: stores emblem_id at BadgeInfo +0xb0 as (u16 << 16) \| 8. | new |

Data, vtables and code sites:

| address | name | kind | level | role | KB |
|---|---|---|---|---|---|
| `0x14568dd80` | `vt_Game_BadgeEmblemInfo` | vtable | L2 | Vtable of the BadgeInfo +0xa8 emblem sub-object. | new |
| `0x1456e4f20` | `vt_Game_PlayerCustomisationInfoSystem` | vtable | L2 | Vtable; slot 0 GetType 0x1414559a0; set only by ctor 0x14143f390 and dtor 0x1414559b0. | new |
| `0x146064cb8` | `g_Game_PlayerCustomisationInfoSystem` | data | L2 | Singleton pointer stored by ctor 0x14143f390, cleared by dtor. | new |
| `0x146064e78` | `k_Game_PlayerCustomisationInfoSystemDesc` | data | L2 | Type descriptor of game::playerCustomisationInfoSystem (name 0x1456e4ef8, hash 0xd93d94d5, parent 0x146cb8538), written in 0x1414751e0; 94 rip refs, 92 are registry probes. | new |
| `0x146066be0` | `g_FeTile_AssignedVehicleCustomisationKey` | data | L2 | Assigned vehicle customisation key = the decal editor's livery key; written with 0x146066c18 by 0x1420527d0; read by 0x1416cb9b0. | new |
| `0x146066c18` | `g_FeTile_CurrentBadgeKey` | data | L2 | u32 FE current-badge key: written by 0x1420527d0 (career branch, with 0x146066be0) and 0x1420567a0; read by badge apply 0x142074fd0; 25 refs in 17 functions (also F1 World badge slot editors 0x141fc5b80/0x141fc8ea0/0x141fce280). Deletion paths reset it to 0x3fff0000. prepare_signatures asserts writers/reader reference it and it shares the descriptor's section. | new |
| `0x14606d340` | `g_Game_MyTeamSetupData` | data | L2 | MyTeamSetupData static; +0xec badge key (= 0x14606d42c); lazily initialised. | new |
| `0x14606d42c` | `g_Game_MyTeamSetupBadgeKey` | data | L2 | Setup badge key (MyTeamSetupData+0xec); copied by the overview mode-3 branch when +0x2e9 is 0. | new |
| `0x14c6645f8` | `site_Game_MyTeamSetup_DriverCustomisationWriter` | code site | L3 | .data1 writer of new-setup driver customisation keys 0x20001 / 0xa0001 (via 0x14c71d4a4 / 0x14c6d4b5a <- 0x1442c02fb <- 0x14398c32c). | doc only |

Fields (proposed struct files in `re/structs/` hold the ones with concrete types):

| struct | offset | type | name | level | meaning |
|---|---|---|---|---|---|
| BadgeInfo | `0x18` | ? | `fields_18_90` | L2 | Read by badge apply 0x142074fd0 (+0x18..+0x90). |
| BadgeInfo | `0x98` | string | `loadout_name` | L2 | +0x98/+0xa0. |
| BadgeInfo | `0xa8` | object | `emblem` | L2 | Emblem sub-object (vtable 0x14568dd80); passed to the emblem item provider by setupBadgeData. |
| BadgeInfo | `0xb0` | uint32_t | `emblem_key` | L2 | (BadgeEmblem.id << 16) \| 8. |
| BadgeInfo | `0xb2` | uint16_t | `emblem_id` | L2 | BadgeEmblem.id (not statId). |
| BadgeInfo | `0xb8` | float | `emblem_rotation` | L2 |  |
| BadgeInfo | `0xbc` | float | `emblem_scale` | L2 |  |
| BadgeInfo | `0xc0` | ? | `emblem_colours` | L2 | +0xc0..+0xc8. |
| BadgeInfo | `0xd0` | ? | `emblem_style` | L2 |  |
| BadgeInfo | `0x110` | uint32_t | `background_key` | L2 | (id << 16) \| 7. |
| BadgeInfo | `0x118` | ? | `background_colours` | L2 |  |
| BadgeInfo | `0x128` | ? | `background_style` | L2 |  |
| BadgeInfo | `0x160` | uint32_t | `stat_id` | L2 | Own key of the stored record (set by replace 0x1424710e0). |
| CustomisationMyTeamOverviewViewModel | `0x2e9` | uint8_t | `flag_2e9` | L2 | Mode-3 branch copies 0x14606d42c when 0; meaning not traced. |
| MyTeamSetupData (0x14606d340) | `0xec` | uint32_t | `badge_key` | L2 | Setup badge key (absolute 0x14606d42c). |
| playerCustomisationInfoSystem | `0x120` | list | `assigned_badge_customisation_head` | L2 | List nodes {+0 next, +8 prev, +0x10 game_mode_stat_id, +0x20 tree head, +0x28 tree count}; tree nodes {+0 left, +8 parent, +0x10 right, +0x19 isnil, +0x20 save_slot_id u64, +0x28 badge key u32}. |
| playerCustomisationInfoSystem | `0x128` | uint64_t | `assigned_badge_customisation_count` | L2 |  |
| playerCustomisationInfoSystem | `0x1b0` | ? | `standard_driver` | L2 | Save name. |
| playerCustomisationInfoSystem | `0x1b4` | ? | `standard_driver_customisation` | L2 | Save name. |
| playerCustomisationInfoSystem | `0x1b8` | ? | `standard_badge` | L2 | Save name. |
| playerCustomisationInfoSystem | `0x1d0` | hash map | `standard_vehicle_customisations` | L2 | Save name. |
| playerCustomisationInfoSystem | `0x280` | EntrantsSystem * | `entrants` | L2 | Badge record store (L2 for the badge-deleted use). |

## Decal rendering and the per-decal colour slot {#decal-rendering}

The decal node's colour slot `+0x18` is saved (`Game_DecalNode_Serialize`) and networked, but nothing in the
UI sets it. On the GPU side, `CustomisationDecals::DecalData` (stride 128, shader reflection) carries
`packedAlbedoColour` at `+0x68`; the bake computes `lerp(texture.rgb, colour.rgb, colour.a)`. How editor nodes become
that buffer isn't found yet.

In T014 the renderer's decal-set proxy message handler `0x143c772f0` saw only message 6. Each one carried null objects
and a generation counter at `+0x28`, so it's a re-bake trigger, not decal data. A write test (T016) is prepared but
hasn't run.

Car decal pixels use a fixed roughness of 0.0977 and metallic 0 in the paint shader, whatever the livery finish.

| address | name | level | role | KB |
|---|---|---|---|---|
| `0x143c60480` | `Render_Bounds_InitFromAabb` | L1 | Negative result only: fingerprint false positive (our label 'area geometry'). | in KB L1 |
| `0x143c65870` | `Render_CustomisationDecals_Teardown` | L1 | Teardown; NOT the writer of the bake buffer at renderer object +0x218. | new |
| `0x143c6e260` | `Ne_CustomisationDecalRenderer_Render` | L1 | Negative result only: hit by our fingerprint scan for the node -> GPU DecalData writer and rejected as a false positive (our note labelled it 'skinning'). | in KB L2 |
| `0x143c70c60` | `Ne_CustomisationDecalRenderer_BakePasses` | L1 | Negative result only: fingerprint false positive (our label 'render graph'). | in KB L2 |
| `0x143c772f0` | `Render_CustomisationDecalsSet_OnProxyMessage` | L3 | NeCustomisationDecalsSet connection message handler for PROXY_MESSAGE_ID_BIND_CUSTOMISATION_DECALS_SET (vtable 0x1458e7c60 slot 1; rcx=connection, rdx=&msg id, r8=payload; msg 6: +0 flags, +8/+0x18 objects with owners +0x10/+0x20, +0x28 qword; msg 5 swaps target+0x88 entries). Runtime: all 224 observed messages were msg 6 with null objects and +0x28 a generation counter (1..0x1c1), flags 0 or 2: a re-bake trigger, not a carrier of per-decal records; msg 5 never seen. Dispatcher return 0x143b870dd. | new |
| `0x143c77e30` | `Render_CustomisationDecalsSet_Build` | L1 | Builds the decal-set object at connection+0x18 (0x140 bytes; see struct CustomisationDecalsSet). | new |

Data, vtables and code sites:

| address | name | kind | level | role | KB |
|---|---|---|---|---|---|
| `0x143b870dd` | `ret_Render_ProxyMessageDispatch` | code site | L3 | Dispatcher return address for the decal-set proxy messages. | doc only |
| `0x1458e7c60` | `vt_Render_NeCustomisationDecalsSetConnection` | vtable | L1 | Slot 1 = message handler 0x143c772f0 (hooked and firing in T014). | new |

Fields (proposed struct files in `re/structs/` hold the ones with concrete types):

| struct | offset | type | name | level | meaning |
|---|---|---|---|---|---|
| CustomisationDecalsSet (connection+0x18, 0x140) | `0x0` | ? | `matrices` | L3 | Two identity 3x4 matrices (+0..+0x5f). |
| CustomisationDecalsSet (connection+0x18, 0x140) | `0x60` | ? | `uv_scale?` | L3 | (0.25, 0.25). |
| CustomisationDecalsSet (connection+0x18, 0x140) | `0x88` | ? | `objects` | L3 | Vector of 14 pointers (constant). |
| CustomisationDecalsSet (connection+0x18, 0x140) | `0x98` | ? | `count?` | L3 | Small int 6/9/0x13 changing. |
| CustomisationDecalsSet (connection+0x18, 0x140) | `0xa0` | ? | `bound_objects` | L3 | +0xa0..+0xc0 null. |
| CustomisationDecalsSet (connection+0x18, 0x140) | `0xe8` | ? | `flags` | L3 | Flag bytes. |
| DecalRendererObject | `0x218` | void * | `bake_buffer_holder` | L1 | Bake binds a buffer from [+0x218]+8 -> +0x80; writer unknown. |
| GPU CustomisationDecals::DecalData (shader, stride 128) | `0x60` | uint32_t | `packedDecalFlags` | L1 | Bit 8 = draw, bits 16-23 = projection area. Shader-side (customisation_decals.nefx2 reflection), not exe memory. |
| GPU CustomisationDecals::DecalData (shader, stride 128) | `0x64` | uint32_t | `albedoTextureHeapIndex` | L1 |  |
| GPU CustomisationDecals::DecalData (shader, stride 128) | `0x68` | uint32_t | `packedAlbedoColour` | L1 | RGBA8, R low byte; bake rgb = lerp(texture.rgb, colour.rgb, colour.a). |
| ProxyMsg6 (BIND_CUSTOMISATION_DECALS_SET) | `0x28` | uint64_t | `generation` | L3 | Generation counter (1..0x1c1); +0 flags 0/2; +8/+0x18 objects (owners +0x10/+0x20) always null. |

## Teamwear (suits and gloves) {#teamwear}

`Suit` and `Gloves` rows have no brand field. `customisation_brands` (15 brands, Alpinestars, Puma and Sparco
among them) is used only by casual wear. The suit screen is already tabbed natively (`0x14204cce0`, tabs through
`0x1417ee390`), and so is the gloves screen (`0x14202f0d0`).

In T015 the equip functions `0x14204f900` and `0x1420310b0` were never called on the Race Gear or My Team setup paths.
In a running career, `0x141401110` copies the livery decals into the driver customisation records (keys `0x1`,
`0x10001`, `0x30001`, `0x40001`) through `0x14246fde0`. The record's slot lists sit at `+0x340`, `+0x1b60` (gloves) and
`+0x3380` (suits), with counts at `+0x1b50`, `+0x3370` and `+0x4b90`.

| address | name | level | role | KB |
|---|---|---|---|---|
| `0x1413f9120` | `Game_Load_CustomisationBrands` | L1 | Loader of customisation.customisation_brands.brands (15 brands; used by casual wear only). | new |
| `0x141401110` | `Career_MyTeam_SyncDriverCustomisationsFromLivery` | L3 | Builds driver customisation records from the vehicle-customisation key + driver key, copying up to 5 livery decal entries into the driver record (0x141401480 / 0x141401620; suit logos follow the car livery), then replace 0x14246fde0 for keys 0x1, 0x10001, 0x30001, 0x40001. Called from 0x14144b722; upstream callers 0x1436cfdb3, 0x141a49c7a, 0x141a49396 and 0x14142da2a (decal editor writeback). | new |
| `0x141401dd0` | `Career_OwnerSetup_ApplySuit` | L1 | My Team branch after suit equip, via Career OwnerSetupComponent (+0x348 map, +0x370 list). | new |
| `0x1414059c0` | `Game_Build_CustomisationCatalogue` | L1 | Suit/Gloves part reads only asset_group, id, nb_colours, name_loc_tag, material_override, logos_model(_override_pattern), apply_non_customisable_colours, outfit (no brand field). Livery part keeps available_paint_finishes (item+0x48) and reads optional colour_%u_default (item+0x34+4i) and finish_%u_default (enum byte at item+0x50+i), u = 0..3. | in KB L2, adds detail |
| `0x14148a320` | `Game_Register_DriverSuitScreens` | L1 | Only referrer of the screen names customisation_driver_suit / customisation_driver_gloves. | new |
| `0x14163dcc0` | `Game_Register_MyTeamSetupGenericScreen` | L1 | Refers to customisation_my_team_setup_generic (also 0x144bddbb0). | new |
| `0x1417ee390` | `Ui_Tabs_AddTab` | L1 | Generic tab helper (tab, items, ctx, name, loctag, 0, index). | new |
| `0x1418881e0` | `Game_CustomisationOptionVM_Fill` | L1 | Shared option model writing origin_type, has_brand, brand_icon, show_dlc_car_category, show_default_car_category. | new |
| `0x141fd6750` | `Game_CustomisationVM_PopulateOptions` | L1 | Used by 0x14204eb10 with suits/lng_suits. | new |
| `0x14202f0d0` | `Game_DriverGlovesVM_BuildTabs` | L1 | Gloves tab builder: gloves/lng_gloves + paddock_items. | new |
| `0x1420310b0` | `Game_DriverGlovesVM_EquipGloves` | L1 | Gloves twin of 0x14204f900 (uses list B apply 0x142496870). Runtime: 0 calls in T015 run1. | new |
| `0x14204cce0` | `Game_DriverSuitVM_BuildTabs` | L1 | Suit screen tab builder: two fixed tabs through 0x1417ee390: suits/lng_suits (index 0) and paddock_items/lng_paddock_items (index 1). Byte-identical twin of the gloves builder (no unique signature). | new |
| `0x14204eb10` | `Game_DriverSuitVM_Populate` | L1 | Suit populate (uses 0x141fd6750 with suits/lng_suits); tab split not traced. | new |
| `0x14204f900` | `Game_DriverSuitVM_EquipSuit` | L1 | (screen, item): key from global 0x146066bdc, system [screen+0x2d0], copy 0x14248efc0, apply 0x142496950, store 0x14246fde0; item +0x10 type byte selects g_FeTile_CustomisationSystem+0x38+type*0x10 provider; in My Team then 0x141401dd0([screen+0x320], ...) via Career OwnerSetupComponent (+0x348 map, +0x370 list). Runtime: never called on the Race Gear path or the My Team setup teamwear step. Gloves twin 0x1420310b0. | new |
| `0x142466a10` | `Wear_Load_DriverCustomisations` | L1 | (system, kind) fills the +0x290/+0x298 pair. | in KB L2 |
| `0x14246d940` | `Wear_Get_DriverCustomisation_Thunk` | L1 | Thunk -> 0x14caf39c0(system, &key): key byte0 kind (<3), u16 at +2 index; list [system+0x298+kind*0x10] (+8 begin, +0x10 end, 16-byte entries, value +8). Same ENTRANTS_SYSTEM as vehicle liveries (+0x388) and badges (+0x3b8); Fixed Sponsors reaches it as [decal system+0x258]. | in KB L2 |
| `0x14246fde0` | `Ent_EntrantsSystem_ReplaceDriverCustomisation` | L1 | (system, key, record): refuses kind 0 (preloaded). Runtime: 60 calls in a My Team session; running career stores keys 0x1 (twice), 0x10001, 0x30001, 0x40001 together from one record (caller 0x1414011ac in 0x141401110); new setup key 0x20001 (caller 0x14144b163 <- 0x1437003e8) then 0xa0001 via .data1 caller 0x14c6645f8. | in KB L1, adds detail |
| `0x14248efc0` | `Wear_Copy_DriverCustomisationRecord` | L1 | Driver customisation record copy: three 0x30-byte slot lists A +0x340 (count +0x1b50, apply 0x142496790, probably helmet), B +0x1b60 (count +0x3370, gloves, 0x142496870), C +0x3380 (count +0x4b90, suits, 0x142496950); head = three 4-colour ARGB groups each followed by a pointer; name string at +0xc8. Supersedes the earlier static note (+0x10 slots, count +0x1810). | new |
| `0x142496950` | `Wear_Apply_SuitSlot` | L1 | (record, resolved, item): replaces the slot (list C, suits) whose key matches resolved+0x18, else appends. Twins 0x142496790 (list A) and 0x142496870 (list B, gloves) are byte-identical. | new |
| `0x14276fda0` | `Ent_Parse_TeamSetupRow` | L1 | Also reads `suit` from the My Team setup row. | in KB L2 |
| `0x142ead3f0` | `Game_Load_UnlockableItems` | L1 | Handles unlockable_suit/gloves/tops/... (type list, not UI). | in KB L1 |

Data, vtables and code sites:

| address | name | kind | level | role | KB |
|---|---|---|---|---|---|
| `0x14142da2a` | `site_Game_DecalWriteback_DriverCustomisationSync` | code site | L3 | Inside writeback; appears in backtraces of 0x14246fde0 via 0x141401110 (suit logos follow the livery). | doc only |
| `0x146066bdc` | `g_FeTile_DriverCustomisationKey` | data | L1 | Key used by suit equip 0x14204f900. | new |

Fields (proposed struct files in `re/structs/` hold the ones with concrete types):

| struct | offset | type | name | level | meaning |
|---|---|---|---|---|---|
| DriverCustomisationRecord | `0x20` | ? | `colour_groups` | L3 | Three 4-colour ARGB groups each followed by a pointer (+0x20+0x30*i). |
| DriverCustomisationRecord | `0xc8` | ? | `name` | L3 | Name string ('Design 1' / 'Team Name'). |
| DriverCustomisationRecord | `0x340` | ? | `slots_a` | L1 | 0x30-byte slots, probably helmet. |
| DriverCustomisationRecord | `0x1b50` | ? | `slots_a_count` | L1 |  |
| DriverCustomisationRecord | `0x1b60` | ? | `slots_b` | L1 | Gloves. |
| DriverCustomisationRecord | `0x3370` | ? | `slots_b_count` | L1 |  |
| DriverCustomisationRecord | `0x3380` | ? | `slots_c` | L1 | Suits. |
| DriverCustomisationRecord | `0x4b90` | ? | `slots_c_count` | L1 |  |
| SuitScreen (0x14204f900) | `0x2d0` | EntrantsSystem * | `system` | L1 |  |
| SuitScreen (0x14204f900) | `0x320` | OwnerSetupComponent * | `owner_setup` | L1 | OwnerSetupComponent +0x348 map, +0x370 list. |
| g_FeTile_CustomisationSystem | `0x38` | Provider[] | `item_providers` | L1 | +0x38+type*0x10 provider selected by item +0x10 type byte (suit equip). |

## Seen in game {#runtime}

| session | what was seen |
|---|---|
| T013 run1 (2026-10-30, session 20261030-133314) | Passive add/writeback hooks: node item key = (DecalSet.id<<16)\|9 (595 monomic_n, 301 euterpe_n, 661 techpace_n), +0x18 = 0xffffffff, area = ProjectionArea.id (2026 car 21..38). Add template uid -1 (allocated max+1: 12..15), +0x30 translation, +0x50 scale, +0x60 0, +0x68 -1. One left_nose add produced two list nodes (right_nose partner) from one add call. Writeback after each add (ret 0x141fed3de) and on editor exit (ret 0x141ff694a); editor key sel = 0x110001 constant. Add returns to 0x141fef4df. |
| T014 run1 (2026-11-03, sessions 20261103-085839/-094519/-112501) | PARTIAL. One add: key 0x12d0009 (euterpe_n), area 0x17, colour 0xffffffff, ret 0x141fef4df. 224 decal-set messages, all msg 6 with null payload objects and a generation counter at +0x28 (re-bake trigger, not decal data); msg 5 never seen; dispatcher ret 0x143b870dd. Set object contents logged. Writeback hook rejected because Sponsor Base's sponsor_base.dll already hooks 0x14142d800. |
| T015 run1 (2026-11-01, session 20261101-185534) | Suit/gloves equip 0x14204f900/0x1420310b0 never called on Race Gear or My Team setup paths. 0x14246fde0: running career stores keys 0x1, 0x10001, 0x30001, 0x40001 from one record via 0x141401110 (also after decal writeback 0x14142da2a); new setup 0x20001 then 0xa0001 via .data1 0x14c6645f8. Record head = three 4-colour ARGB groups + pointers, name at +0xc8. Configured slot offsets were wrong (corrected to +0x340/+0x1b60/+0x3380). |
| Fixed Sponsors 0.2.0 (session 20261030-190848, the contributor's log FS001 run3; owner confirmation) | Copy hook 0x14142bd50 fires on editor load; nodes added through 0x14142dc70 appear on the car. Owner confirmed: Swap and Delete greyed with the reserved message on the locked decal when the Sponsor Base DLL raised the reserved count at sites 0x141ff80b0/0x141ff81eb; locked Monomic did not change the income figure with the Sponsor Base DLL subtracting at 0x141104491 (Euterpe did). |
| Fixed Sponsors 0.9.1 diagnostics (session 20261101-122006) | One decal system object for the whole session, vtable image offset +0x56e3b50 stable; editor keys F1 World 0x00100001 and My Team 0x00110001, team 0x00050001; editor load/save hooks and the frame tick run on game worker threads. EngineSuppliersComponent history read gave engine 33 (RedBull_Ford_2026). Carousel site logged 'honda hidden' but the user still saw the Honda tile -> tiles come from 0x141fe5a10. Captured 2026 positions logged (e.g. FIA 764 areas 23/24 t=(+-0.234,0.18,1.587)). |
| Owner in-game confirmation 2026-11-01 (Sponsor Studio 0.9.4) | Engine-supplier logos only appear in the My Team catalogue for the matching engine (catalogue tile sites 0x141fe6e8e / 0x141fe637a work). |
| Owner in-game confirmation 2026-11-02 (Sponsor Studio 0.10.0, screenshot) | Own <code>_list_da referenced by sponsor and DecalGroup texture_name is shown as the catalogue grid tile (texture via item+0x30 -> +0x10). |
| Session 20261102-132712 (loader.log, perf.log) | Tile texture of a sponsor with a list tile = DecalGroup.texture_name (locked Pirelli not hidden until group texture added). api->career->component('EngineSuppliersComponent') costs 13-30 ms per call. |
| Crash sessions 20261030-212950/-213736/-215348 | AV reading 0xffffffffffffffff at F1_25.exe+0x11154c8 in sponsor loader 0x141114160 when a loyalty decal field held a comma list (unknown decal name). |
| Crash session 20261101-114324 | AV reading -1 at F1_25.exe+0xca413b in 0x140ca0510 during Game_ModeChampionship_Build when cloned Outcome rows lacked outcome_modifier_1..3. |
| Crash sessions 20261030-233300/-235303/20261031-023337 | Softlock then AV at 0x142ef58b0+0x46 reading 0xb8 from 0x141fec530+0x114 with an empty editor row list (rdx=rdi=0xb0), after the Sponsor Base DLL called the career component registry from inside the carousel build on the VM thread (cause L1). |
| Owner observation (2026-10-30) | The reserved side slot belongs to the current title sponsor (owner-observed behaviour, not an agent trace). |
| T013 run1 crash report | Shutdown AV at F1_25.exe+0x537db28 on launcher Stop is pre-existing (~29 sessions incl. tracer-free). |

## Corrections to KB rows {#corrections}

- **`0x1410f85c0` `Career_Sponsors_Remove`** is the `CareerSponsorsComponent` save reader, slot `+0x18` of
  `0x1456be490` (proposed `Career_SponsorsComponent_Load`). Its "removed" log strings come from a cleanup pass inside it
  ([above](#career-sponsors)). This agrees with the `partial` verdict V4586, which suggested a serialise name.
- **`0x141107420` `Game_CarDecals_FilterListByLod`** builds a sponsor's unlocked decal list: `default_decals` (`+0xb8`)
  plus each loyalty level's decal (`+0x98` levels) whose threshold is at or below `loyalty_xp` (`+0x114`, as the KB's own
  `SponsorEntry` has it). Proposed `Career_Sponsors_GetAvailableDecals`.
- **`0x14142e7f0` `Game_DecalCustomisation_SwapAssetGroup`** looks like the decal system's update and service state
  machine, of which the swap is one branch.

Where the contributor's earlier labels disagreed with the KB (`0x143c6e260`, `0x143c60480`, `0x1413ffe80`), the KB's
names are kept.

## Open questions {#open}

- T016 (decal colour write test) is pending: does node +0x18 reach DecalData.packedAlbedoColour, and is 0xffffffff mapped to 'no tint'?
- Writer of the node -> GPU DecalData buffer (renderer object +0x218 -> [+8]+0x80) is not found; 0x143c65870 is teardown.
- Thread safety of calling copy/add/writeback (0x14142bd50/0x14142dc70/0x14142d800) and discover 0x1410f43f0 from the frame tick (worker thread) is unverified; career-load apply and existing-career unlock not yet run in game.
- Whether kind-1 index 0x11 is the My Team livery in other saves (only learned/observed in the owner's save).
- Is [playerCustomisationInfoSystem+0x280] == [GameDecalCustomisationSystem+0x258]? Value of 0x146066c18 in the team_hq decal editor and in my_team_setup; badge record kind (1 expected); edit timing (preview vs confirm). Emblem filter 0.11.0 not yet seen in game.
- Meaning of vehicle customisation +0x78 (livery-bound sponsor reference candidate) and of GameDecalCustomisationSystem +0x418.
- Decal-type carousel sponsor-group identity filter in 0x1417be2a0 not established.
- Movement clamp against area corners not located (only the scale clamp 0x14142d630); centre vs outline clamping unknown.
- Mode-3 branch flag CustomisationMyTeamOverviewViewModel+0x2e9 and badge cancel/'changes not saved' restore not traced.
- My Team suit runtime source (which list/slot holds the chosen team suit) - T015 run 2 pending; what selects suit tab 0 vs 1.
- Whether the game-value table {b6e0ab1bb0280e02} reader is generic (so a 4th core-belief row YouthDevelopment works) is unverified.
- Whether 0x141fe5a10/0x141fe9600 resize the tile box at runtime (tile fill 0.10.0 not seen in game).
- Contents/semantics of decal_editor placement presets precedence vs active loadouts; preset/randomise application (0x14142b100/0x14142afe0) named as further ensure points but not traced.
- 0x141ff70a0 and 0x141ff72f0 are below the menu builder start 0x141ff79b0; our notes call them stack-slot sources for [rsp+0x78]/[rsp+0x80] - which function they belong to is not stated.

