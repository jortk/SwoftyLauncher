#pragma once
#include <swofty/game/base.hpp>

namespace swofty::game {

struct NeAllocator {
    NeAllocatorVtbl *vtable; // L2 experimental
};
static_assert(offsetof(NeAllocator, vtable) == 0x0);

struct NeAllocatorVtbl {
    uint8_t _pad_0x0[0x18];
    void *alloc; // L2 experimental
    uint8_t _pad_0x20[0x18];
    void *free; // L2 experimental
};
static_assert(offsetof(NeAllocatorVtbl, alloc) == 0x18);
static_assert(offsetof(NeAllocatorVtbl, free) == 0x38);

struct NeBagCodec {
    void **vtable; // L3 stable
    void *stream; // L3 stable
};
static_assert(offsetof(NeBagCodec, vtable) == 0x0);
static_assert(offsetof(NeBagCodec, stream) == 0x8);

struct NeCountingStream {
    void **vtable; // L3 stable
    uint8_t _pad_0x8[0x20];
    uint64_t count; // L3 stable
};
static_assert(offsetof(NeCountingStream, vtable) == 0x0);
static_assert(offsetof(NeCountingStream, count) == 0x28);

struct NeLogGate {
    uint32_t channel_hashes[373]; // L2 experimental
    uint8_t enabled; // L2 experimental
};
static_assert(offsetof(NeLogGate, channel_hashes) == 0x0);
static_assert(offsetof(NeLogGate, enabled) == 0x5d4);

struct NeMemWriteStream {
    void **vtable; // L3 stable
    uint8_t *data; // L3 stable
    uint64_t capacity; // L3 stable
    uint64_t position; // L3 stable
    uint64_t size; // L2 experimental
};
static_assert(offsetof(NeMemWriteStream, vtable) == 0x0);
static_assert(offsetof(NeMemWriteStream, data) == 0x8);
static_assert(offsetof(NeMemWriteStream, capacity) == 0x10);
static_assert(offsetof(NeMemWriteStream, position) == 0x18);
static_assert(offsetof(NeMemWriteStream, size) == 0x20);

struct NeMessageBus {
    uint8_t _pad_0x0[0x68];
    uint8_t active; // L2 experimental
};
static_assert(offsetof(NeMessageBus, active) == 0x68);

struct NeOptionalVec {
    NeAllocator *alloc; // L2 experimental
    void *begin; // L2 experimental
    void *end; // L2 experimental
    uint64_t count; // L2 experimental
    uint8_t engaged; // L2 experimental
    uint8_t _pad_0x21[0x7];
};
static_assert(offsetof(NeOptionalVec, alloc) == 0x0);
static_assert(offsetof(NeOptionalVec, begin) == 0x8);
static_assert(offsetof(NeOptionalVec, end) == 0x10);
static_assert(offsetof(NeOptionalVec, count) == 0x18);
static_assert(offsetof(NeOptionalVec, engaged) == 0x20);
static_assert(sizeof(NeOptionalVec) == 0x28);

struct NePropertyTypeVtbl {
    uint8_t _pad_0x0[0x10];
    void *id; // L3 stable
    uint8_t _pad_0x18[0x8];
    void *key_offset; // L3 stable
};
static_assert(offsetof(NePropertyTypeVtbl, id) == 0x10);
static_assert(offsetof(NePropertyTypeVtbl, key_offset) == 0x20);

struct NePropertyVtbl {
    uint8_t _pad_0x0[0x8];
    void *type; // L3 stable
    uint8_t _pad_0x10[0x28];
    void *value; // L3 stable
};
static_assert(offsetof(NePropertyVtbl, type) == 0x8);
static_assert(offsetof(NePropertyVtbl, value) == 0x38);

struct NeSharedCtrl {
    void **vtable; // L2 experimental
    int32_t strong; // L2 experimental
    int32_t weak; // L2 experimental
};
static_assert(offsetof(NeSharedCtrl, vtable) == 0x0);
static_assert(offsetof(NeSharedCtrl, strong) == 0x8);
static_assert(offsetof(NeSharedCtrl, weak) == 0xc);

struct NeStreamVtbl {
    uint8_t _pad_0x0[0x48];
    void *write; // L3 stable
    void *write_length; // L3 stable
};
static_assert(offsetof(NeStreamVtbl, write) == 0x48);
static_assert(offsetof(NeStreamVtbl, write_length) == 0x50);

struct SaveDataContainer {
    void **vtable; // L3 stable
    uint8_t _pad_0x8[0x78];
    const char *name; // L3 stable
    uint8_t _pad_0x88[0x78];
    uint8_t retain_unknown; // L2 experimental
    uint8_t _pad_0x101[0x7];
    uint64_t retain_unknown_key; // L2 experimental
    uint8_t _pad_0x110[0xb8];
    uint64_t retained_count; // L2 experimental
};
static_assert(offsetof(SaveDataContainer, vtable) == 0x0);
static_assert(offsetof(SaveDataContainer, name) == 0x80);
static_assert(offsetof(SaveDataContainer, retain_unknown) == 0x100);
static_assert(offsetof(SaveDataContainer, retain_unknown_key) == 0x108);
static_assert(offsetof(SaveDataContainer, retained_count) == 0x1c8);

inline constexpr address<"Ne_ActorManager_InsertEntryNode"> Ne_ActorManager_InsertEntryNode{}; // L2 experimental
inline constexpr address<"Ne_ActorManager_RegisterEntry"> Ne_ActorManager_RegisterEntry{}; // L2 experimental
inline constexpr address<"Ne_ActorSceneDataResourceFactory_DestroyResource"> Ne_ActorSceneDataResourceFactory_DestroyResource{}; // L2 experimental
inline constexpr address<"Ne_ActorSceneDataResource_DestroyScenes"> Ne_ActorSceneDataResource_DestroyScenes{}; // L2 experimental
inline constexpr address<"Ne_AnimClipResourceFactory_CreateResource"> Ne_AnimClipResourceFactory_CreateResource{}; // L2 experimental, NeAnimClipResourceFactory::createResource
inline constexpr address<"Ne_AnimLayer_ApplyChannels"> Ne_AnimLayer_ApplyChannels{}; // L2 experimental
inline constexpr address<"Ne_AnimSetResourceFactory_CreateResource"> Ne_AnimSetResourceFactory_CreateResource{}; // L2 experimental, NeAnimSetResource::NeAnimSetResource
inline constexpr address<"Ne_Application2_OnFocusChanged"> Ne_Application2_OnFocusChanged{}; // L2 experimental, NeApplication2::onFocusChanged
inline constexpr address<"Ne_Application2_OnVisibilityChanged"> Ne_Application2_OnVisibilityChanged{}; // L2 experimental, NeApplication2::onVisibilityChanged
inline constexpr address<"Ne_Array_InsertionSortPtrs"> Ne_Array_InsertionSortPtrs{}; // L2 experimental
inline constexpr address<"Ne_AssetGroupSubTask_OnComplete"> Ne_AssetGroupSubTask_OnComplete{}; // L2 experimental
inline constexpr address<"Ne_AssetLoader_RequestGroupDescs"> Ne_AssetLoader_RequestGroupDescs{}; // L2 experimental
inline constexpr address<"Ne_AssetLoader_RequestGroupHandles"> Ne_AssetLoader_RequestGroupHandles{}; // L2 experimental
inline constexpr address<"Ne_AsyncJob_StopAndNotify"> Ne_AsyncJob_StopAndNotify{}; // L2 experimental
inline constexpr address<"Ne_AudioIo_OpenFileRequest"> Ne_AudioIo_OpenFileRequest{}; // L2 experimental, NeAudioIoConfig::m_maxConcurrentIo
inline constexpr address<"Ne_BindingTable_AddOrUpdate"> Ne_BindingTable_AddOrUpdate{}; // L2 experimental
inline constexpr address<"Ne_BindingTable_FindChannel"> Ne_BindingTable_FindChannel{}; // L2 experimental
inline constexpr address<"Ne_BitAllocator_AllocBit"> Ne_BitAllocator_AllocBit{}; // L2 experimental
inline constexpr address<"Ne_BitStream_ReadQuantisedFloat"> Ne_BitStream_ReadQuantisedFloat{}; // L2 experimental
inline constexpr address<"Ne_BitStream_ReadQuantizedFloat"> Ne_BitStream_ReadQuantizedFloat{}; // L2 experimental
inline constexpr address<"Ne_BitStream_ReadRangedInt"> Ne_BitStream_ReadRangedInt{}; // L2 experimental
inline constexpr address<"Ne_BitStream_ReadRangedUInt"> Ne_BitStream_ReadRangedUInt{}; // L2 experimental
inline constexpr address<"Ne_BitStream_WriteBits"> Ne_BitStream_WriteBits{}; // L2 experimental
inline constexpr address<"Ne_BitStream_WriteFloatByPrecision"> Ne_BitStream_WriteFloatByPrecision{}; // L2 experimental
inline constexpr address<"Ne_BitStream_WriteQuantizedFloat"> Ne_BitStream_WriteQuantizedFloat{}; // L2 experimental
inline constexpr address<"Ne_BitStream_WriteRangedInt"> Ne_BitStream_WriteRangedInt{}; // L2 experimental
inline constexpr address<"Ne_BitStream_WriteZeroBits"> Ne_BitStream_WriteZeroBits{}; // L2 experimental
inline constexpr address<"Ne_Bitset_ThrowInvalidPosition_A"> Ne_Bitset_ThrowInvalidPosition_A{}; // L2 experimental
inline constexpr address<"Ne_Bitset_ThrowInvalidPosition_B"> Ne_Bitset_ThrowInvalidPosition_B{}; // L2 experimental
inline constexpr address<"Ne_Bitset_ThrowInvalidPosition_C"> Ne_Bitset_ThrowInvalidPosition_C{}; // L2 experimental
inline constexpr address<"Ne_BufferedAssetGroupLoader_GroupSwapAssetGroups"> Ne_BufferedAssetGroupLoader_GroupSwapAssetGroups{}; // L2 experimental, NeBufferedAssetGroupLoader::groupSwapAssetGroups
inline constexpr address<"Ne_BusMessage_SubscribeHandlerUnique"> Ne_BusMessage_SubscribeHandlerUnique{}; // L2 experimental
inline constexpr address<"Ne_ByteVector_InsertUnique"> Ne_ByteVector_InsertUnique{}; // L2 experimental
inline constexpr address<"Ne_CallbackList_InvokeReverse"> Ne_CallbackList_InvokeReverse{}; // L2 experimental
inline constexpr address<"Ne_CloudGenerator_Destroy"> Ne_CloudGenerator_Destroy{}; // L2 experimental, NeCloudGenerator::~NeCloudGenerator
inline constexpr address<"Ne_CloudUploader_Upload"> Ne_CloudUploader_Upload{}; // L2 experimental
using Ne_Compare_StringNoCase_t = int32_t(const char *a, const char *b);
inline constexpr fn<"Ne_Compare_StringNoCase", Ne_Compare_StringNoCase_t> Ne_Compare_StringNoCase{}; // L2 experimental, _stricmp
inline constexpr address<"Ne_ComponentDefinitions_CreateComponent"> Ne_ComponentDefinitions_CreateComponent{}; // L2 experimental
inline constexpr address<"Ne_ComponentDefinitions_CreateComponent_2"> Ne_ComponentDefinitions_CreateComponent_2{}; // L2 experimental
inline constexpr address<"Ne_Component_ResolveProviderChain"> Ne_Component_ResolveProviderChain{}; // L2 experimental
inline constexpr address<"Ne_Component_ResolveProviderChain2"> Ne_Component_ResolveProviderChain2{}; // L2 experimental
inline constexpr address<"Ne_ConcurrentHashMap_FatalStub"> Ne_ConcurrentHashMap_FatalStub{}; // L2 experimental
inline constexpr address<"Ne_ConcurrentHashMap_FindOrInsert"> Ne_ConcurrentHashMap_FindOrInsert{}; // L2 experimental
inline constexpr address<"Ne_ConcurrentHashMap_Rehash"> Ne_ConcurrentHashMap_Rehash{}; // L2 experimental
inline constexpr address<"Ne_ConfigDict_FindStringIndex"> Ne_ConfigDict_FindStringIndex{}; // L2 experimental
inline constexpr address<"Ne_ConfigValue_ReadInt"> Ne_ConfigValue_ReadInt{}; // L2 experimental
inline constexpr address<"Ne_Config_GetFloatOrDefault"> Ne_Config_GetFloatOrDefault{}; // L2 experimental
inline constexpr address<"Ne_Config_GetString"> Ne_Config_GetString{}; // L2 experimental
inline constexpr address<"Ne_Config_ReadSize"> Ne_Config_ReadSize{}; // L2 experimental
inline constexpr address<"Ne_Container_AbortNoReturn_142b52980"> Ne_Container_AbortNoReturn_142b52980{}; // L2 experimental
inline constexpr address<"Ne_Container_AbortNoReturn_142cc4e50"> Ne_Container_AbortNoReturn_142cc4e50{}; // L2 experimental
inline constexpr address<"Ne_Container_AbortNoReturn_142ee16c0"> Ne_Container_AbortNoReturn_142ee16c0{}; // L2 experimental
inline constexpr address<"Ne_Container_AbortNoReturn_1435d82f0"> Ne_Container_AbortNoReturn_1435d82f0{}; // L2 experimental
inline constexpr address<"Ne_Container_AbortNoReturn_1443908d0"> Ne_Container_AbortNoReturn_1443908d0{}; // L2 experimental
inline constexpr address<"Ne_Container_GrowVectorChecked"> Ne_Container_GrowVectorChecked{}; // L2 experimental
inline constexpr address<"Ne_Create_EventQueueTransactions"> Ne_Create_EventQueueTransactions{}; // L2 experimental
inline constexpr address<"Ne_CrowdAnimationLoader_LoadBehavioursInStage"> Ne_CrowdAnimationLoader_LoadBehavioursInStage{}; // L2 experimental, NeCrowdAnimationLoader::loadBehavioursInStage
inline constexpr address<"Ne_CrowdAnimationLoader_OnAnimClipChanged"> Ne_CrowdAnimationLoader_OnAnimClipChanged{}; // L2 experimental, NeCrowdAnimationLoader::onAnimClipChanged
inline constexpr address<"Ne_CrowdPluginConnection_ProcessMessage"> Ne_CrowdPluginConnection_ProcessMessage{}; // L2 experimental, NeCrowdPluginConnection::processMessage
inline constexpr address<"Ne_CrowdPluginProxy_Destructor"> Ne_CrowdPluginProxy_Destructor{}; // L2 experimental
inline constexpr address<"Ne_CrowdPluginProxy_Update"> Ne_CrowdPluginProxy_Update{}; // L2 experimental
inline constexpr address<"Ne_CrowdPlugin_FindZoneIndex"> Ne_CrowdPlugin_FindZoneIndex{}; // L2 experimental, NeCrowdPlugin::setAnimType
inline constexpr address<"Ne_CrowdPlugin_LoadXml"> Ne_CrowdPlugin_LoadXml{}; // L2 experimental
inline constexpr address<"Ne_CrowdZoneInstance_Initialise"> Ne_CrowdZoneInstance_Initialise{}; // L2 experimental, NeCrowdZoneInstance::updateTransforms
inline constexpr address<"Ne_Crt_AbortNoReturn_A"> Ne_Crt_AbortNoReturn_A{}; // L2 experimental
inline constexpr address<"Ne_Crt_AbortNoReturn_B"> Ne_Crt_AbortNoReturn_B{}; // L2 experimental
inline constexpr address<"Ne_Crt_Acosf"> Ne_Crt_Acosf{}; // L2 experimental
inline constexpr address<"Ne_Crt_AsinfImpl"> Ne_Crt_AsinfImpl{}; // L2 experimental
inline constexpr address<"Ne_Crt_Atan2f"> Ne_Crt_Atan2f{}; // L2 experimental
inline constexpr address<"Ne_Crt_Atanf"> Ne_Crt_Atanf{}; // L2 experimental
inline constexpr address<"Ne_Crt_Atexit"> Ne_Crt_Atexit{}; // L2 experimental
inline constexpr address<"Ne_Crt_Chkstk"> Ne_Crt_Chkstk{}; // L2 experimental
inline constexpr address<"Ne_Crt_Cosf"> Ne_Crt_Cosf{}; // L2 experimental
inline constexpr address<"Ne_Crt_DiffClock"> Ne_Crt_DiffClock{}; // L2 experimental
inline constexpr address<"Ne_Crt_Exp2f"> Ne_Crt_Exp2f{}; // L2 experimental
inline constexpr address<"Ne_Crt_FastFailTerminate"> Ne_Crt_FastFailTerminate{}; // L2 experimental
inline constexpr address<"Ne_Crt_FatalNoReturn"> Ne_Crt_FatalNoReturn{}; // L2 experimental
inline constexpr address<"Ne_Crt_FatalTrapB"> Ne_Crt_FatalTrapB{}; // L2 experimental
inline constexpr address<"Ne_Crt_FatalTrapC"> Ne_Crt_FatalTrapC{}; // L2 experimental
using Ne_Crt_FloatToInt32Checked_t = int(uint32_t bits);
inline constexpr fn<"Ne_Crt_FloatToInt32Checked", Ne_Crt_FloatToInt32Checked_t> Ne_Crt_FloatToInt32Checked{}; // L2 experimental
inline constexpr address<"Ne_Crt_Fmodf"> Ne_Crt_Fmodf{}; // L2 experimental
inline constexpr address<"Ne_Crt_ForwardTo14534a050"> Ne_Crt_ForwardTo14534a050{}; // L2 experimental
inline constexpr address<"Ne_Crt_InitThreadHeader"> Ne_Crt_InitThreadHeader{}; // L2 experimental
inline constexpr address<"Ne_Crt_InvalidParameterAbort"> Ne_Crt_InvalidParameterAbort{}; // L2 experimental
using Ne_Crt_InvalidParameterAbortStub_140e0fd20_t = void();
inline constexpr fn<"Ne_Crt_InvalidParameterAbortStub_140e0fd20", Ne_Crt_InvalidParameterAbortStub_140e0fd20_t> Ne_Crt_InvalidParameterAbortStub_140e0fd20{}; // L2 experimental
inline constexpr address<"Ne_Crt_InvalidParameterDispatch"> Ne_Crt_InvalidParameterDispatch{}; // L2 experimental
inline constexpr address<"Ne_Crt_InvalidParameterNoReturn"> Ne_Crt_InvalidParameterNoReturn{}; // L2 experimental
inline constexpr address<"Ne_Crt_LazyInitPreserveLastError"> Ne_Crt_LazyInitPreserveLastError{}; // L2 experimental
inline constexpr address<"Ne_Crt_Localtime64"> Ne_Crt_Localtime64{}; // L2 experimental
inline constexpr address<"Ne_Crt_LogFloatAvx"> Ne_Crt_LogFloatAvx{}; // L2 experimental
inline constexpr address<"Ne_Crt_MathErrorHandlerF"> Ne_Crt_MathErrorHandlerF{}; // L2 experimental
inline constexpr address<"Ne_Crt_Memcmp"> Ne_Crt_Memcmp{}; // L2 experimental
inline constexpr address<"Ne_Crt_Memcpy"> Ne_Crt_Memcpy{}; // L2 experimental
inline constexpr address<"Ne_Crt_Memset"> Ne_Crt_Memset{}; // L2 experimental
inline constexpr address<"Ne_Crt_ParseNumber_L"> Ne_Crt_ParseNumber_L{}; // L2 experimental
inline constexpr address<"Ne_Crt_Powf"> Ne_Crt_Powf{}; // L2 experimental
inline constexpr address<"Ne_Crt_Qsort_Impl"> Ne_Crt_Qsort_Impl{}; // L2 experimental
inline constexpr address<"Ne_Crt_ReportGsFailure"> Ne_Crt_ReportGsFailure{}; // L2 experimental
inline constexpr address<"Ne_Crt_SecurityCookieFailure"> Ne_Crt_SecurityCookieFailure{}; // L2 experimental
inline constexpr address<"Ne_Crt_SetMxcsrExceptionFlags"> Ne_Crt_SetMxcsrExceptionFlags{}; // L2 experimental
inline constexpr address<"Ne_Crt_Setjmp"> Ne_Crt_Setjmp{}; // L2 experimental
inline constexpr address<"Ne_Crt_Sinf"> Ne_Crt_Sinf{}; // L2 experimental
inline constexpr address<"Ne_Crt_Snprintf"> Ne_Crt_Snprintf{}; // L2 experimental
inline constexpr address<"Ne_Crt_Sqrtf"> Ne_Crt_Sqrtf{}; // L2 experimental
inline constexpr address<"Ne_Crt_Strcmp"> Ne_Crt_Strcmp{}; // L2 experimental
inline constexpr address<"Ne_Crt_Strncmp"> Ne_Crt_Strncmp{}; // L2 experimental
inline constexpr address<"Ne_Crt_Strnicmp"> Ne_Crt_Strnicmp{}; // L2 experimental
inline constexpr address<"Ne_Crt_Strstr"> Ne_Crt_Strstr{}; // L2 experimental
inline constexpr address<"Ne_Crt_StrtolCore"> Ne_Crt_StrtolCore{}; // L2 experimental
inline constexpr address<"Ne_Crt_Swprintf"> Ne_Crt_Swprintf{}; // L2 experimental
inline constexpr address<"Ne_Crt_ThrowOverflow_A"> Ne_Crt_ThrowOverflow_A{}; // L2 experimental
inline constexpr address<"Ne_Crt_ThrowOverflow_B"> Ne_Crt_ThrowOverflow_B{}; // L2 experimental
inline constexpr address<"Ne_Crt_ThrowOverflow_C"> Ne_Crt_ThrowOverflow_C{}; // L2 experimental
inline constexpr address<"Ne_Crt_Time64"> Ne_Crt_Time64{}; // L2 experimental
inline constexpr address<"Ne_Crt_ToLower"> Ne_Crt_ToLower{}; // L2 experimental
inline constexpr address<"Ne_Crt_TrigDouble"> Ne_Crt_TrigDouble{}; // L2 experimental
inline constexpr address<"Ne_Crt_VsnprintfWrapper"> Ne_Crt_VsnprintfWrapper{}; // L2 experimental
inline constexpr address<"Ne_CustomisationDecalRenderer_BakePasses"> Ne_CustomisationDecalRenderer_BakePasses{}; // L2 experimental
inline constexpr address<"Ne_CustomisationDecalRenderer_Render"> Ne_CustomisationDecalRenderer_Render{}; // L2 experimental, NeCustomisationDecalRenderer::render
inline constexpr address<"Ne_CustomisationDecalsProjectionAreasSetPlugin_CloseConnection"> Ne_CustomisationDecalsProjectionAreasSetPlugin_CloseConnection{}; // L2 experimental, NeCustomisationDecalsProjectionAreasSetPlugin::closeConnection
inline constexpr address<"Ne_CustomisationDecalsSetPlugin_CloseConnection"> Ne_CustomisationDecalsSetPlugin_CloseConnection{}; // L2 experimental, NeCustomisationDecalsSetPlugin::closeConnection
inline constexpr address<"Ne_CutsceneScriptResourceFactory_CreateResource"> Ne_CutsceneScriptResourceFactory_CreateResource{}; // L2 experimental
inline constexpr address<"Ne_CutsceneScriptResourceFactory_CreateResourceFinished"> Ne_CutsceneScriptResourceFactory_CreateResourceFinished{}; // L2 experimental
inline constexpr address<"Ne_CutsceneScriptResourceFactory_DestroyResource"> Ne_CutsceneScriptResourceFactory_DestroyResource{}; // L2 experimental
inline constexpr address<"Ne_CutsceneScriptResourceFactory_StartDestroyResource"> Ne_CutsceneScriptResourceFactory_StartDestroyResource{}; // L2 experimental
inline constexpr address<"Ne_DataReader_DetectFormat"> Ne_DataReader_DetectFormat{}; // L2 experimental
inline constexpr address<"Ne_Debug_GetCallerAddress"> Ne_Debug_GetCallerAddress{}; // L2 experimental
inline constexpr address<"Ne_Debug_RecordRenderError"> Ne_Debug_RecordRenderError{}; // L2 experimental
inline constexpr address<"Ne_Debug_RecordTraceEvent3"> Ne_Debug_RecordTraceEvent3{}; // L2 experimental
inline constexpr address<"Ne_Debug_RecordTraceEvent5"> Ne_Debug_RecordTraceEvent5{}; // L2 experimental
inline constexpr address<"Ne_DecalsProjectionAreaProxy_Ctor"> Ne_DecalsProjectionAreaProxy_Ctor{}; // L2 experimental
inline constexpr address<"Ne_DecalsProjectionAreaProxy_Dtor"> Ne_DecalsProjectionAreaProxy_Dtor{}; // L2 experimental
inline constexpr address<"Ne_DecalsSetProxy_Dtor"> Ne_DecalsSetProxy_Dtor{}; // L2 experimental
inline constexpr address<"Ne_Delegate_InvokeAll"> Ne_Delegate_InvokeAll{}; // L2 experimental
inline constexpr address<"Ne_Deque_Grow_Qword"> Ne_Deque_Grow_Qword{}; // L2 experimental
using Ne_Detach_ChildNode_t = int(void *parent, void *child);
inline constexpr fn<"Ne_Detach_ChildNode", Ne_Detach_ChildNode_t> Ne_Detach_ChildNode{}; // L2 experimental
inline constexpr address<"Ne_EadpErrorsDataService_UninstallExceptionHandler"> Ne_EadpErrorsDataService_UninstallExceptionHandler{}; // L2 experimental
inline constexpr address<"Ne_EadpErrorsDataService_UploadAttachment_OnComplete"> Ne_EadpErrorsDataService_UploadAttachment_OnComplete{}; // L2 experimental
inline constexpr address<"Ne_EadpGameInviteList_DeleteSessionInvite"> Ne_EadpGameInviteList_DeleteSessionInvite{}; // L2 experimental, neon::NeEadpGameInviteList::deleteSessionInvite
inline constexpr address<"Ne_EadpGameInviteList_OnInviteValidated"> Ne_EadpGameInviteList_OnInviteValidated{}; // L2 experimental, neon::NeEadpGameInviteList::onInviteValidated
inline constexpr address<"Ne_EadpGameInviteList_UpdateInviteLifetime"> Ne_EadpGameInviteList_UpdateInviteLifetime{}; // L2 experimental, neon::NeEadpGameInviteList::updateInviteLifetime
inline constexpr address<"Ne_EadpPresenceService_UnsubscribeFriends"> Ne_EadpPresenceService_UnsubscribeFriends{}; // L2 experimental, neon::NeEadpPresenceService::unsubscribeFriends
using Ne_Entity_FindComponent_t = void *(void *entity, const void *type);
inline constexpr fn<"Ne_Entity_FindComponent", Ne_Entity_FindComponent_t> Ne_Entity_FindComponent{}; // L2 experimental
inline constexpr address<"Ne_ErrorContext_SetCode"> Ne_ErrorContext_SetCode{}; // L2 experimental
inline constexpr address<"Ne_ErrorTrace_PushRing"> Ne_ErrorTrace_PushRing{}; // L2 experimental
inline constexpr address<"Ne_EventBus_Unsubscribe"> Ne_EventBus_Unsubscribe{}; // L2 experimental
inline constexpr address<"Ne_EventDispatcher_ProcessQueue"> Ne_EventDispatcher_ProcessQueue{}; // L2 experimental
inline constexpr address<"Ne_EventDispatcher_SendEvent"> Ne_EventDispatcher_SendEvent{}; // L2 experimental
inline constexpr address<"Ne_EventDispatcher_TrackQueuePeak"> Ne_EventDispatcher_TrackQueuePeak{}; // L2 experimental
inline constexpr address<"Ne_EventQueue_DispatchPending"> Ne_EventQueue_DispatchPending{}; // L2 experimental
inline constexpr address<"Ne_Event_ResetWithErrorTrace"> Ne_Event_ResetWithErrorTrace{}; // L2 experimental
inline constexpr address<"Ne_FatalFormat"> Ne_FatalFormat{}; // L2 experimental
inline constexpr address<"Ne_FatalFormat_Trap"> Ne_FatalFormat_Trap{}; // L2 experimental
inline constexpr address<"Ne_FileSystem_OpenStream"> Ne_FileSystem_OpenStream{}; // L2 experimental
inline constexpr address<"Ne_FlagMaskSet_SetBit"> Ne_FlagMaskSet_SetBit{}; // L2 experimental
inline constexpr address<"Ne_FlatHashMap_FindSlotU64_A"> Ne_FlatHashMap_FindSlotU64_A{}; // L2 experimental
inline constexpr address<"Ne_FlatHashMap_FindSlotU64_B"> Ne_FlatHashMap_FindSlotU64_B{}; // L2 experimental
inline constexpr address<"Ne_FlatHashMap_FindSlotU64_C"> Ne_FlatHashMap_FindSlotU64_C{}; // L2 experimental
inline constexpr address<"Ne_FlowStateInterruptManager_ConsumeInterrupt"> Ne_FlowStateInterruptManager_ConsumeInterrupt{}; // L2 experimental, NeFlowStateInterruptManager::consumeInterrupt
inline constexpr address<"Ne_FogSet_SetFogData"> Ne_FogSet_SetFogData{}; // L2 experimental, NeFogSet::setFogData
inline constexpr address<"Ne_Format_HresultUnknownFacility"> Ne_Format_HresultUnknownFacility{}; // L2 experimental
using Ne_Format_String_t = int64_t(NeStr **out, const char *fmt, ...);
inline constexpr fn<"Ne_Format_String", Ne_Format_String_t> Ne_Format_String{}; // L2 experimental
inline constexpr address<"Ne_Get_CStrLen"> Ne_Get_CStrLen{}; // L2 experimental
using Ne_Get_SettingBool_t = uint64_t(void *node, const char *name, uint8_t *out, uint8_t def);
inline constexpr fn<"Ne_Get_SettingBool", Ne_Get_SettingBool_t> Ne_Get_SettingBool{}; // L2 experimental
using Ne_Get_SettingU32_t = uint64_t(void *node, const char *name, uint32_t *out, uint32_t def);
inline constexpr fn<"Ne_Get_SettingU32", Ne_Get_SettingU32_t> Ne_Get_SettingU32{}; // L2 experimental
inline constexpr address<"Ne_Gfx2CommandQueue_FlushResourceBarriers"> Ne_Gfx2CommandQueue_FlushResourceBarriers{}; // L2 experimental
inline constexpr address<"Ne_Gfx2CommandQueue_PopProfileMarker"> Ne_Gfx2CommandQueue_PopProfileMarker{}; // L2 experimental
inline constexpr address<"Ne_Gfx2CommandQueue_PushProfileMarker"> Ne_Gfx2CommandQueue_PushProfileMarker{}; // L2 experimental
inline constexpr address<"Ne_Gfx2Renderer_BuildRfgSubmitJob"> Ne_Gfx2Renderer_BuildRfgSubmitJob{}; // L2 experimental
inline constexpr address<"Ne_Gfx2Renderer_ConstructDevice"> Ne_Gfx2Renderer_ConstructDevice{}; // L2 experimental
inline constexpr address<"Ne_Gfx2Renderer_ConstructQueueState"> Ne_Gfx2Renderer_ConstructQueueState{}; // L2 experimental
inline constexpr address<"Ne_Gfx2Renderer_DestroyDeferredEntryB"> Ne_Gfx2Renderer_DestroyDeferredEntryB{}; // L2 experimental
inline constexpr address<"Ne_Gfx2Renderer_EnqueueJobLocked"> Ne_Gfx2Renderer_EnqueueJobLocked{}; // L2 experimental
inline constexpr address<"Ne_Gfx2Renderer_ForwardEventToListener"> Ne_Gfx2Renderer_ForwardEventToListener{}; // L2 experimental
inline constexpr address<"Ne_Gfx2Renderer_FrameEntryNameMap_FatalStub"> Ne_Gfx2Renderer_FrameEntryNameMap_FatalStub{}; // L2 experimental
inline constexpr address<"Ne_Gfx2Renderer_FrameEntryNameMap_Rehash"> Ne_Gfx2Renderer_FrameEntryNameMap_Rehash{}; // L2 experimental
inline constexpr address<"Ne_Gfx2Renderer_FrameStartEnd"> Ne_Gfx2Renderer_FrameStartEnd{}; // L2 experimental
inline constexpr address<"Ne_Gfx2Renderer_GetCommandList"> Ne_Gfx2Renderer_GetCommandList{}; // L2 experimental
inline constexpr address<"Ne_Gfx2Renderer_Initialise"> Ne_Gfx2Renderer_Initialise{}; // L2 experimental
inline constexpr address<"Ne_Gfx2Renderer_PostEvent_AllocateResource"> Ne_Gfx2Renderer_PostEvent_AllocateResource{}; // L2 experimental
inline constexpr address<"Ne_Gfx2Renderer_PostEvent_ErpLoaded"> Ne_Gfx2Renderer_PostEvent_ErpLoaded{}; // L2 experimental
inline constexpr address<"Ne_Gfx2Renderer_PostEvent_ErpRead"> Ne_Gfx2Renderer_PostEvent_ErpRead{}; // L2 experimental
inline constexpr address<"Ne_Gfx2Renderer_PostEvent_ErpUnloaded"> Ne_Gfx2Renderer_PostEvent_ErpUnloaded{}; // L2 experimental
inline constexpr address<"Ne_Gfx2Renderer_ProcessResourceEvent"> Ne_Gfx2Renderer_ProcessResourceEvent{}; // L2 experimental
inline constexpr address<"Ne_Gfx2Renderer_RecordCommandNodes"> Ne_Gfx2Renderer_RecordCommandNodes{}; // L2 experimental
inline constexpr address<"Ne_Gfx2Renderer_ReleaseDeferredObjects"> Ne_Gfx2Renderer_ReleaseDeferredObjects{}; // L2 experimental
inline constexpr address<"Ne_Gfx2Renderer_RenderFrame"> Ne_Gfx2Renderer_RenderFrame{}; // L2 experimental
inline constexpr address<"Ne_Gfx2Renderer_UploadTransientBatch"> Ne_Gfx2Renderer_UploadTransientBatch{}; // L2 experimental
inline constexpr address<"Ne_Gfx2TextureManager_SetInRaceOff"> Ne_Gfx2TextureManager_SetInRaceOff{}; // L2 experimental
inline constexpr address<"Ne_Gfx2TextureManager_SetInRaceOn"> Ne_Gfx2TextureManager_SetInRaceOn{}; // L2 experimental
inline constexpr address<"Ne_Gfx2_BeginTripleBufferedScope"> Ne_Gfx2_BeginTripleBufferedScope{}; // L2 experimental
inline constexpr address<"Ne_Gfx2_TrackResourceInsert_A"> Ne_Gfx2_TrackResourceInsert_A{}; // L2 experimental
inline constexpr address<"Ne_Gfx2_TrackResourceInsert_B"> Ne_Gfx2_TrackResourceInsert_B{}; // L2 experimental
inline constexpr address<"Ne_Gfx2_TrackResourceInsert_C"> Ne_Gfx2_TrackResourceInsert_C{}; // L2 experimental
inline constexpr address<"Ne_GlobalHandle_Release"> Ne_GlobalHandle_Release{}; // L2 experimental
using Ne_Handle_WebSocketUpgradeWait_t = void(void *self, uint64_t a2, uint64_t a3, char *a4, char a5, char *param_6);
inline constexpr fn<"Ne_Handle_WebSocketUpgradeWait", Ne_Handle_WebSocketUpgradeWait_t> Ne_Handle_WebSocketUpgradeWait{}; // L2 experimental
using Ne_HashMap_AtByteKey_OrThrow_t = void(void **map_ref, void *key_obj);
inline constexpr fn<"Ne_HashMap_AtByteKey_OrThrow", Ne_HashMap_AtByteKey_OrThrow_t> Ne_HashMap_AtByteKey_OrThrow{}; // L2 experimental
inline constexpr address<"Ne_HashMap_EmplaceNode"> Ne_HashMap_EmplaceNode{}; // L2 experimental
inline constexpr address<"Ne_HashMap_FatalStub_GroundCover"> Ne_HashMap_FatalStub_GroundCover{}; // L2 experimental
inline constexpr address<"Ne_HashMap_FatalStub_Hbcn"> Ne_HashMap_FatalStub_Hbcn{}; // L2 experimental
inline constexpr address<"Ne_HashMap_FatalStub_ModelMaterial"> Ne_HashMap_FatalStub_ModelMaterial{}; // L2 experimental
inline constexpr address<"Ne_HashMap_FatalStub_ParticleResourcePack"> Ne_HashMap_FatalStub_ParticleResourcePack{}; // L2 experimental
inline constexpr address<"Ne_HashMap_FatalStub_Particles"> Ne_HashMap_FatalStub_Particles{}; // L2 experimental
inline constexpr address<"Ne_HashMap_FatalStub_SkidmarksA"> Ne_HashMap_FatalStub_SkidmarksA{}; // L2 experimental
inline constexpr address<"Ne_HashMap_FatalStub_SkidmarksB"> Ne_HashMap_FatalStub_SkidmarksB{}; // L2 experimental
using Ne_HashMap_FindOrInsert_ByteKey_t = void *(void *map, uint8_t key);
inline constexpr fn<"Ne_HashMap_FindOrInsert_ByteKey", Ne_HashMap_FindOrInsert_ByteKey_t> Ne_HashMap_FindOrInsert_ByteKey{}; // L2 experimental
inline constexpr address<"Ne_HashMap_FindOrInsert_Float"> Ne_HashMap_FindOrInsert_Float{}; // L2 experimental
inline constexpr address<"Ne_HashMap_FindSlot_Thumbnail"> Ne_HashMap_FindSlot_Thumbnail{}; // L2 experimental
inline constexpr address<"Ne_HashMap_Find_141b732f0"> Ne_HashMap_Find_141b732f0{}; // L2 experimental
inline constexpr address<"Ne_HashMap_GrowRehash_DecalLinks"> Ne_HashMap_GrowRehash_DecalLinks{}; // L2 experimental
inline constexpr address<"Ne_HashMap_GrowRehash_Decals"> Ne_HashMap_GrowRehash_Decals{}; // L2 experimental
inline constexpr address<"Ne_HashMap_Grow_Gfx2ResourceObject"> Ne_HashMap_Grow_Gfx2ResourceObject{}; // L2 experimental
inline constexpr address<"Ne_HashMap_Grow_GroundCoverPlugin"> Ne_HashMap_Grow_GroundCoverPlugin{}; // L2 experimental
inline constexpr address<"Ne_HashMap_Grow_Hbcn"> Ne_HashMap_Grow_Hbcn{}; // L2 experimental
inline constexpr address<"Ne_HashMap_Grow_MaterialActorId"> Ne_HashMap_Grow_MaterialActorId{}; // L2 experimental
inline constexpr address<"Ne_HashMap_Grow_ParticleResourcePack"> Ne_HashMap_Grow_ParticleResourcePack{}; // L2 experimental
inline constexpr address<"Ne_HashMap_Grow_SkidmarksA"> Ne_HashMap_Grow_SkidmarksA{}; // L2 experimental
inline constexpr address<"Ne_HashMap_Grow_SkidmarksB"> Ne_HashMap_Grow_SkidmarksB{}; // L2 experimental
inline constexpr address<"Ne_HashMap_Guid128_Grow"> Ne_HashMap_Guid128_Grow{}; // L2 experimental
inline constexpr address<"Ne_HashMap_Insert_DecalLinks"> Ne_HashMap_Insert_DecalLinks{}; // L2 experimental
inline constexpr address<"Ne_HashMap_MoveAssign_Decals"> Ne_HashMap_MoveAssign_Decals{}; // L2 experimental
inline constexpr address<"Ne_HashMap_Rehash"> Ne_HashMap_Rehash{}; // L2 experimental
inline constexpr address<"Ne_HashMap_Rehash_142b52990"> Ne_HashMap_Rehash_142b52990{}; // L2 experimental
inline constexpr address<"Ne_HashMap_Rehash_142ee16d0"> Ne_HashMap_Rehash_142ee16d0{}; // L2 experimental
inline constexpr address<"Ne_HashMap_Rehash_1435d8440"> Ne_HashMap_Rehash_1435d8440{}; // L2 experimental
inline constexpr address<"Ne_HashMap_Rehash_1443908e0"> Ne_HashMap_Rehash_1443908e0{}; // L2 experimental
inline constexpr address<"Ne_HashMap_Rehash_Gfx2Renderer"> Ne_HashMap_Rehash_Gfx2Renderer{}; // L2 experimental
inline constexpr address<"Ne_HashMap_ReserveBuckets_Gfx"> Ne_HashMap_ReserveBuckets_Gfx{}; // L2 experimental
inline constexpr address<"Ne_HashMap_ThrowLengthError_DecalFiles"> Ne_HashMap_ThrowLengthError_DecalFiles{}; // L2 experimental
inline constexpr address<"Ne_HashMap_ThrowLengthError_DecalLinks"> Ne_HashMap_ThrowLengthError_DecalLinks{}; // L2 experimental
inline constexpr address<"Ne_HashMap_ThrowLengthError_DecalLinks2"> Ne_HashMap_ThrowLengthError_DecalLinks2{}; // L2 experimental
inline constexpr address<"Ne_HashMap_ThrowLengthError_Decals"> Ne_HashMap_ThrowLengthError_Decals{}; // L2 experimental
inline constexpr address<"Ne_HashMap_ThrowLengthError_Decals2"> Ne_HashMap_ThrowLengthError_Decals2{}; // L2 experimental
inline constexpr address<"Ne_HashMap_ThrowLengthError_Gfx2ResourceObject"> Ne_HashMap_ThrowLengthError_Gfx2ResourceObject{}; // L2 experimental
inline constexpr address<"Ne_HashMap_ThrowLengthError_MaterialActorId"> Ne_HashMap_ThrowLengthError_MaterialActorId{}; // L2 experimental
inline constexpr address<"Ne_HashMap_ThrowLengthError_Thumbnail"> Ne_HashMap_ThrowLengthError_Thumbnail{}; // L2 experimental
inline constexpr address<"Ne_HashMap_U64_FindOrInsertSlot"> Ne_HashMap_U64_FindOrInsertSlot{}; // L2 experimental
inline constexpr address<"Ne_HashMap_U64_Grow"> Ne_HashMap_U64_Grow{}; // L2 experimental
inline constexpr address<"Ne_HashMap_U64_Insert"> Ne_HashMap_U64_Insert{}; // L2 experimental
inline constexpr address<"Ne_HashMap_U64_Insert_Fnv"> Ne_HashMap_U64_Insert_Fnv{}; // L2 experimental
using Ne_HashSet_FindUShortKey_t = void(void *set, void *out_it, uint32_t key16);
inline constexpr fn<"Ne_HashSet_FindUShortKey", Ne_HashSet_FindUShortKey_t> Ne_HashSet_FindUShortKey{}; // L2 experimental
inline constexpr address<"Ne_Hash_Djb2Xor"> Ne_Hash_Djb2Xor{}; // L2 experimental
inline constexpr address<"Ne_Hash_Md5Digest"> Ne_Hash_Md5Digest{}; // L2 experimental
inline constexpr address<"Ne_Hash_Md5Finalize"> Ne_Hash_Md5Finalize{}; // L2 experimental
inline constexpr address<"Ne_Hash_Md5PadFinalBlock"> Ne_Hash_Md5PadFinalBlock{}; // L2 experimental
inline constexpr address<"Ne_Hash_Md5Transform"> Ne_Hash_Md5Transform{}; // L2 experimental
inline constexpr address<"Ne_Hash_Md5Transform_Alt"> Ne_Hash_Md5Transform_Alt{}; // L2 experimental
inline constexpr address<"Ne_Hash_Md5U64"> Ne_Hash_Md5U64{}; // L2 experimental
inline constexpr address<"Ne_Hash_StringDjb2_Lookup"> Ne_Hash_StringDjb2_Lookup{}; // L2 experimental
inline constexpr address<"Ne_HashedName_FromString"> Ne_HashedName_FromString{}; // L2 experimental
inline constexpr address<"Ne_HashedString_Assign"> Ne_HashedString_Assign{}; // L2 experimental
inline constexpr address<"Ne_Heap_AllocUnknownTag"> Ne_Heap_AllocUnknownTag{}; // L2 experimental
using Ne_Heap_DeallocateNonSmall_t = uint64_t(void *heap, uint8_t *ptr);
inline constexpr fn<"Ne_Heap_DeallocateNonSmall", Ne_Heap_DeallocateNonSmall_t> Ne_Heap_DeallocateNonSmall{}; // L2 experimental
inline constexpr address<"Ne_Heap_OnCorruptionNotification"> Ne_Heap_OnCorruptionNotification{}; // L2 experimental
inline constexpr address<"Ne_IBLCubeProbe_FreeInterpolatedArrays"> Ne_IBLCubeProbe_FreeInterpolatedArrays{}; // L2 experimental
inline constexpr address<"Ne_Import_ResolveSlotWithRetry"> Ne_Import_ResolveSlotWithRetry{}; // L2 experimental
inline constexpr address<"Ne_Init_ResourceMessageHandlerConfig"> Ne_Init_ResourceMessageHandlerConfig{}; // L2 experimental
inline constexpr address<"Ne_InputDeviceDirectInput_ResetForceFeedback"> Ne_InputDeviceDirectInput_ResetForceFeedback{}; // L2 experimental, NeInputDeviceDirectInput::resetForceFeedback
inline constexpr address<"Ne_InputDevicePadPS_Cleanup"> Ne_InputDevicePadPS_Cleanup{}; // L2 experimental, NeInputDevicePadPS::cleanup
inline constexpr address<"Ne_InputDevicePadPS_Initialise"> Ne_InputDevicePadPS_Initialise{}; // L2 experimental, NeInputDevicePadPS::initialise
inline constexpr address<"Ne_InputLibraryDirectInput_EnumDevicesCallback"> Ne_InputLibraryDirectInput_EnumDevicesCallback{}; // L2 experimental, NeInputLibraryDirectInput::enumDevicesCallback
inline constexpr address<"Ne_InputLibraryDirectInput_IsXInputDevice"> Ne_InputLibraryDirectInput_IsXInputDevice{}; // L2 experimental, NeInputLibraryDirectInput::isXInputDevice
inline constexpr address<"Ne_InputMapper_FindBindingByHash"> Ne_InputMapper_FindBindingByHash{}; // L2 experimental
inline constexpr address<"Ne_InputMapper_HasAction"> Ne_InputMapper_HasAction{}; // L2 experimental
inline constexpr address<"Ne_InputMapper_IsActionPressed"> Ne_InputMapper_IsActionPressed{}; // L2 experimental
inline constexpr address<"Ne_InputMapper_IsActionReleased"> Ne_InputMapper_IsActionReleased{}; // L2 experimental
inline constexpr address<"Ne_InputSystem_DeviceConnection"> Ne_InputSystem_DeviceConnection{}; // L2 experimental, NeInputSystem::deviceConnection
inline constexpr address<"Ne_InputSystem_Init"> Ne_InputSystem_Init{}; // L2 experimental
inline constexpr address<"Ne_InputSystem_MergePreset"> Ne_InputSystem_MergePreset{}; // L2 experimental
inline constexpr address<"Ne_InputSystem_UnbindDevice"> Ne_InputSystem_UnbindDevice{}; // L2 experimental
inline constexpr address<"Ne_Input_InternActionName"> Ne_Input_InternActionName{}; // L2 experimental
inline constexpr address<"Ne_Input_IsActionDownAnyDevice"> Ne_Input_IsActionDownAnyDevice{}; // L2 experimental
inline constexpr address<"Ne_Input_IsActionEnabled"> Ne_Input_IsActionEnabled{}; // L2 experimental
inline constexpr address<"Ne_InternedString_Manage"> Ne_InternedString_Manage{}; // L2 experimental
inline constexpr address<"Ne_InternedString_ManageB"> Ne_InternedString_ManageB{}; // L2 experimental
inline constexpr address<"Ne_IntrusiveList_ClearAndRelease"> Ne_IntrusiveList_ClearAndRelease{}; // L2 experimental
inline constexpr address<"Ne_IntrusivePtr_Assign"> Ne_IntrusivePtr_Assign{}; // L2 experimental
inline constexpr address<"Ne_JobQueue_FindBestJob"> Ne_JobQueue_FindBestJob{}; // L2 experimental
inline constexpr address<"Ne_JobQueue_RunPendingJobs"> Ne_JobQueue_RunPendingJobs{}; // L2 experimental
inline constexpr address<"Ne_JobSignal_WaitWithTimeout"> Ne_JobSignal_WaitWithTimeout{}; // L2 experimental
inline constexpr address<"Ne_JobSystem_CreateJobGroup"> Ne_JobSystem_CreateJobGroup{}; // L2 experimental
inline constexpr address<"Ne_JsonObject_FindMember"> Ne_JsonObject_FindMember{}; // L2 experimental
inline constexpr address<"Ne_JsonObject_FindMember_2"> Ne_JsonObject_FindMember_2{}; // L2 experimental
inline constexpr address<"Ne_JsonWriter_WriteBool"> Ne_JsonWriter_WriteBool{}; // L2 experimental
inline constexpr address<"Ne_Libpng_CheckIhdr"> Ne_Libpng_CheckIhdr{}; // L2 experimental
inline constexpr address<"Ne_Libpng_DeflateEndChecked"> Ne_Libpng_DeflateEndChecked{}; // L2 experimental
inline constexpr address<"Ne_Libpng_ErrorNoReturn"> Ne_Libpng_ErrorNoReturn{}; // L2 experimental
inline constexpr address<"Ne_Libpng_PrepareWrite"> Ne_Libpng_PrepareWrite{}; // L2 experimental
inline constexpr address<"Ne_Libpng_SetIhdr"> Ne_Libpng_SetIhdr{}; // L2 experimental
inline constexpr address<"Ne_Libpng_SetjmpCleanup_1447a0cf0"> Ne_Libpng_SetjmpCleanup_1447a0cf0{}; // L2 experimental
inline constexpr address<"Ne_Libpng_Warning"> Ne_Libpng_Warning{}; // L2 experimental
inline constexpr address<"Ne_Libpng_WriteChunkBkgd"> Ne_Libpng_WriteChunkBkgd{}; // L2 experimental
inline constexpr address<"Ne_Libpng_WriteChunkHeader"> Ne_Libpng_WriteChunkHeader{}; // L2 experimental
inline constexpr address<"Ne_Libpng_WriteChunkHist"> Ne_Libpng_WriteChunkHist{}; // L2 experimental
inline constexpr address<"Ne_Libpng_WriteChunkIccp"> Ne_Libpng_WriteChunkIccp{}; // L2 experimental
inline constexpr address<"Ne_Libpng_WriteChunkPcal"> Ne_Libpng_WriteChunkPcal{}; // L2 experimental
inline constexpr address<"Ne_Libpng_WriteChunkPlte"> Ne_Libpng_WriteChunkPlte{}; // L2 experimental
inline constexpr address<"Ne_Libpng_WriteChunkSbit"> Ne_Libpng_WriteChunkSbit{}; // L2 experimental
inline constexpr address<"Ne_Libpng_WriteChunkSplt"> Ne_Libpng_WriteChunkSplt{}; // L2 experimental
inline constexpr address<"Ne_Libpng_WriteChunkTrns"> Ne_Libpng_WriteChunkTrns{}; // L2 experimental
inline constexpr address<"Ne_Libpng_WriteEXIf"> Ne_Libpng_WriteEXIf{}; // L2 experimental
inline constexpr address<"Ne_Libpng_WriteEnd"> Ne_Libpng_WriteEnd{}; // L2 experimental
inline constexpr address<"Ne_Libpng_WriteITXt"> Ne_Libpng_WriteITXt{}; // L2 experimental
inline constexpr address<"Ne_Libpng_WriteIhdr"> Ne_Libpng_WriteIhdr{}; // L2 experimental
inline constexpr address<"Ne_Libpng_WriteImage"> Ne_Libpng_WriteImage{}; // L2 experimental
inline constexpr address<"Ne_Libpng_WriteInfo"> Ne_Libpng_WriteInfo{}; // L2 experimental
inline constexpr address<"Ne_Libpng_WriteRow"> Ne_Libpng_WriteRow{}; // L2 experimental
inline constexpr address<"Ne_Libpng_WriteTEXt"> Ne_Libpng_WriteTEXt{}; // L2 experimental
inline constexpr address<"Ne_Libpng_WriteZTXt"> Ne_Libpng_WriteZTXt{}; // L2 experimental
inline constexpr address<"Ne_LinkResolver_PushPending"> Ne_LinkResolver_PushPending{}; // L2 experimental
inline constexpr address<"Ne_LinkResolver_RegisterLinkable"> Ne_LinkResolver_RegisterLinkable{}; // L2 experimental
inline constexpr address<"Ne_LinkResolver_ResolveLinks"> Ne_LinkResolver_ResolveLinks{}; // L2 experimental, NeLinkResolver::resolveLinks
inline constexpr address<"Ne_List_PushEntryOrFail"> Ne_List_PushEntryOrFail{}; // L2 experimental
inline constexpr address<"Ne_ListenerHandle_Assign"> Ne_ListenerHandle_Assign{}; // L2 experimental
inline constexpr address<"Ne_ListenerHandle_Unregister"> Ne_ListenerHandle_Unregister{}; // L2 experimental
inline constexpr address<"Ne_Listener_DetachFromOwner"> Ne_Listener_DetachFromOwner{}; // L2 experimental
using Ne_Load_SlUpgradeInterface_t = void();
inline constexpr fn<"Ne_Load_SlUpgradeInterface", Ne_Load_SlUpgradeInterface_t> Ne_Load_SlUpgradeInterface{}; // L2 experimental
using Ne_Load_SystemConfigXml_t = int(void *sys);
inline constexpr fn<"Ne_Load_SystemConfigXml", Ne_Load_SystemConfigXml_t> Ne_Load_SystemConfigXml{}; // L2 experimental
inline constexpr address<"Ne_Loader_RegisterRequest"> Ne_Loader_RegisterRequest{}; // L2 experimental
using Ne_Log_Thunk_t = void(const char *channel, const char *fmt, ...);
inline constexpr fn<"Ne_Log_Thunk", Ne_Log_Thunk_t> Ne_Log_Thunk{}; // L2 experimental
inline constexpr address<"Ne_Log_VirtualHeapBudget_A"> Ne_Log_VirtualHeapBudget_A{}; // L2 experimental
inline constexpr address<"Ne_Log_VirtualHeapBudget_B"> Ne_Log_VirtualHeapBudget_B{}; // L2 experimental
using Ne_Make_RandomIdBlock_t = uint8_t *(uint8_t *out);
inline constexpr fn<"Ne_Make_RandomIdBlock", Ne_Make_RandomIdBlock_t> Ne_Make_RandomIdBlock{}; // L2 experimental
inline constexpr address<"Ne_Make_StrFromBuffer"> Ne_Make_StrFromBuffer{}; // L2 experimental
using Ne_Make_StrFromCStr_t = uint32_t(NeStr **out, const char *s, uint32_t flags, uintptr_t len);
inline constexpr fn<"Ne_Make_StrFromCStr", Ne_Make_StrFromCStr_t> Ne_Make_StrFromCStr{}; // L3 stable
using Ne_Map_CloneNode_String255_t = void *(void **tree, void *src_node, void *parent);
inline constexpr fn<"Ne_Map_CloneNode_String255", Ne_Map_CloneNode_String255_t> Ne_Map_CloneNode_String255{}; // L2 experimental
inline constexpr address<"Ne_Map_DestroySubtree"> Ne_Map_DestroySubtree{}; // L2 experimental
inline constexpr address<"Ne_Map_DestroySubtree_Decals"> Ne_Map_DestroySubtree_Decals{}; // L2 experimental
using Ne_Map_DestroySubtree_RefCounted_t = void(void *a, void *b, void *node);
inline constexpr fn<"Ne_Map_DestroySubtree_RefCounted", Ne_Map_DestroySubtree_RefCounted_t> Ne_Map_DestroySubtree_RefCounted{}; // L2 experimental
inline constexpr address<"Ne_Map_FindOrDefaultWithErrorContext"> Ne_Map_FindOrDefaultWithErrorContext{}; // L2 experimental
inline constexpr address<"Ne_Map_FindOrInsertSlot_StringKey"> Ne_Map_FindOrInsertSlot_StringKey{}; // L2 experimental
using Ne_Map_InsertUnique_StringKey_t = void *(void *map, void *out_it, void *key_node);
inline constexpr fn<"Ne_Map_InsertUnique_StringKey", Ne_Map_InsertUnique_StringKey_t> Ne_Map_InsertUnique_StringKey{}; // L2 experimental
inline constexpr address<"Ne_Map_LowerBoundPairIntInt"> Ne_Map_LowerBoundPairIntInt{}; // L2 experimental
inline constexpr address<"Ne_Math_BasisFromNormal"> Ne_Math_BasisFromNormal{}; // L2 experimental
inline constexpr address<"Ne_Math_Mat4Invert"> Ne_Math_Mat4Invert{}; // L2 experimental
inline constexpr address<"Ne_Math_Mat4Multiply"> Ne_Math_Mat4Multiply{}; // L2 experimental
inline constexpr address<"Ne_Math_Matrix4x4_Invert"> Ne_Math_Matrix4x4_Invert{}; // L2 experimental
inline constexpr address<"Ne_Math_ObbOverlapTest"> Ne_Math_ObbOverlapTest{}; // L2 experimental
inline constexpr address<"Ne_Math_QuatFromEulerXYZ"> Ne_Math_QuatFromEulerXYZ{}; // L2 experimental
inline constexpr address<"Ne_MessageQueue_Alloc"> Ne_MessageQueue_Alloc{}; // L2 experimental
inline constexpr address<"Ne_MipTailGeneratorFactory_ObtainGeneratorResource"> Ne_MipTailGeneratorFactory_ObtainGeneratorResource{}; // L2 experimental, NeMipTailGeneratorFactory::obtainGeneratorResource
inline constexpr address<"Ne_MipTailGenerator_Construct"> Ne_MipTailGenerator_Construct{}; // L2 experimental, NeMipTailGenerator::NeMipTailGenerator
inline constexpr address<"Ne_MipTailGenerator_CreateResources"> Ne_MipTailGenerator_CreateResources{}; // L2 experimental, NeMipTailGenerator::createResources
inline constexpr address<"Ne_MipTailGenerator_Destroy"> Ne_MipTailGenerator_Destroy{}; // L2 experimental, NeMipTailGenerator::~NeMipTailGenerator
inline constexpr address<"Ne_MipTailGenerator_Record"> Ne_MipTailGenerator_Record{}; // L2 experimental, NeMipTailGenerator::record
inline constexpr address<"Ne_ModelInstance_SetNodeMatrices"> Ne_ModelInstance_SetNodeMatrices{}; // L2 experimental
inline constexpr address<"Ne_ModelNodeBinding_Clear"> Ne_ModelNodeBinding_Clear{}; // L2 experimental
inline constexpr address<"Ne_ModelNodeBinding_Init"> Ne_ModelNodeBinding_Init{}; // L2 experimental
inline constexpr address<"Ne_ModuleSystem_CheckConfig"> Ne_ModuleSystem_CheckConfig{}; // L2 experimental
inline constexpr address<"Ne_ModuleSystem_CheckPlatform"> Ne_ModuleSystem_CheckPlatform{}; // L2 experimental
inline constexpr address<"Ne_ModuleSystem_Destroy"> Ne_ModuleSystem_Destroy{}; // L2 experimental
inline constexpr address<"Ne_ModuleSystem_StartModules"> Ne_ModuleSystem_StartModules{}; // L2 experimental
inline constexpr address<"Ne_MoverCollision_BuildMoverShapes"> Ne_MoverCollision_BuildMoverShapes{}; // L2 experimental
inline constexpr address<"Ne_MoverCollision_BuildQueryContext"> Ne_MoverCollision_BuildQueryContext{}; // L2 experimental
inline constexpr address<"Ne_MoverCollision_CreateResource"> Ne_MoverCollision_CreateResource{}; // L2 experimental
inline constexpr address<"Ne_NameRegistry_FindId"> Ne_NameRegistry_FindId{}; // L2 experimental
inline constexpr address<"Ne_NameTable_FindByFnv1a_B"> Ne_NameTable_FindByFnv1a_B{}; // L2 experimental
inline constexpr address<"Ne_NetConnectionQuality_ConnectionArrived"> Ne_NetConnectionQuality_ConnectionArrived{}; // L2 experimental, neon::NeNetConnectionQuality::connectionArrived
inline constexpr address<"Ne_NetConnectionQuality_ConnectionLeft"> Ne_NetConnectionQuality_ConnectionLeft{}; // L2 experimental, neon::NeNetConnectionQuality::connectionLeft
inline constexpr address<"Ne_NetConnectionQuality_OnBandwidthEvalPayload"> Ne_NetConnectionQuality_OnBandwidthEvalPayload{}; // L2 experimental, neon::NeNetConnectionQuality::onBandwidthEvalPayload
inline constexpr address<"Ne_NetConnectionQuality_OnBandwidthEvalResultMessage"> Ne_NetConnectionQuality_OnBandwidthEvalResultMessage{}; // L2 experimental, neon::NeNetConnectionQuality::onBandwidthEvalResultMessage
inline constexpr address<"Ne_NetConnectionQuality_OnBandwidthEvalStartMessage"> Ne_NetConnectionQuality_OnBandwidthEvalStartMessage{}; // L2 experimental, neon::NeNetConnectionQuality::onBandwidthEvalStartMessage
inline constexpr address<"Ne_NetConnectionQuality_OnBandwidthEvalStartReplyMessage"> Ne_NetConnectionQuality_OnBandwidthEvalStartReplyMessage{}; // L2 experimental, neon::NeNetConnectionQuality::onBandwidthEvalStartReplyMessage
inline constexpr address<"Ne_NetConnectionQuality_OnBandwidthTesterStateRequestMessage"> Ne_NetConnectionQuality_OnBandwidthTesterStateRequestMessage{}; // L2 experimental, neon::NeNetConnectionQuality::onBandwidthTesterStateRequestMessage
inline constexpr address<"Ne_NetConnectionQuality_OnBandwidthTesterStateUpdateMessage"> Ne_NetConnectionQuality_OnBandwidthTesterStateUpdateMessage{}; // L2 experimental, neon::NeNetConnectionQuality::onBandwidthTesterStateUpdateMessage
inline constexpr address<"Ne_NetConnectionQuality_OnStatsUpdateMessage"> Ne_NetConnectionQuality_OnStatsUpdateMessage{}; // L2 experimental, neon::NeNetConnectionQuality::onStatsUpdateMessage
inline constexpr address<"Ne_NetConnectionQuality_SendAllStatsUpdates"> Ne_NetConnectionQuality_SendAllStatsUpdates{}; // L2 experimental, neon::NeNetConnectionQuality::sendAllStatsUpdates
inline constexpr address<"Ne_NetConnectionQuality_SendBandwidthTesterStateUpdateMessage"> Ne_NetConnectionQuality_SendBandwidthTesterStateUpdateMessage{}; // L2 experimental, neon::NeNetConnectionQuality::sendBandwidthTesterStateUpdateMessage
inline constexpr address<"Ne_NetConnectionQuality_SendSingleStatsUpdate"> Ne_NetConnectionQuality_SendSingleStatsUpdate{}; // L2 experimental, neon::NeNetConnectionQuality::sendSingleStatsUpdate
inline constexpr address<"Ne_NetConnectionQuality_SetTesterState"> Ne_NetConnectionQuality_SetTesterState{}; // L2 experimental, neon::NeNetConnectionQuality::setTesterState
inline constexpr address<"Ne_NetConnectionStore_Deinitialise"> Ne_NetConnectionStore_Deinitialise{}; // L2 experimental, neon::NeNetConnectionStore::deinitialise
inline constexpr address<"Ne_NetConnectionStore_Dtor"> Ne_NetConnectionStore_Dtor{}; // L2 experimental, neon::NeNetConnectionStore::~NeNetConnectionStore
inline constexpr address<"Ne_NetConnectionStore_Initialise"> Ne_NetConnectionStore_Initialise{}; // L2 experimental, neon::NeNetConnectionStore::initialise
inline constexpr address<"Ne_NetConnectionStore_RemoveConnection"> Ne_NetConnectionStore_RemoveConnection{}; // L2 experimental
inline constexpr address<"Ne_NetHttpRequestHandler_CreateRequest"> Ne_NetHttpRequestHandler_CreateRequest{}; // L2 experimental, NeNetHttpRequestHandler::createRequest
inline constexpr address<"Ne_NetNullSSLPeer_Update"> Ne_NetNullSSLPeer_Update{}; // L2 experimental, neon::NeNetNullSSLPeer::update
inline constexpr address<"Ne_NetReliableAsyncSocket_DeletingDtor"> Ne_NetReliableAsyncSocket_DeletingDtor{}; // L2 experimental, NeNetReliableAsyncSocket::Shutdown
inline constexpr address<"Ne_NetReliableAsyncSocket_DisconnectAll"> Ne_NetReliableAsyncSocket_DisconnectAll{}; // L2 experimental, neon::NeNetReliableAsyncSocket::DisconnectAll
inline constexpr address<"Ne_NetReliableAsyncSocket_OnHandshakeCompleted"> Ne_NetReliableAsyncSocket_OnHandshakeCompleted{}; // L2 experimental, NeNetReliableAsyncSocket::onHandshakeCompleted
inline constexpr address<"Ne_NetReliableAsyncSocket_OnMessageDecrypted"> Ne_NetReliableAsyncSocket_OnMessageDecrypted{}; // L2 experimental, neon::NeNetReliableAsyncSocket::onMessageDecrypted
inline constexpr address<"Ne_NetReliableAsyncSocket_OnMessageEncrypted"> Ne_NetReliableAsyncSocket_OnMessageEncrypted{}; // L2 experimental, neon::NeNetReliableAsyncSocket::onMessageEncrypted
inline constexpr address<"Ne_NetReliableAsyncSocket_Restart"> Ne_NetReliableAsyncSocket_Restart{}; // L2 experimental, NeNetReliableAsyncSocket::Restart
inline constexpr address<"Ne_NetReliableAsyncSocket_SetKeyMaterial"> Ne_NetReliableAsyncSocket_SetKeyMaterial{}; // L2 experimental, neon::NeNetReliableAsyncSocket::setKeyMaterial
inline constexpr address<"Ne_NetSessionF1OS_CallSessionBrowserCreateSession"> Ne_NetSessionF1OS_CallSessionBrowserCreateSession{}; // L2 experimental
inline constexpr address<"Ne_NetSessionF1OS_JoinSession"> Ne_NetSessionF1OS_JoinSession{}; // L2 experimental, neon::NeNetSessionF1OS::joinSession
inline constexpr address<"Ne_NetSessionF1OS_MigrateSessionClient"> Ne_NetSessionF1OS_MigrateSessionClient{}; // L2 experimental, NeNetSessionF1OS::migrateSessionClient
inline constexpr address<"Ne_NetSessionF1OS_MigrateSessionHost"> Ne_NetSessionF1OS_MigrateSessionHost{}; // L2 experimental, neon::NeNetSessionF1OS::migrateSessionHost
inline constexpr address<"Ne_NetSessionF1OS_Reset"> Ne_NetSessionF1OS_Reset{}; // L2 experimental, neon::NeNetSessionF1OS::reset
inline constexpr address<"Ne_NetSessionF1OS_SetCrossplay"> Ne_NetSessionF1OS_SetCrossplay{}; // L2 experimental, neon::NeNetSessionF1OS::setCrossplay
inline constexpr address<"Ne_NetSessionF1OS_SetSessionInfo"> Ne_NetSessionF1OS_SetSessionInfo{}; // L2 experimental
inline constexpr address<"Ne_NetSessionF1OS_SetSessionName"> Ne_NetSessionF1OS_SetSessionName{}; // L2 experimental, neon::NeNetSessionF1OS::setSessionName
inline constexpr address<"Ne_NetSessionF1OS_SetStateCreating"> Ne_NetSessionF1OS_SetStateCreating{}; // L2 experimental
inline constexpr address<"Ne_NetSessionF1OS_UpdateAttributes"> Ne_NetSessionF1OS_UpdateAttributes{}; // L2 experimental
inline constexpr address<"Ne_NetSessionLAN_MigrateSessionClient"> Ne_NetSessionLAN_MigrateSessionClient{}; // L2 experimental, NeNetSessionLAN::migrateSessionClient
inline constexpr address<"Ne_NetSessionLAN_MigrateSessionHost"> Ne_NetSessionLAN_MigrateSessionHost{}; // L2 experimental, NeNetSessionLAN::migrateSessionHost
inline constexpr address<"Ne_NetSessionLAN_OnCreateMigratedSessionFinished"> Ne_NetSessionLAN_OnCreateMigratedSessionFinished{}; // L2 experimental, NeNetSessionLAN::onCreateMigratedSessionFinished
inline constexpr address<"Ne_NetSessionSearcherF1OS_OnSessionBrowserResults"> Ne_NetSessionSearcherF1OS_OnSessionBrowserResults{}; // L2 experimental, neon::NeNetSessionAttribsContainerF1OS::fillFromAdvertisedSessionData
inline constexpr address<"Ne_NetSessionSearcherLAN_SearchForSessions"> Ne_NetSessionSearcherLAN_SearchForSessions{}; // L2 experimental, NeNetSessionSearcherLAN::searchForSessions
inline constexpr address<"Ne_NetSyncClock_IntegrateRemoteHostClock"> Ne_NetSyncClock_IntegrateRemoteHostClock{}; // L2 experimental, NeNetSyncClock::integrateRemoteHostClock
inline constexpr address<"Ne_NetSyncClock_RecalculateClockOffset"> Ne_NetSyncClock_RecalculateClockOffset{}; // L2 experimental, neon::NeNetSyncClock::recalculateClockOffset
inline constexpr address<"Ne_NetSystemClassicMatchmaking_CreateSession"> Ne_NetSystemClassicMatchmaking_CreateSession{}; // L2 experimental, neon::NeNetSystemClassicMatchmaking::createSession
inline constexpr address<"Ne_NetSystemClassicMatchmaking_JoinSession"> Ne_NetSystemClassicMatchmaking_JoinSession{}; // L2 experimental
inline constexpr address<"Ne_NetSystemClassicMatchmaking_MigrateSession"> Ne_NetSystemClassicMatchmaking_MigrateSession{}; // L2 experimental, neon::NeNetSystemClassicMatchmaking::migrateSession
inline constexpr address<"Ne_NetSystemClassicMatchmaking_NotifyPeersMigrationResult"> Ne_NetSystemClassicMatchmaking_NotifyPeersMigrationResult{}; // L2 experimental, neon::NeNetSystemClassicMatchmaking::notifyPeersMigrationResult
inline constexpr address<"Ne_NetSystemClassicMatchmaking_OnCreateSessionComplete"> Ne_NetSystemClassicMatchmaking_OnCreateSessionComplete{}; // L2 experimental, neon::NeNetSystemClassicMatchmaking::onCreateSessionComplete
inline constexpr address<"Ne_NetSystemClassicMatchmaking_OnHandshakingComplete"> Ne_NetSystemClassicMatchmaking_OnHandshakingComplete{}; // L2 experimental, neon::NeNetSystemClassicMatchmaking::onHandshakingComplete
inline constexpr address<"Ne_NetSystemClassicMatchmaking_OnJoinSessionComplete"> Ne_NetSystemClassicMatchmaking_OnJoinSessionComplete{}; // L2 experimental, neon::NeNetSystemClassicMatchmaking::onJoinSessionComplete
inline constexpr address<"Ne_NetSystemClassicMatchmaking_OnMigrateSessionClient"> Ne_NetSystemClassicMatchmaking_OnMigrateSessionClient{}; // L2 experimental, neon::NeNetSystemClassicMatchmaking::onMigrateSessionClient
inline constexpr address<"Ne_NetSystemClassicMatchmaking_OnSessionError"> Ne_NetSystemClassicMatchmaking_OnSessionError{}; // L2 experimental, neon::NeNetSystemClassicMatchmaking::onSessionError
inline constexpr address<"Ne_NetSystemClassicMatchmaking_OnSessionSearchComplete"> Ne_NetSystemClassicMatchmaking_OnSessionSearchComplete{}; // L2 experimental, neon::NeNetSystemClassicMatchmaking::onSessionSearchComplete
inline constexpr address<"Ne_NetSystemClassicMatchmaking_ResetNetworkSystem"> Ne_NetSystemClassicMatchmaking_ResetNetworkSystem{}; // L2 experimental, neon::NeNetSystemClassicMatchmaking::resetNetworkSystem
inline constexpr address<"Ne_NetSystemClassicMatchmaking_ResetNetworkSystemCompleted"> Ne_NetSystemClassicMatchmaking_ResetNetworkSystemCompleted{}; // L2 experimental, neon::NeNetSystemClassicMatchmaking::resetNetworkSystemCompleted
inline constexpr address<"Ne_NetSystemClassicMatchmaking_SearchForSessions"> Ne_NetSystemClassicMatchmaking_SearchForSessions{}; // L2 experimental, neon::NeNetSystemClassicMatchmaking::searchForSessions
inline constexpr address<"Ne_NetSystemCommon_HandleSendToConnection"> Ne_NetSystemCommon_HandleSendToConnection{}; // L2 experimental, neon::NeNetSystemCommon::handleSendToConnection
inline constexpr address<"Ne_NetSystemCommon_LeaveSession"> Ne_NetSystemCommon_LeaveSession{}; // L2 experimental, neon::NeNetSystemCommon::leaveSession
inline constexpr address<"Ne_NetSystemCommon_Migrate"> Ne_NetSystemCommon_Migrate{}; // L2 experimental, neon::NeNetSystemCommon::migrate
inline constexpr address<"Ne_NetSystemCommon_OnConnectionArrived"> Ne_NetSystemCommon_OnConnectionArrived{}; // L2 experimental, neon::NeNetSystemCommon::onConnectionArrived
inline constexpr address<"Ne_NetSystemCommon_ResetNetworkSystem"> Ne_NetSystemCommon_ResetNetworkSystem{}; // L2 experimental, neon::NeNetSystemCommon::resetNetworkSystem
inline constexpr address<"Ne_NetSystemCommon_ResetNetworkSystemCompleted"> Ne_NetSystemCommon_ResetNetworkSystemCompleted{}; // L2 experimental, neon::NeNetSystemCommon::resetNetworkSystemCompleted
inline constexpr address<"Ne_NetSystemCommon_SendKeepAliveToConnections"> Ne_NetSystemCommon_SendKeepAliveToConnections{}; // L2 experimental, neon::NeNetSystemCommon::sendKeepAliveToConnections
inline constexpr address<"Ne_NetSystemHelloMessage_Deserialise"> Ne_NetSystemHelloMessage_Deserialise{}; // L2 experimental, neon::NeNetSystemHelloMessage::deserialise
inline constexpr address<"Ne_NetSystemHelloMessage_Serialise"> Ne_NetSystemHelloMessage_Serialise{}; // L2 experimental, neon::NeNetSystemHelloMessage::serialise
inline constexpr address<"Ne_NetSystemServerHelloResponseMessage_Deserialise"> Ne_NetSystemServerHelloResponseMessage_Deserialise{}; // L2 experimental, neon::NeNetSystemServerHelloResponseMessage::deserialise
inline constexpr address<"Ne_NetSystemServerHelloResponseMessage_Serialise"> Ne_NetSystemServerHelloResponseMessage_Serialise{}; // L2 experimental, neon::NeNetSystemServerHelloResponseMessage::serialise
inline constexpr address<"Ne_NetTransportCommon_AddHostConnectionInfoToConnectionStore"> Ne_NetTransportCommon_AddHostConnectionInfoToConnectionStore{}; // L2 experimental, NeNetTransportCommon::addHostConnectionInfoToConnectionStore
inline constexpr address<"Ne_NetTransportCommon_BroadcastNewConnectionToClients"> Ne_NetTransportCommon_BroadcastNewConnectionToClients{}; // L2 experimental, neon::NeNetTransportCommon::broadcastNewConnectionToClients
inline constexpr address<"Ne_NetTransportCommon_BroadcastRemovedConnectionToClients"> Ne_NetTransportCommon_BroadcastRemovedConnectionToClients{}; // L2 experimental
inline constexpr address<"Ne_NetTransportCommon_FreeConnectionID"> Ne_NetTransportCommon_FreeConnectionID{}; // L2 experimental
inline constexpr address<"Ne_NetTransportCommon_HandleClientHello"> Ne_NetTransportCommon_HandleClientHello{}; // L2 experimental, neon::NeNetTransportCommon::handleClientHello
inline constexpr address<"Ne_NetTransportCommon_HandleServerDisconnection"> Ne_NetTransportCommon_HandleServerDisconnection{}; // L2 experimental
inline constexpr address<"Ne_NetTransportCommon_InitialiseClient"> Ne_NetTransportCommon_InitialiseClient{}; // L2 experimental, neon::NeNetTransportCommon::initialiseClient
inline constexpr address<"Ne_NetTransportCommon_Migrate"> Ne_NetTransportCommon_Migrate{}; // L2 experimental, neon::NeNetTransportCommon::migrate
inline constexpr address<"Ne_NetTransportCommon_ReportClientInitialConnections"> Ne_NetTransportCommon_ReportClientInitialConnections{}; // L2 experimental, neon::NeNetTransportCommon::reportClientInitialConnections
inline constexpr address<"Ne_NetTransportCommon_SetLocalConnectivity"> Ne_NetTransportCommon_SetLocalConnectivity{}; // L2 experimental
inline constexpr address<"Ne_NetTransportCommon_SetMyConnectionId"> Ne_NetTransportCommon_SetMyConnectionId{}; // L2 experimental
inline constexpr address<"Ne_NetTransportCommon_StartHandshaking"> Ne_NetTransportCommon_StartHandshaking{}; // L2 experimental, neon::NeNetTransportCommon::startHandshaking
inline constexpr address<"Ne_NetTransportLAN_OnConnectionLost"> Ne_NetTransportLAN_OnConnectionLost{}; // L2 experimental
inline constexpr address<"Ne_NetTransportLAN_RegisterArrivedConnection"> Ne_NetTransportLAN_RegisterArrivedConnection{}; // L2 experimental, NeNetTransportLAN::registerArrivedConnection
inline constexpr address<"Ne_NetTransportPlayFab_HandlePartyStateChange"> Ne_NetTransportPlayFab_HandlePartyStateChange{}; // L2 experimental, neon::NeNetTransportPlayFab::handlePartyConnectToNetworkCompleted
inline constexpr address<"Ne_NetTransportPlayFab_MigrateConnections"> Ne_NetTransportPlayFab_MigrateConnections{}; // L2 experimental, neon::NeNetTransportPlayFab::migrateConnections
inline constexpr address<"Ne_NetTransportPlayFab_SendTo"> Ne_NetTransportPlayFab_SendTo{}; // L2 experimental, neon::NeNetTransportPlayFab::sendTo
inline constexpr address<"Ne_NetTransportPlayFab_SendToConnectionInfo"> Ne_NetTransportPlayFab_SendToConnectionInfo{}; // L2 experimental, neon::NeNetTransportPlayFab::sendToConnectionInfo
inline constexpr address<"Ne_NetTransport_MigrateConnections"> Ne_NetTransport_MigrateConnections{}; // L2 experimental
inline constexpr address<"Ne_NetVoiceNotificationClient_WaveInAddBuffer"> Ne_NetVoiceNotificationClient_WaveInAddBuffer{}; // L2 experimental, NeNetVoiceNotificationClient::waveInAddBuffer
inline constexpr address<"Ne_Network_Init"> Ne_Network_Init{}; // L2 experimental, neon::NeNetwork::init
inline constexpr address<"Ne_Network_StartAsync"> Ne_Network_StartAsync{}; // L2 experimental, neon::NeNetwork::startAsync
inline constexpr address<"Ne_NodeTable_FindIndexByHash"> Ne_NodeTable_FindIndexByHash{}; // L2 experimental
inline constexpr address<"Ne_Node_DestroyTreeRecursive"> Ne_Node_DestroyTreeRecursive{}; // L2 experimental
inline constexpr address<"Ne_Noise_Fbm1D"> Ne_Noise_Fbm1D{}; // L2 experimental
inline constexpr address<"Ne_OSHTTPRequestAndResponseResponseHandler_CbHttpResponse"> Ne_OSHTTPRequestAndResponseResponseHandler_CbHttpResponse{}; // L2 experimental, neon::NeOSHTTPRequestAndResponse::ResponseHandler::cbHttpResponse
inline constexpr address<"Ne_OSHttpOperation_OnStart"> Ne_OSHttpOperation_OnStart{}; // L2 experimental
inline constexpr address<"Ne_OSHttpRequest_Send"> Ne_OSHttpRequest_Send{}; // L2 experimental
inline constexpr address<"Ne_OSOperationHandle_UnregisterSelf"> Ne_OSOperationHandle_UnregisterSelf{}; // L2 experimental, neon::NeOSOperationHandle::unregisterSelf
inline constexpr address<"Ne_OSOperationQueue_StartPendingOperation"> Ne_OSOperationQueue_StartPendingOperation{}; // L2 experimental, neon::NeOSOperationHandle::registerSelf
inline constexpr address<"Ne_OptionalSharedPtr_Reset"> Ne_OptionalSharedPtr_Reset{}; // L2 experimental
inline constexpr address<"Ne_OptionalVector56_CopyAssign"> Ne_OptionalVector56_CopyAssign{}; // L2 experimental
inline constexpr address<"Ne_ParameterStudio_ReserveInstancePools"> Ne_ParameterStudio_ReserveInstancePools{}; // L2 experimental
inline constexpr address<"Ne_Parse_BoolCString"> Ne_Parse_BoolCString{}; // L2 experimental
inline constexpr address<"Ne_Parse_BoolValue"> Ne_Parse_BoolValue{}; // L2 experimental
using Ne_Parse_Long_t = int32_t(const char *text, char **end, int32_t base);
inline constexpr fn<"Ne_Parse_Long", Ne_Parse_Long_t> Ne_Parse_Long{}; // L2 experimental, strtol
inline constexpr address<"Ne_PathBuf_Construct"> Ne_PathBuf_Construct{}; // L2 experimental
using Ne_PhysBody_ApplyImpulse_t = void(void *body, const float *J, const float *worldPoint);
inline constexpr fn<"Ne_PhysBody_ApplyImpulse", Ne_PhysBody_ApplyImpulse_t> Ne_PhysBody_ApplyImpulse{}; // L2 experimental
inline constexpr address<"Ne_PhysBody_SetAngularVelocity"> Ne_PhysBody_SetAngularVelocity{}; // L2 experimental
inline constexpr address<"Ne_PhysBody_SetLinearVelocity"> Ne_PhysBody_SetLinearVelocity{}; // L2 experimental
inline constexpr address<"Ne_PhysBody_SetTransform"> Ne_PhysBody_SetTransform{}; // L2 experimental
inline constexpr address<"Ne_PhysCollision_FreeIntersectionUnits"> Ne_PhysCollision_FreeIntersectionUnits{}; // L2 experimental
inline constexpr address<"Ne_PhysDeform_BuildTyreWallSoftBody"> Ne_PhysDeform_BuildTyreWallSoftBody{}; // L2 experimental
inline constexpr address<"Ne_PhysMesh_RaycastTriangles"> Ne_PhysMesh_RaycastTriangles{}; // L2 experimental
inline constexpr address<"Ne_PhysPool_AllocZeroed"> Ne_PhysPool_AllocZeroed{}; // L2 experimental
inline constexpr address<"Ne_PhysShape_BuildConvexFromPoints"> Ne_PhysShape_BuildConvexFromPoints{}; // L2 experimental
inline constexpr address<"Ne_PhysShape_CreateConvex"> Ne_PhysShape_CreateConvex{}; // L2 experimental
inline constexpr address<"Ne_PhysShape_InitConvexDefaults"> Ne_PhysShape_InitConvexDefaults{}; // L2 experimental
inline constexpr address<"Ne_PhysUpdateStage_FinishStepNotifyBodies"> Ne_PhysUpdateStage_FinishStepNotifyBodies{}; // L2 experimental
inline constexpr address<"Ne_PhysUpdateStage_FlushSubstepRecords"> Ne_PhysUpdateStage_FlushSubstepRecords{}; // L2 experimental
inline constexpr address<"Ne_PhysUpdateStage_InterpolateBodyStates"> Ne_PhysUpdateStage_InterpolateBodyStates{}; // L2 experimental
inline constexpr address<"Ne_PhysUpdateStage_RunSubsteps"> Ne_PhysUpdateStage_RunSubsteps{}; // L2 experimental
inline constexpr address<"Ne_PhysWorld_AdvanceStepState"> Ne_PhysWorld_AdvanceStepState{}; // L2 experimental
inline constexpr address<"Ne_PhysWorld_ApplyBroadPhaseCcdCollisions"> Ne_PhysWorld_ApplyBroadPhaseCcdCollisions{}; // L2 experimental
inline constexpr address<"Ne_PhysWorld_ApplyBroadPhaseCollisions"> Ne_PhysWorld_ApplyBroadPhaseCollisions{}; // L2 experimental
inline constexpr address<"Ne_PhysWorld_BuildBroadphaseGrid"> Ne_PhysWorld_BuildBroadphaseGrid{}; // L2 experimental
inline constexpr address<"Ne_PhysWorld_CollectBroadphasePairsForBody"> Ne_PhysWorld_CollectBroadphasePairsForBody{}; // L2 experimental
inline constexpr address<"Ne_PhysWorld_CommitBody"> Ne_PhysWorld_CommitBody{}; // L2 experimental
inline constexpr address<"Ne_PhysWorld_CreateConstrainedBody"> Ne_PhysWorld_CreateConstrainedBody{}; // L2 experimental
inline constexpr address<"Ne_PhysWorld_CreateRigidBody"> Ne_PhysWorld_CreateRigidBody{}; // L2 experimental
inline constexpr address<"Ne_PhysWorld_InitFromConfig"> Ne_PhysWorld_InitFromConfig{}; // L2 experimental
inline constexpr address<"Ne_PhysWorld_NotifyBodyStepCallbacks"> Ne_PhysWorld_NotifyBodyStepCallbacks{}; // L2 experimental
inline constexpr address<"Ne_PhysWorld_PostSolveIslands"> Ne_PhysWorld_PostSolveIslands{}; // L2 experimental
inline constexpr address<"Ne_PhysWorld_QueryAabbAnyHit"> Ne_PhysWorld_QueryAabbAnyHit{}; // L2 experimental
inline constexpr address<"Ne_PhysWorld_RaycastClosest"> Ne_PhysWorld_RaycastClosest{}; // L2 experimental
inline constexpr address<"Ne_PhysWorld_ReleaseContainers"> Ne_PhysWorld_ReleaseContainers{}; // L2 experimental
inline constexpr address<"Ne_PhysWorld_ResetFrameState"> Ne_PhysWorld_ResetFrameState{}; // L2 experimental
inline constexpr address<"Ne_PhysWorld_ResizeWorkerContexts"> Ne_PhysWorld_ResizeWorkerContexts{}; // L2 experimental
inline constexpr address<"Ne_PhysWorld_RunIntegratorJobs"> Ne_PhysWorld_RunIntegratorJobs{}; // L2 experimental
inline constexpr address<"Ne_PhysWorld_RunSolveJobs"> Ne_PhysWorld_RunSolveJobs{}; // L2 experimental
inline constexpr address<"Ne_PhysWorld_RunStepPipeline"> Ne_PhysWorld_RunStepPipeline{}; // L2 experimental
inline constexpr address<"Ne_PhysWorld_UpdateBodyBroadphase"> Ne_PhysWorld_UpdateBodyBroadphase{}; // L2 experimental
inline constexpr address<"Ne_Phys_BuildConvexHullShapeBlob"> Ne_Phys_BuildConvexHullShapeBlob{}; // L2 experimental
inline constexpr address<"Ne_Phys_EnsureReflectionRegistered"> Ne_Phys_EnsureReflectionRegistered{}; // L2 experimental
inline constexpr address<"Ne_Phys_GridSegmentTraverse"> Ne_Phys_GridSegmentTraverse{}; // L2 experimental
inline constexpr address<"Ne_Phys_ObbOverlapTest"> Ne_Phys_ObbOverlapTest{}; // L2 experimental
inline constexpr address<"Ne_Phys_ParseMaterials"> Ne_Phys_ParseMaterials{}; // L2 experimental
inline constexpr address<"Ne_Phys_PrepareConstraintRowsBatch"> Ne_Phys_PrepareConstraintRowsBatch{}; // L2 experimental
inline constexpr address<"Ne_Phys_SolveRowsIterate"> Ne_Phys_SolveRowsIterate{}; // L2 experimental
using Ne_Play_FullscreenVideo_t = void(void *feature_system, NeStr **path, const void *on_end, const void *frame_callbacks, int32_t layer);
inline constexpr fn<"Ne_Play_FullscreenVideo", Ne_Play_FullscreenVideo_t> Ne_Play_FullscreenVideo{}; // L2 experimental, neon::NeUIGenericFeatureSystem::playFullscreenVideo
inline constexpr address<"Ne_PlayerCustomInputProvider_BindToDevices"> Ne_PlayerCustomInputProvider_BindToDevices{}; // L2 experimental, NePlayerCustomInputProvider::bindToDevices
inline constexpr address<"Ne_PlayerInputProvider_BindToDeviceNode"> Ne_PlayerInputProvider_BindToDeviceNode{}; // L2 experimental
inline constexpr address<"Ne_PlayerInputProvider_SetDeviceBound"> Ne_PlayerInputProvider_SetDeviceBound{}; // L2 experimental
inline constexpr address<"Ne_PlayerManager_DisplayUnsupportedDeviceWarning"> Ne_PlayerManager_DisplayUnsupportedDeviceWarning{}; // L2 experimental, NePlayerManager::displayUnsupportedDeviceWarning
inline constexpr address<"Ne_PlayerManager_HandleLeadPlayerLogoutOrChangeCallback"> Ne_PlayerManager_HandleLeadPlayerLogoutOrChangeCallback{}; // L2 experimental, NePlayerManager::handleLeadPlayerLogoutOrChangeCallback
inline constexpr address<"Ne_PlayerManager_PlatformProcessAllInputDevices"> Ne_PlayerManager_PlatformProcessAllInputDevices{}; // L2 experimental, NePlayerManager::platformProcessAllInputDevices
inline constexpr address<"Ne_PlayerManager_PlatformTryAssignInputToPlayer"> Ne_PlayerManager_PlatformTryAssignInputToPlayer{}; // L2 experimental, NePlayerManager::platformTryAssignInputToPlayer
inline constexpr address<"Ne_Player_AddPlayerInputProvider"> Ne_Player_AddPlayerInputProvider{}; // L2 experimental, NePlayer::addPlayerInputProvider
inline constexpr address<"Ne_Player_DestroyCustomInputProvider"> Ne_Player_DestroyCustomInputProvider{}; // L2 experimental, NePlayer::destroyCustomInputProvider
inline constexpr address<"Ne_Player_DisableCustomControls"> Ne_Player_DisableCustomControls{}; // L2 experimental, NePlayer::disableCustomControls
inline constexpr address<"Ne_Player_RemovePlayerInputProvider"> Ne_Player_RemovePlayerInputProvider{}; // L2 experimental, NePlayer::removePlayerInputProvider
inline constexpr address<"Ne_Player_SetFFBInputDeviceId"> Ne_Player_SetFFBInputDeviceId{}; // L2 experimental, NePlayer::setFFBInputDeviceId
inline constexpr address<"Ne_PoolAllocator_GrowChunk112"> Ne_PoolAllocator_GrowChunk112{}; // L2 experimental
inline constexpr address<"Ne_PoolAllocator_GrowChunk24"> Ne_PoolAllocator_GrowChunk24{}; // L2 experimental
inline constexpr address<"Ne_PoolAllocator_GrowChunk80"> Ne_PoolAllocator_GrowChunk80{}; // L2 experimental
inline constexpr address<"Ne_PoolList_DestroyNodeChain_A"> Ne_PoolList_DestroyNodeChain_A{}; // L2 experimental
inline constexpr address<"Ne_PoolList_DestroyNodeChain_B"> Ne_PoolList_DestroyNodeChain_B{}; // L2 experimental
inline constexpr address<"Ne_PoolList_DestroyNodes"> Ne_PoolList_DestroyNodes{}; // L2 experimental
inline constexpr address<"Ne_PoolList_Resize"> Ne_PoolList_Resize{}; // L2 experimental
inline constexpr address<"Ne_PoolList_Resize_B"> Ne_PoolList_Resize_B{}; // L2 experimental
inline constexpr address<"Ne_Pool_ComputeBlockRange"> Ne_Pool_ComputeBlockRange{}; // L2 experimental
inline constexpr address<"Ne_Pool_ReleaseVirtualBlock"> Ne_Pool_ReleaseVirtualBlock{}; // L2 experimental
inline constexpr address<"Ne_PostProcessXGenerator_Copy_Render"> Ne_PostProcessXGenerator_Copy_Render{}; // L2 experimental, NePostProcessXGenerator_Copy::updateCrossFade
using Ne_Post_BusMessage_t = uint64_t(void *bus, void *msg);
inline constexpr fn<"Ne_Post_BusMessage", Ne_Post_BusMessage_t> Ne_Post_BusMessage{}; // L2 experimental
inline constexpr address<"Ne_Post_BusMessageQueued_FlashbackUsed"> Ne_Post_BusMessageQueued_FlashbackUsed{}; // L2 experimental
inline constexpr address<"Ne_PresenceEadp_ProcessPlayerCardList"> Ne_PresenceEadp_ProcessPlayerCardList{}; // L2 experimental
inline constexpr address<"Ne_PresenceNameResolverEADP_OnBatchPlayerCardsRequestComplete"> Ne_PresenceNameResolverEADP_OnBatchPlayerCardsRequestComplete{}; // L2 experimental, neon::NePresenceNameResolverEADP::onBatchPlayerCardsRequestComplete
inline constexpr address<"Ne_PresenceNameResolverEADP_UpdateToResolveList"> Ne_PresenceNameResolverEADP_UpdateToResolveList{}; // L2 experimental, neon::NePresenceNameResolverEADP::updateToResolveList
inline constexpr address<"Ne_PresenceNameResolverEADP_UpdateToResolveList_Lambda"> Ne_PresenceNameResolverEADP_UpdateToResolveList_Lambda{}; // L2 experimental, neon::NePresenceNameResolverEADP::updateToResolveList
inline constexpr address<"Ne_PresenceNameResolverEADP_VectorGrow"> Ne_PresenceNameResolverEADP_VectorGrow{}; // L2 experimental
inline constexpr address<"Ne_PresenceNameResolverFixed_InsertName"> Ne_PresenceNameResolverFixed_InsertName{}; // L2 experimental
inline constexpr address<"Ne_PresenceNameResolverFixed_MapFatalStub"> Ne_PresenceNameResolverFixed_MapFatalStub{}; // L2 experimental
inline constexpr address<"Ne_PresenceNameResolverFixed_StartResolving"> Ne_PresenceNameResolverFixed_StartResolving{}; // L2 experimental, neon::NePresenceNameResolverFixed::startResolving
inline constexpr address<"Ne_PresenceQueue_Enqueue"> Ne_PresenceQueue_Enqueue{}; // L2 experimental
inline constexpr address<"Ne_PresenceQueue_VectorPush"> Ne_PresenceQueue_VectorPush{}; // L2 experimental
inline constexpr address<"Ne_Presence_OnSubscribeResult"> Ne_Presence_OnSubscribeResult{}; // L2 experimental
inline constexpr address<"Ne_Property_ResolveBindingChain"> Ne_Property_ResolveBindingChain{}; // L2 experimental
inline constexpr address<"Ne_Property_SetS32IfChanged"> Ne_Property_SetS32IfChanged{}; // L2 experimental
inline constexpr address<"Ne_Property_SetS64IfChanged"> Ne_Property_SetS64IfChanged{}; // L2 experimental
inline constexpr address<"Ne_PtrList_RemoveLocked"> Ne_PtrList_RemoveLocked{}; // L2 experimental
inline constexpr address<"Ne_Queue_EnqueueEventRecord"> Ne_Queue_EnqueueEventRecord{}; // L2 experimental
inline constexpr address<"Ne_RbTree_EraseSubtree"> Ne_RbTree_EraseSubtree{}; // L2 experimental
inline constexpr address<"Ne_RbTree_EraseSubtree_RefValues"> Ne_RbTree_EraseSubtree_RefValues{}; // L2 experimental
inline constexpr address<"Ne_RefCounted_Release"> Ne_RefCounted_Release{}; // L2 experimental
inline constexpr address<"Ne_RefPtr_MoveAssign"> Ne_RefPtr_MoveAssign{}; // L2 experimental
using Ne_Ref_AssignIntrusiveRc16_t = void **(void **dst, void **src);
inline constexpr fn<"Ne_Ref_AssignIntrusiveRc16", Ne_Ref_AssignIntrusiveRc16_t> Ne_Ref_AssignIntrusiveRc16{}; // L2 experimental
inline constexpr address<"Ne_RegisteredHandle_Assign"> Ne_RegisteredHandle_Assign{}; // L2 experimental
inline constexpr address<"Ne_RenderGraph_ApplyBindingsByType"> Ne_RenderGraph_ApplyBindingsByType{}; // L2 experimental
inline constexpr address<"Ne_RenderGraph_BuildPassFlagsDesc"> Ne_RenderGraph_BuildPassFlagsDesc{}; // L2 experimental
inline constexpr address<"Ne_RenderGraph_ResolveHandleFlag"> Ne_RenderGraph_ResolveHandleFlag{}; // L2 experimental
inline constexpr address<"Ne_RenderParams_ApplyGridPreset"> Ne_RenderParams_ApplyGridPreset{}; // L2 experimental
inline constexpr address<"Ne_RenderParams_ApplyLoadingPreset"> Ne_RenderParams_ApplyLoadingPreset{}; // L2 experimental
inline constexpr address<"Ne_RenderParams_ApplyRacePreset"> Ne_RenderParams_ApplyRacePreset{}; // L2 experimental
inline constexpr address<"Ne_RenderParams_ApplyStartLightsPreset"> Ne_RenderParams_ApplyStartLightsPreset{}; // L2 experimental
inline constexpr address<"Ne_RenderViewParams_InitDefaults"> Ne_RenderViewParams_InitDefaults{}; // L2 experimental
inline constexpr address<"Ne_ReplayBuffer_GetOldestValidPosition"> Ne_ReplayBuffer_GetOldestValidPosition{}; // L2 experimental
inline constexpr address<"Ne_ReplayRecordable_ReadFrom"> Ne_ReplayRecordable_ReadFrom{}; // L2 experimental
inline constexpr address<"Ne_ReplaySystem_ForceReset"> Ne_ReplaySystem_ForceReset{}; // L2 experimental
inline constexpr address<"Ne_Replay_SetState"> Ne_Replay_SetState{}; // L2 experimental
inline constexpr address<"Ne_ResourceHandle_Init"> Ne_ResourceHandle_Init{}; // L2 experimental
inline constexpr address<"Ne_ResourceMgr_ReleaseIfOwned"> Ne_ResourceMgr_ReleaseIfOwned{}; // L2 experimental
inline constexpr address<"Ne_ResourceRefGroup_Destruct"> Ne_ResourceRefGroup_Destruct{}; // L2 experimental
inline constexpr address<"Ne_RichText_BuildLocalisedFragmentList"> Ne_RichText_BuildLocalisedFragmentList{}; // L2 experimental
inline constexpr address<"Ne_RichText_ConstructMixedString"> Ne_RichText_ConstructMixedString{}; // L2 experimental
inline constexpr address<"Ne_RingDeque_PushBack_F1os"> Ne_RingDeque_PushBack_F1os{}; // L2 experimental
using Ne_Rng_Mt19937_Refill_t = uint32_t(uint32_t *state);
inline constexpr fn<"Ne_Rng_Mt19937_Refill", Ne_Rng_Mt19937_Refill_t> Ne_Rng_Mt19937_Refill{}; // L2 experimental
inline constexpr address<"Ne_RobinHoodMap12_Rehash"> Ne_RobinHoodMap12_Rehash{}; // L2 experimental
inline constexpr address<"Ne_RobinHoodMap_ClearDestroy"> Ne_RobinHoodMap_ClearDestroy{}; // L2 experimental
inline constexpr address<"Ne_RobinHoodMap_FindKey64Fnv"> Ne_RobinHoodMap_FindKey64Fnv{}; // L2 experimental
inline constexpr address<"Ne_RobinHoodMap_Rehash_A"> Ne_RobinHoodMap_Rehash_A{}; // L2 experimental
inline constexpr address<"Ne_RobinHoodMap_Str_FindIndex8"> Ne_RobinHoodMap_Str_FindIndex8{}; // L2 experimental
inline constexpr address<"Ne_RobinHoodMap_Str_FindIndex_2"> Ne_RobinHoodMap_Str_FindIndex_2{}; // L2 experimental
inline constexpr address<"Ne_RobinHoodMap_U8F_Insert"> Ne_RobinHoodMap_U8F_Insert{}; // L2 experimental
inline constexpr address<"Ne_RttRenderer_CreateSceneJob"> Ne_RttRenderer_CreateSceneJob{}; // L2 experimental
inline constexpr address<"Ne_RttRenderer_FlushJobs"> Ne_RttRenderer_FlushJobs{}; // L2 experimental
inline constexpr address<"Ne_RttRenderer_SubmitJob"> Ne_RttRenderer_SubmitJob{}; // L2 experimental
inline constexpr address<"Ne_Rtt_DefaultCallback"> Ne_Rtt_DefaultCallback{}; // L2 experimental
inline constexpr address<"Ne_Rtt_RegisterCameras"> Ne_Rtt_RegisterCameras{}; // L2 experimental
inline constexpr address<"Ne_Rtt_UnregisterGenerators"> Ne_Rtt_UnregisterGenerators{}; // L2 experimental
inline constexpr address<"Ne_SaveError_CorruptFmtSUU"> Ne_SaveError_CorruptFmtSUU{}; // L2 experimental
inline constexpr address<"Ne_SaveError_EncryptionSourceFailed"> Ne_SaveError_EncryptionSourceFailed{}; // L2 experimental
inline constexpr address<"Ne_SaveError_NotDirectoryFmtSS"> Ne_SaveError_NotDirectoryFmtSS{}; // L2 experimental
inline constexpr address<"Ne_SaveError_SelectedIndexOutOfRange"> Ne_SaveError_SelectedIndexOutOfRange{}; // L2 experimental
inline constexpr address<"Ne_SaveError_SetFmt"> Ne_SaveError_SetFmt{}; // L2 experimental
inline constexpr address<"Ne_SaveError_SetFmtSSS"> Ne_SaveError_SetFmtSSS{}; // L2 experimental
inline constexpr address<"Ne_SaveError_SetFmtWS"> Ne_SaveError_SetFmtWS{}; // L2 experimental
inline constexpr address<"Ne_SaveError_ThumbnailTooLarge"> Ne_SaveError_ThumbnailTooLarge{}; // L2 experimental
inline constexpr address<"Ne_SaveError_TransformFailed"> Ne_SaveError_TransformFailed{}; // L2 experimental
inline constexpr address<"Ne_SaveError_Win32FmtSSSU"> Ne_SaveError_Win32FmtSSSU{}; // L2 experimental
inline constexpr address<"Ne_SaveError_Win32FmtSSU"> Ne_SaveError_Win32FmtSSU{}; // L2 experimental
inline constexpr address<"Ne_SaveError_Win32FmtSU"> Ne_SaveError_Win32FmtSU{}; // L2 experimental
inline constexpr address<"Ne_SaveError_Win32FmtU"> Ne_SaveError_Win32FmtU{}; // L2 experimental
inline constexpr address<"Ne_SaveObject_ReserveChildSlot_14393a770"> Ne_SaveObject_ReserveChildSlot_14393a770{}; // L2 experimental
inline constexpr address<"Ne_SaveSystem2Win_EmptyDirectory"> Ne_SaveSystem2Win_EmptyDirectory{}; // L2 experimental
using Ne_SaveSystem2Win_Mount_t = uint32_t(void *device, void *op, void *save, uint8_t for_saving, uint8_t a5, uint8_t *created);
inline constexpr fn<"Ne_SaveSystem2Win_Mount", Ne_SaveSystem2Win_Mount_t> Ne_SaveSystem2Win_Mount{}; // L2 experimental
inline constexpr address<"Ne_SaveSystem2Win_ProcessSerialiserActions"> Ne_SaveSystem2Win_ProcessSerialiserActions{}; // L2 experimental
inline constexpr address<"Ne_SaveSystem2Win_RenameDirectory"> Ne_SaveSystem2Win_RenameDirectory{}; // L2 experimental
inline constexpr address<"Ne_SaveSystem2Win_SaveSelected"> Ne_SaveSystem2Win_SaveSelected{}; // L2 experimental
using Ne_SaveSystem2Win_Start_t = uint64_t(void *device);
inline constexpr fn<"Ne_SaveSystem2Win_Start", Ne_SaveSystem2Win_Start_t> Ne_SaveSystem2Win_Start{}; // L2 experimental
inline constexpr address<"Ne_SaveSystem2Win_WriteInfoXml"> Ne_SaveSystem2Win_WriteInfoXml{}; // L2 experimental
inline constexpr address<"Ne_SaveSystem_AutoSave"> Ne_SaveSystem_AutoSave{}; // L2 experimental
inline constexpr address<"Ne_SaveSystem_BuildSaveInfoList"> Ne_SaveSystem_BuildSaveInfoList{}; // L2 experimental
inline constexpr address<"Ne_SaveSystem_CreateTempDirectory"> Ne_SaveSystem_CreateTempDirectory{}; // L2 experimental
inline constexpr address<"Ne_SaveSystem_Delete"> Ne_SaveSystem_Delete{}; // L2 experimental
inline constexpr address<"Ne_SaveSystem_DeleteDirectory"> Ne_SaveSystem_DeleteDirectory{}; // L2 experimental
inline constexpr address<"Ne_SaveSystem_Enumerate"> Ne_SaveSystem_Enumerate{}; // L2 experimental
using Ne_SaveSystem_InitDevice_t = int(void *save_system);
inline constexpr fn<"Ne_SaveSystem_InitDevice", Ne_SaveSystem_InitDevice_t> Ne_SaveSystem_InitDevice{}; // L2 experimental
inline constexpr address<"Ne_SaveSystem_ListDelete"> Ne_SaveSystem_ListDelete{}; // L2 experimental
inline constexpr address<"Ne_SaveSystem_ListLoad"> Ne_SaveSystem_ListLoad{}; // L2 experimental
inline constexpr address<"Ne_SaveSystem_ListSave"> Ne_SaveSystem_ListSave{}; // L2 experimental
inline constexpr address<"Ne_SaveSystem_SelectAndDelete"> Ne_SaveSystem_SelectAndDelete{}; // L2 experimental
inline constexpr address<"Ne_SaveSystem_SelectAndLoad"> Ne_SaveSystem_SelectAndLoad{}; // L2 experimental
inline constexpr address<"Ne_SaveSystem_SelectAndSave"> Ne_SaveSystem_SelectAndSave{}; // L2 experimental
inline constexpr address<"Ne_SaveSystem_Terminate"> Ne_SaveSystem_Terminate{}; // L2 experimental
inline constexpr address<"Ne_SaveSystem_ThreadProcessRequest"> Ne_SaveSystem_ThreadProcessRequest{}; // L2 experimental
inline constexpr address<"Ne_SceneNode_SetTransform"> Ne_SceneNode_SetTransform{}; // L2 experimental
inline constexpr address<"Ne_Schema_ParseFieldsA"> Ne_Schema_ParseFieldsA{}; // L2 experimental
inline constexpr address<"Ne_Schema_ParseFieldsB"> Ne_Schema_ParseFieldsB{}; // L2 experimental
inline constexpr address<"Ne_ScriptValue_GetFloat"> Ne_ScriptValue_GetFloat{}; // L2 experimental
inline constexpr address<"Ne_ScriptValue_GetString"> Ne_ScriptValue_GetString{}; // L2 experimental
inline constexpr address<"Ne_SequenceAudioStreamPacket_PreloadAudioStream"> Ne_SequenceAudioStreamPacket_PreloadAudioStream{}; // L2 experimental, NeSequenceAudioStreamPacket::preloadAudioStream
inline constexpr address<"Ne_SequenceClipInstance_BindOptional"> Ne_SequenceClipInstance_BindOptional{}; // L2 experimental, NeSequenceClipInstance::bindOptional
inline constexpr address<"Ne_SequenceInstance_Construct"> Ne_SequenceInstance_Construct{}; // L2 experimental
inline constexpr address<"Ne_SequenceResource_CreateResource"> Ne_SequenceResource_CreateResource{}; // L2 experimental, NeSequenceResource::createResource
inline constexpr address<"Ne_Sequence_Deserialize"> Ne_Sequence_Deserialize{}; // L2 experimental, NeSequence::setName
using Ne_Set_FixedString_t = void(void *dst, const char *src);
inline constexpr fn<"Ne_Set_FixedString", Ne_Set_FixedString_t> Ne_Set_FixedString{}; // L2 experimental
inline constexpr address<"Ne_Settings_BindNamedEntries"> Ne_Settings_BindNamedEntries{}; // L2 experimental
inline constexpr address<"Ne_SharedPtr_CopyAssign"> Ne_SharedPtr_CopyAssign{}; // L2 experimental
inline constexpr address<"Ne_SharedPtr_MoveAssign"> Ne_SharedPtr_MoveAssign{}; // L2 experimental
inline constexpr address<"Ne_SharedPtr_Release"> Ne_SharedPtr_Release{}; // L2 experimental
inline constexpr address<"Ne_SharedPtr_ReleaseA"> Ne_SharedPtr_ReleaseA{}; // L2 experimental
inline constexpr address<"Ne_SharedPtr_ReleaseB"> Ne_SharedPtr_ReleaseB{}; // L2 experimental
inline constexpr address<"Ne_Skeleton_CollectDescendants"> Ne_Skeleton_CollectDescendants{}; // L2 experimental
inline constexpr address<"Ne_Skeleton_ComputeWorldMatrices"> Ne_Skeleton_ComputeWorldMatrices{}; // L2 experimental
inline constexpr address<"Ne_Sort_Elem24"> Ne_Sort_Elem24{}; // L2 experimental
inline constexpr address<"Ne_Sort_Elem32ByU64"> Ne_Sort_Elem32ByU64{}; // L2 experimental
inline constexpr address<"Ne_Sort_Elem40WithPred"> Ne_Sort_Elem40WithPred{}; // L2 experimental
inline constexpr address<"Ne_Sort_Elem48_Median3Order"> Ne_Sort_Elem48_Median3Order{}; // L2 experimental
using Ne_Sort_HeapSortDriverPairs_t = void(int64_t *first, int64_t *last, uint64_t depth);
inline constexpr fn<"Ne_Sort_HeapSortDriverPairs", Ne_Sort_HeapSortDriverPairs_t> Ne_Sort_HeapSortDriverPairs{}; // L2 experimental
inline constexpr address<"Ne_Sort_IntroSort16"> Ne_Sort_IntroSort16{}; // L2 experimental
using Ne_Sort_IntrosortDriverPtrsByKey_t = void(int64_t *first, int64_t *last, uint64_t depth, uint32_t reverse);
inline constexpr fn<"Ne_Sort_IntrosortDriverPtrsByKey", Ne_Sort_IntrosortDriverPtrsByKey_t> Ne_Sort_IntrosortDriverPtrsByKey{}; // L2 experimental
using Ne_Sort_IntrosortU16_t = void(uint16_t *first, uint16_t *last, int64_t depth);
inline constexpr fn<"Ne_Sort_IntrosortU16", Ne_Sort_IntrosortU16_t> Ne_Sort_IntrosortU16{}; // L2 experimental
inline constexpr address<"Ne_Sort_Introsort_Elem48"> Ne_Sort_Introsort_Elem48{}; // L2 experimental
inline constexpr address<"Ne_Sort_Introsort_Pair16_A"> Ne_Sort_Introsort_Pair16_A{}; // L2 experimental
inline constexpr address<"Ne_Sort_Introsort_Pair16_B"> Ne_Sort_Introsort_Pair16_B{}; // L2 experimental
inline constexpr address<"Ne_StdMap_At_141107280"> Ne_StdMap_At_141107280{}; // L2 experimental
inline constexpr address<"Ne_StdMap_At_141403b30"> Ne_StdMap_At_141403b30{}; // L2 experimental
inline constexpr address<"Ne_Std_FatalExceptionTerminate"> Ne_Std_FatalExceptionTerminate{}; // L2 experimental
inline constexpr address<"Ne_Std_ThrowArraySubscript"> Ne_Std_ThrowArraySubscript{}; // L2 experimental
inline constexpr address<"Ne_Std_ThrowBadArrayNewLength"> Ne_Std_ThrowBadArrayNewLength{}; // L2 experimental
inline constexpr address<"Ne_Std_ThrowBadFunctionCall"> Ne_Std_ThrowBadFunctionCall{}; // L2 experimental
inline constexpr address<"Ne_Std_ThrowDequeTooLong"> Ne_Std_ThrowDequeTooLong{}; // L2 experimental
inline constexpr address<"Ne_Std_ThrowException"> Ne_Std_ThrowException{}; // L2 experimental
inline constexpr address<"Ne_Std_ThrowExceptionNoReturn"> Ne_Std_ThrowExceptionNoReturn{}; // L2 experimental
inline constexpr address<"Ne_Std_ThrowInvalidBitsetPosition"> Ne_Std_ThrowInvalidBitsetPosition{}; // L2 experimental
inline constexpr address<"Ne_Std_ThrowMapSetTooLong"> Ne_Std_ThrowMapSetTooLong{}; // L2 experimental
inline constexpr address<"Ne_Std_ThrowStdException"> Ne_Std_ThrowStdException{}; // L2 experimental
inline constexpr address<"Ne_Std_ThrowStringTooLong"> Ne_Std_ThrowStringTooLong{}; // L2 experimental
inline constexpr address<"Ne_Std_ThrowVectorSubscriptError"> Ne_Std_ThrowVectorSubscriptError{}; // L2 experimental
inline constexpr address<"Ne_Std_ThrowVectorTooLong"> Ne_Std_ThrowVectorTooLong{}; // L2 experimental
inline constexpr address<"Ne_Std_ThrowVectorTooLongC"> Ne_Std_ThrowVectorTooLongC{}; // L2 experimental
inline constexpr address<"Ne_Std_ThrowVectorTooLong_14029b1a0"> Ne_Std_ThrowVectorTooLong_14029b1a0{}; // L2 experimental
inline constexpr address<"Ne_SteamTransport_FlushPeerBuffers"> Ne_SteamTransport_FlushPeerBuffers{}; // L2 experimental
inline constexpr address<"Ne_SteamTransport_QueuePacket"> Ne_SteamTransport_QueuePacket{}; // L2 experimental
inline constexpr address<"Ne_Steam_OnEncryptedAppTicket"> Ne_Steam_OnEncryptedAppTicket{}; // L2 experimental
inline constexpr address<"Ne_StrBuf_CopyConstruct"> Ne_StrBuf_CopyConstruct{}; // L2 experimental
inline constexpr address<"Ne_StrBuf_Printf"> Ne_StrBuf_Printf{}; // L2 experimental
inline constexpr address<"Ne_StringMap_EmplaceNode"> Ne_StringMap_EmplaceNode{}; // L2 experimental
inline constexpr address<"Ne_StringPool_InternByHash"> Ne_StringPool_InternByHash{}; // L2 experimental
inline constexpr address<"Ne_StringPool_InternCStr"> Ne_StringPool_InternCStr{}; // L2 experimental
inline constexpr address<"Ne_StringPool_LogMemoryStats"> Ne_StringPool_LogMemoryStats{}; // L2 experimental
using Ne_StringTable_Lookup_t = const char *(void *table, uint64_t hash);
inline constexpr fn<"Ne_StringTable_Lookup", Ne_StringTable_Lookup_t> Ne_StringTable_Lookup{}; // L3 stable
inline constexpr address<"Ne_StringTable_LookupWithFallback"> Ne_StringTable_LookupWithFallback{}; // L2 experimental
inline constexpr address<"Ne_String_AssignWithMode"> Ne_String_AssignWithMode{}; // L2 experimental
inline constexpr address<"Ne_String_CloneAppendCStr"> Ne_String_CloneAppendCStr{}; // L2 experimental
inline constexpr address<"Ne_String_ConstructFromCStr"> Ne_String_ConstructFromCStr{}; // L2 experimental
inline constexpr address<"Ne_String_FormatToPool"> Ne_String_FormatToPool{}; // L2 experimental
inline constexpr address<"Ne_String_ParseTokens"> Ne_String_ParseTokens{}; // L2 experimental
inline constexpr address<"Ne_String_Reserve"> Ne_String_Reserve{}; // L2 experimental
inline constexpr address<"Ne_String_Utf8ToWide"> Ne_String_Utf8ToWide{}; // L2 experimental
inline constexpr address<"Ne_Struct_CopyConstructInlineStrings"> Ne_Struct_CopyConstructInlineStrings{}; // L2 experimental
using Ne_Subscribe_BusMessage_t = void(void *bus, const char *message_name, const void *delegate);
inline constexpr fn<"Ne_Subscribe_BusMessage", Ne_Subscribe_BusMessage_t> Ne_Subscribe_BusMessage{}; // L2 experimental
inline constexpr address<"Ne_Sync_WaitObject"> Ne_Sync_WaitObject{}; // L2 experimental
using Ne_TempPool_Alloc_t = void *(void *pool, uint64_t size, uint64_t align);
inline constexpr fn<"Ne_TempPool_Alloc", Ne_TempPool_Alloc_t> Ne_TempPool_Alloc{}; // L2 experimental
inline constexpr address<"Ne_TerrainResource_GetMaterialResourcel"> Ne_TerrainResource_GetMaterialResourcel{}; // L2 experimental, NeTerrainResource::getMaterialResourcel
inline constexpr address<"Ne_TextRingBuffer_FlushPending"> Ne_TextRingBuffer_FlushPending{}; // L2 experimental
inline constexpr address<"Ne_TextureDecompressionGeneratorFactory_ObtainGeneratorResource"> Ne_TextureDecompressionGeneratorFactory_ObtainGeneratorResource{}; // L2 experimental, NeTextureDecompressionGeneratorFactory::obtainGeneratorResource
inline constexpr address<"Ne_Tga_ConvertToRgba8Buffer"> Ne_Tga_ConvertToRgba8Buffer{}; // L2 experimental
inline constexpr address<"Ne_Thread_Destroy"> Ne_Thread_Destroy{}; // L2 experimental, NeThread::~NeThread
inline constexpr address<"Ne_Thread_SetPriority"> Ne_Thread_SetPriority{}; // L2 experimental
inline constexpr address<"Ne_Thunk_ExternalCall"> Ne_Thunk_ExternalCall{}; // L2 experimental
inline constexpr address<"Ne_TimeManager_OnSetFixedTimeStep"> Ne_TimeManager_OnSetFixedTimeStep{}; // L2 experimental
inline constexpr address<"Ne_Time_BreakDownMicroseconds"> Ne_Time_BreakDownMicroseconds{}; // L2 experimental
inline constexpr address<"Ne_Time_FormatBrokenDownUnits"> Ne_Time_FormatBrokenDownUnits{}; // L2 experimental
inline constexpr address<"Ne_Timer_ConstructQpcDeadline"> Ne_Timer_ConstructQpcDeadline{}; // L2 experimental
inline constexpr address<"Ne_Timer_GetElapsedSeconds"> Ne_Timer_GetElapsedSeconds{}; // L2 experimental
inline constexpr address<"Ne_Tls_GetSlotB5f0Value"> Ne_Tls_GetSlotB5f0Value{}; // L2 experimental
inline constexpr address<"Ne_Tls_GetThreadContext"> Ne_Tls_GetThreadContext{}; // L2 experimental
inline constexpr address<"Ne_Tls_GetValue"> Ne_Tls_GetValue{}; // L2 experimental
inline constexpr address<"Ne_Tweak_FindByName"> Ne_Tweak_FindByName{}; // L2 experimental
inline constexpr address<"Ne_TypedArray2_LogStats_8db0"> Ne_TypedArray2_LogStats_8db0{}; // L2 experimental
inline constexpr address<"Ne_U64Map_FindValue_At580"> Ne_U64Map_FindValue_At580{}; // L2 experimental
inline constexpr address<"Ne_UIAction_C_closeScreenId"> Ne_UIAction_C_closeScreenId{}; // L2 experimental, NeUIAction::c_closeScreenId
inline constexpr address<"Ne_UICrashDumpInfo_OnScreenEvent"> Ne_UICrashDumpInfo_OnScreenEvent{}; // L2 experimental
inline constexpr address<"Ne_UICrashDumpInfo_SettingsScreenClearSelectedCategory"> Ne_UICrashDumpInfo_SettingsScreenClearSelectedCategory{}; // L2 experimental
inline constexpr address<"Ne_UIElementBase_HandleCommunicationMessage"> Ne_UIElementBase_HandleCommunicationMessage{}; // L2 experimental, NeUIElementBase::NeUICommunicationMessageType
inline constexpr address<"Ne_UIFlow_AssetGroupPreUnloaded"> Ne_UIFlow_AssetGroupPreUnloaded{}; // L2 experimental, NeUIFlow::assetGroupPreUnloaded
inline constexpr address<"Ne_UIFlow_AssetGroupPreloaded"> Ne_UIFlow_AssetGroupPreloaded{}; // L2 experimental, NeUIFlow::assetGroupPreloaded
inline constexpr address<"Ne_UIFlow_FlowAssetsUnloaded"> Ne_UIFlow_FlowAssetsUnloaded{}; // L2 experimental, NeUIFlow::flowAssetsUnloaded
inline constexpr address<"Ne_UIFlow_UnloadAssetsGroups"> Ne_UIFlow_UnloadAssetsGroups{}; // L2 experimental, NeUIFlow::unloadAssetsGroups
inline constexpr address<"Ne_UIInterrupt_HandleSignal"> Ne_UIInterrupt_HandleSignal{}; // L2 experimental, NeUIInterrupt::handleSignal
inline constexpr address<"Ne_UIMessageSystem_OnMetadataAssetCleanup"> Ne_UIMessageSystem_OnMetadataAssetCleanup{}; // L2 experimental, neon::NeUIMessageSystem::onMetadataAssetCleanup
inline constexpr address<"Ne_UINodeBase_OnElementNodeBind"> Ne_UINodeBase_OnElementNodeBind{}; // L2 experimental, NeUINodeBase::onElementNodeBind
inline constexpr address<"Ne_UINodeBase_OnElementNodeUnBind"> Ne_UINodeBase_OnElementNodeUnBind{}; // L2 experimental, NeUINodeBase::onElementNodeUnBind
inline constexpr address<"Ne_UINodeBase_OnVMSetDataContext"> Ne_UINodeBase_OnVMSetDataContext{}; // L2 experimental, NeUINodeBase::NeUICommunicationMessageType
inline constexpr address<"Ne_UINodeBase_ParentVisible"> Ne_UINodeBase_ParentVisible{}; // L2 experimental, NeUINodeBase::parentVisible
inline constexpr address<"Ne_UINodeBase_SetElementVisibility"> Ne_UINodeBase_SetElementVisibility{}; // L2 experimental, NeUINodeBase::setElementVisibility
inline constexpr address<"Ne_UINodeBase_Visible"> Ne_UINodeBase_Visible{}; // L2 experimental, NeUINodeBase::visible
inline constexpr address<"Ne_UIPCOptions_PopulateOutputMonitors"> Ne_UIPCOptions_PopulateOutputMonitors{}; // L2 experimental, NeUIPCOptions::populateOutputMonitors
inline constexpr address<"Ne_UITextElement_SetText"> Ne_UITextElement_SetText{}; // L2 experimental, NeUITextElement::setText
inline constexpr address<"Ne_UITextElement_UpdateDocument"> Ne_UITextElement_UpdateDocument{}; // L2 experimental, NeUITextElement::updateDocument
inline constexpr address<"Ne_UIVideo_SendSamplerMessage"> Ne_UIVideo_SendSamplerMessage{}; // L2 experimental
inline constexpr address<"Ne_UIVideo_SetLayer"> Ne_UIVideo_SetLayer{}; // L2 experimental
inline constexpr address<"Ne_UIVideo_SetSource"> Ne_UIVideo_SetSource{}; // L2 experimental
inline constexpr address<"Ne_UgcSearcher_SearchCallback"> Ne_UgcSearcher_SearchCallback{}; // L2 experimental
inline constexpr address<"Ne_UgcSearcher_StartSearch"> Ne_UgcSearcher_StartSearch{}; // L2 experimental
inline constexpr address<"Ne_UgcUtil_BrowseWorkshopInSteamOverlay"> Ne_UgcUtil_BrowseWorkshopInSteamOverlay{}; // L2 experimental
inline constexpr address<"Ne_UgcUtil_OpenWorkshopFileInSteamOverlay"> Ne_UgcUtil_OpenWorkshopFileInSteamOverlay{}; // L2 experimental
inline constexpr address<"Ne_Ugc_DeleteTemporaryFiles"> Ne_Ugc_DeleteTemporaryFiles{}; // L2 experimental
inline constexpr address<"Ne_Ugc_GameOverlayActivatedCallback"> Ne_Ugc_GameOverlayActivatedCallback{}; // L2 experimental
inline constexpr address<"Ne_Ugc_ReadInstalledItem"> Ne_Ugc_ReadInstalledItem{}; // L2 experimental
inline constexpr address<"Ne_Ugc_UpdateItemCallback"> Ne_Ugc_UpdateItemCallback{}; // L2 experimental
inline constexpr address<"Ne_UiEvent_DispatchByName"> Ne_UiEvent_DispatchByName{}; // L2 experimental
inline constexpr address<"Ne_UniquePtr_MoveAssign"> Ne_UniquePtr_MoveAssign{}; // L2 experimental
inline constexpr address<"Ne_UnorderedMap_FindOrEmplace_Fnv8_A"> Ne_UnorderedMap_FindOrEmplace_Fnv8_A{}; // L2 experimental
inline constexpr address<"Ne_UnorderedMap_FindOrEmplace_Fnv8_B"> Ne_UnorderedMap_FindOrEmplace_Fnv8_B{}; // L2 experimental
inline constexpr address<"Ne_UnorderedMap_FindOrEmplace_Fnv8_C"> Ne_UnorderedMap_FindOrEmplace_Fnv8_C{}; // L2 experimental
inline constexpr address<"Ne_UnorderedMap_FindOrEmplace_Fnv8_D"> Ne_UnorderedMap_FindOrEmplace_Fnv8_D{}; // L2 experimental
inline constexpr address<"Ne_UnorderedMap_FindOrEmplace_Fnv_E"> Ne_UnorderedMap_FindOrEmplace_Fnv_E{}; // L2 experimental
inline constexpr address<"Ne_UnorderedMap_RehashBuckets"> Ne_UnorderedMap_RehashBuckets{}; // L2 experimental
inline constexpr address<"Ne_UnorderedMap_Rehash_Gfx2Buffer"> Ne_UnorderedMap_Rehash_Gfx2Buffer{}; // L2 experimental
inline constexpr address<"Ne_UnorderedMap_Rehash_Gfx2DeferredCommand"> Ne_UnorderedMap_Rehash_Gfx2DeferredCommand{}; // L2 experimental
inline constexpr address<"Ne_UserId_Equals"> Ne_UserId_Equals{}; // L2 experimental
inline constexpr address<"Ne_UserId_ToDisplayString"> Ne_UserId_ToDisplayString{}; // L2 experimental
inline constexpr address<"Ne_Variant_FormatToString"> Ne_Variant_FormatToString{}; // L2 experimental
inline constexpr address<"Ne_Vector16_InsertGrow"> Ne_Vector16_InsertGrow{}; // L2 experimental
inline constexpr address<"Ne_Vector16_PushBack"> Ne_Vector16_PushBack{}; // L2 experimental
inline constexpr address<"Ne_Vector24_PushBackMove"> Ne_Vector24_PushBackMove{}; // L2 experimental
inline constexpr address<"Ne_Vector24_PushBackMove_B"> Ne_Vector24_PushBackMove_B{}; // L2 experimental
inline constexpr address<"Ne_Vector2A8_PushBackGrow"> Ne_Vector2A8_PushBackGrow{}; // L2 experimental
inline constexpr address<"Ne_Vector80_EmplaceBack"> Ne_Vector80_EmplaceBack{}; // L2 experimental
inline constexpr address<"Ne_Vector88_CopyAssign"> Ne_Vector88_CopyAssign{}; // L2 experimental
inline constexpr address<"Ne_Vector96_Resize"> Ne_Vector96_Resize{}; // L2 experimental
inline constexpr address<"Ne_VectorOfFloatVec_Assign"> Ne_VectorOfFloatVec_Assign{}; // L2 experimental
using Ne_Vector_CopyConstruct_NamedEntry28_t = void *(void *dst, void *src);
inline constexpr fn<"Ne_Vector_CopyConstruct_NamedEntry28", Ne_Vector_CopyConstruct_NamedEntry28_t> Ne_Vector_CopyConstruct_NamedEntry28{}; // L2 experimental
inline constexpr address<"Ne_Vector_GrowEmplace_Elem384"> Ne_Vector_GrowEmplace_Elem384{}; // L2 experimental
inline constexpr address<"Ne_Vector_InsertGrow_Elem40"> Ne_Vector_InsertGrow_Elem40{}; // L2 experimental
inline constexpr address<"Ne_Vector_PushBack_Elem16"> Ne_Vector_PushBack_Elem16{}; // L2 experimental
using Ne_Vector_PushBack_Elem64_WithCallback_t = uint64_t(void **vec, void *elem);
inline constexpr fn<"Ne_Vector_PushBack_Elem64_WithCallback", Ne_Vector_PushBack_Elem64_WithCallback_t> Ne_Vector_PushBack_Elem64_WithCallback{}; // L2 experimental
inline constexpr address<"Ne_Vector_PushBack_Ptr_Locked"> Ne_Vector_PushBack_Ptr_Locked{}; // L2 experimental
inline constexpr address<"Ne_Vector_ResizeIntrusivePtrs"> Ne_Vector_ResizeIntrusivePtrs{}; // L2 experimental
inline constexpr address<"Ne_VideoInstance_Close"> Ne_VideoInstance_Close{}; // L2 experimental
inline constexpr address<"Ne_VideoManager_OpenVideo"> Ne_VideoManager_OpenVideo{}; // L2 experimental
inline constexpr address<"Ne_VideoManager_PauseVideo"> Ne_VideoManager_PauseVideo{}; // L2 experimental
inline constexpr address<"Ne_VideoManager_PlayIntervalFrames"> Ne_VideoManager_PlayIntervalFrames{}; // L2 experimental
inline constexpr address<"Ne_VideoManager_PlayIntervalSeconds"> Ne_VideoManager_PlayIntervalSeconds{}; // L2 experimental
inline constexpr address<"Ne_VideoManager_PlayOneFrame"> Ne_VideoManager_PlayOneFrame{}; // L2 experimental
inline constexpr address<"Ne_VideoManager_PlayVideo"> Ne_VideoManager_PlayVideo{}; // L2 experimental
inline constexpr address<"Ne_VideoManager_ResumeVideo"> Ne_VideoManager_ResumeVideo{}; // L2 experimental
inline constexpr address<"Ne_VideoManager_StopVideo"> Ne_VideoManager_StopVideo{}; // L2 experimental
inline constexpr address<"Ne_VideoPlayer_OpenFile"> Ne_VideoPlayer_OpenFile{}; // L2 experimental
inline constexpr address<"Ne_VideoResource_Destroy"> Ne_VideoResource_Destroy{}; // L2 experimental, NeVideoResource::~NeVideoResource
inline constexpr address<"Ne_Video_InitBinkThreads"> Ne_Video_InitBinkThreads{}; // L2 experimental
inline constexpr address<"Ne_VmNode_SetFloat"> Ne_VmNode_SetFloat{}; // L2 experimental
inline constexpr address<"Ne_Vsnprintf"> Ne_Vsnprintf{}; // L2 experimental
inline constexpr address<"Ne_WindowWin_BuildResolutionOptions"> Ne_WindowWin_BuildResolutionOptions{}; // L2 experimental, NeWindowWin::getMonitorDimensionsByIndex
inline constexpr address<"Ne_WindowWin_MonitorEnumProc"> Ne_WindowWin_MonitorEnumProc{}; // L2 experimental, NeWindowWin::monitorEnumProc
inline constexpr address<"Ne_WindowWin_ResizeTarget"> Ne_WindowWin_ResizeTarget{}; // L2 experimental, NeWindowWin::resizeTarget
inline constexpr address<"Ne_WindowWin_SaveWindowPosition"> Ne_WindowWin_SaveWindowPosition{}; // L2 experimental, NeWindowWin::saveWindowPosition
inline constexpr address<"Ne_WindowWin_SetAspectRatio"> Ne_WindowWin_SetAspectRatio{}; // L2 experimental, NeWindowWin::setAspectRatio
inline constexpr address<"Ne_WindowWin_SetOnMinimised"> Ne_WindowWin_SetOnMinimised{}; // L2 experimental, NeWindowWin::setOnMinimised
inline constexpr address<"Ne_WindowWin_Startup"> Ne_WindowWin_Startup{}; // L2 experimental, NeWindowWin::startup
inline constexpr address<"Ne_WindowWin_ToggleFullScreen"> Ne_WindowWin_ToggleFullScreen{}; // L2 experimental, NeWindowWin::toggleFullScreen
inline constexpr address<"Ne_WindowWin_Update"> Ne_WindowWin_Update{}; // L2 experimental, NeWindowWin::update
inline constexpr address<"Ne_WindowWin_WinProc"> Ne_WindowWin_WinProc{}; // L2 experimental, NeWindowWin::winProc
inline constexpr address<"Ne_WorkQueue_PushAndSignal"> Ne_WorkQueue_PushAndSignal{}; // L2 experimental
inline constexpr address<"Ne_WorldInstance_ActivateAllWorldObjects"> Ne_WorldInstance_ActivateAllWorldObjects{}; // L2 experimental
inline constexpr address<"Ne_WorldInstance_DeactivateAllWorldObjects"> Ne_WorldInstance_DeactivateAllWorldObjects{}; // L2 experimental
inline constexpr address<"Ne_WorldInstance_DestroyAllWorldObjectsInLayer"> Ne_WorldInstance_DestroyAllWorldObjectsInLayer{}; // L2 experimental
inline constexpr address<"Ne_WorldInstance_PreWarmTextures"> Ne_WorldInstance_PreWarmTextures{}; // L2 experimental
inline constexpr address<"Ne_WorldInstance_ReleaseWarmedTextures"> Ne_WorldInstance_ReleaseWarmedTextures{}; // L2 experimental
inline constexpr address<"Ne_WorldLayerModule_DeregisterLayer"> Ne_WorldLayerModule_DeregisterLayer{}; // L2 experimental, NeWorldLayerModule::deregisterLayer
inline constexpr address<"Ne_WorldLoadTask_Update"> Ne_WorldLoadTask_Update{}; // L2 experimental
inline constexpr address<"Ne_Wwise_ConvolutionReverb_Init"> Ne_Wwise_ConvolutionReverb_Init{}; // L2 experimental
using Ne_Wwise_ConvolutionReverb_InitImpl_t = uint64_t(void *fx, uint32_t a2, uint32_t a3);
inline constexpr fn<"Ne_Wwise_ConvolutionReverb_InitImpl", Ne_Wwise_ConvolutionReverb_InitImpl_t> Ne_Wwise_ConvolutionReverb_InitImpl{}; // L2 experimental
inline constexpr data<"g_Ne_DefaultAllocator", void *> g_Ne_DefaultAllocator{}; // L2 experimental
inline constexpr data<"g_Ne_InlineStr128Vtable", void *> g_Ne_InlineStr128Vtable{}; // L2 experimental
inline constexpr data<"g_Ne_LogGate", NeLogGate> g_Ne_LogGate{}; // L2 experimental
inline constexpr data<"g_Ne_NameTable", void *> g_Ne_NameTable{}; // L2 experimental
inline constexpr data<"g_Ne_StringTable", void *> g_Ne_StringTable{}; // L3 stable
inline constexpr vtable<"k_Ne_BagCodecVtbl", 4> k_Ne_BagCodecVtbl{}; // L3 stable
inline constexpr vtable<"k_Ne_CountingStreamVtbl", 24> k_Ne_CountingStreamVtbl{}; // L3 stable
inline constexpr vtable<"k_Ne_MemWriteStreamVtbl", 33> k_Ne_MemWriteStreamVtbl{}; // L3 stable
inline constexpr data<"k_Ne_PropertyType_Bag", void *> k_Ne_PropertyType_Bag{}; // L3 stable
inline constexpr data<"k_Ne_TypeChainEnd", void *> k_Ne_TypeChainEnd{}; // L2 experimental
inline constexpr data<"k_Ne_TypeRoot", void *> k_Ne_TypeRoot{}; // L2 experimental, NoOtiInfo.
inline constexpr site<"site_Ne_Format_String_Entry"> site_Ne_Format_String_Entry{}; // L2 experimental
inline constexpr call<"site_Ne_HwBool_CrowdInShadows"> site_Ne_HwBool_CrowdInShadows{}; // L2 experimental
inline constexpr call<"site_Ne_HwBool_GodRaysPost"> site_Ne_HwBool_GodRaysPost{}; // L2 experimental
inline constexpr call<"site_Ne_HwBool_GrassFlattening"> site_Ne_HwBool_GrassFlattening{}; // L2 experimental
inline constexpr call<"site_Ne_HwBool_GroundCoverEnabled"> site_Ne_HwBool_GroundCoverEnabled{}; // L2 experimental
inline constexpr call<"site_Ne_HwBool_GroundCoverMeshes"> site_Ne_HwBool_GroundCoverMeshes{}; // L2 experimental
inline constexpr call<"site_Ne_HwBool_LensStreak"> site_Ne_HwBool_LensStreak{}; // L2 experimental
inline constexpr call<"site_Ne_HwBool_MotionBlurV2"> site_Ne_HwBool_MotionBlurV2{}; // L2 experimental
inline constexpr call<"site_Ne_HwBool_NightShadowCharCast"> site_Ne_HwBool_NightShadowCharCast{}; // L2 experimental
inline constexpr call<"site_Ne_HwBool_RainBeads"> site_Ne_HwBool_RainBeads{}; // L2 experimental
inline constexpr call<"site_Ne_HwBool_RainHigh"> site_Ne_HwBool_RainHigh{}; // L2 experimental
inline constexpr call<"site_Ne_HwBool_RainSheets"> site_Ne_HwBool_RainSheets{}; // L2 experimental
inline constexpr call<"site_Ne_HwBool_RainSplashes"> site_Ne_HwBool_RainSplashes{}; // L2 experimental
inline constexpr call<"site_Ne_HwBool_SkyShadowLowLod"> site_Ne_HwBool_SkyShadowLowLod{}; // L2 experimental
inline constexpr call<"site_Ne_HwBool_SkyShadowMap32Bits"> site_Ne_HwBool_SkyShadowMap32Bits{}; // L2 experimental
inline constexpr call<"site_Ne_HwBool_TrackHeatHaze"> site_Ne_HwBool_TrackHeatHaze{}; // L2 experimental
inline constexpr call<"site_Ne_HwBool_VolumetricFog"> site_Ne_HwBool_VolumetricFog{}; // L2 experimental
inline constexpr call<"site_Ne_HwConfigParseCall"> site_Ne_HwConfigParseCall{}; // L2 experimental
inline constexpr call<"site_Ne_HwInt_CrowdBillboardDistance"> site_Ne_HwInt_CrowdBillboardDistance{}; // L2 experimental
inline constexpr call<"site_Ne_HwInt_NightShadowMapSize"> site_Ne_HwInt_NightShadowMapSize{}; // L2 experimental
inline constexpr call<"site_Ne_HwInt_NightShadowSourceCount"> site_Ne_HwInt_NightShadowSourceCount{}; // L2 experimental
inline constexpr call<"site_Ne_HwInt_ShadowSampling"> site_Ne_HwInt_ShadowSampling{}; // L2 experimental
inline constexpr call<"site_Ne_HwInt_SkyShadowCascadeCount"> site_Ne_HwInt_SkyShadowCascadeCount{}; // L2 experimental
inline constexpr call<"site_Ne_HwInt_SkyShadowMapSize"> site_Ne_HwInt_SkyShadowMapSize{}; // L2 experimental
inline constexpr call<"site_Ne_HwInt_SpotShadowMapSize"> site_Ne_HwInt_SpotShadowMapSize{}; // L2 experimental
inline constexpr call<"site_Ne_HwInt_SpotShadowSampling"> site_Ne_HwInt_SpotShadowSampling{}; // L2 experimental
inline constexpr call<"site_Ne_HwInt_TexStreamSizeInMiB"> site_Ne_HwInt_TexStreamSizeInMiB{}; // L2 experimental
inline constexpr call<"site_Ne_HwInt_VolFogMapHeight"> site_Ne_HwInt_VolFogMapHeight{}; // L2 experimental
inline constexpr call<"site_Ne_HwInt_VolFogMapWidth"> site_Ne_HwInt_VolFogMapWidth{}; // L2 experimental
inline constexpr call<"site_Ne_HwInt_WorldShadowMapSize"> site_Ne_HwInt_WorldShadowMapSize{}; // L2 experimental
inline constexpr call<"site_Ne_HwXmlParseCall"> site_Ne_HwXmlParseCall{}; // L2 experimental
inline constexpr call<"site_Ne_HwXmlWriteCall"> site_Ne_HwXmlWriteCall{}; // L2 experimental
inline constexpr site<"site_Ne_MemoryMapHeapSize"> site_Ne_MemoryMapHeapSize{}; // L2 experimental
inline constexpr site<"site_Ne_MemoryMapInternalHeapSize"> site_Ne_MemoryMapInternalHeapSize{}; // L2 experimental
inline constexpr site<"site_Ne_ReplayAfterRecord"> site_Ne_ReplayAfterRecord{}; // L2 experimental
inline constexpr site<"site_Ne_WatchdogKillPath"> site_Ne_WatchdogKillPath{}; // L2 experimental
inline constexpr site<"site_Ne_WatchdogKillSkip"> site_Ne_WatchdogKillSkip{}; // L2 experimental
inline constexpr vtable<"vt_Ne_ParticleEffectComponent", 16> vt_Ne_ParticleEffectComponent{}; // L2 experimental
inline constexpr vtable<"vt_Ne_SaveSystem2Win", 16> vt_Ne_SaveSystem2Win{}; // L2 experimental

}
