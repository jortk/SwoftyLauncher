#pragma once

#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <string_view>
#include <vector>

// D3D12 Device Removed Extended Data (DRED) for crash reports. The pure parts: when DRED is switched on, which
// register values mean the GPU was removed, and how breadcrumb and page fault records are worded. dred.cpp does
// the D3D12 calls.

namespace runtime::dred {

constexpr int kMaxOps = 64;
constexpr int kMaxNodes = 32;
constexpr int kMaxAllocations = 32;
constexpr int kMaxChain = 4096;

struct Decision {
    bool on = false;
    const char *why = "";
};

inline Decision decide(const char *env, bool prior_removed)
{
    if (env && env[0] == '1' && !env[1])
        return {true, "SWOFTY_DRED=1"};
    if (env && env[0] == '0' && !env[1])
        return {false, "SWOFTY_DRED=0"};
    if (prior_removed)
        return {true, "the previous session ended with a removed GPU device"};
    return {true, "on by default, so the first GPU hang already has breadcrumbs"};
}

inline bool is_removal_hresult(uint64_t v)
{
    uint32_t h = uint32_t(v);
    if ((v >> 32) != 0 && (v >> 32) != 0xffffffffull)
        return false;
    return h == 0x887a0001 || h == 0x887a0005 || h == 0x887a0006 || h == 0x887a0007 || h == 0x887a0020;
}

inline const char *removal_name(uint32_t h)
{
    switch (h) {
    case 0x887a0001: return "DXGI_ERROR_INVALID_CALL";
    case 0x887a0005: return "DXGI_ERROR_DEVICE_REMOVED";
    case 0x887a0006: return "DXGI_ERROR_DEVICE_HUNG";
    case 0x887a0007: return "DXGI_ERROR_DEVICE_RESET";
    case 0x887a0020: return "DXGI_ERROR_DRIVER_INTERNAL_ERROR";
    default: return "unknown";
    }
}

inline const char *op_name(uint32_t op)
{
    static const char *const names[] = {
        "SETMARKER", "BEGINEVENT", "ENDEVENT", "DRAWINSTANCED", "DRAWINDEXEDINSTANCED", "EXECUTEINDIRECT", "DISPATCH",
        "COPYBUFFERREGION", "COPYTEXTUREREGION", "COPYRESOURCE", "COPYTILES", "RESOLVESUBRESOURCE", "CLEARRENDERTARGETVIEW",
        "CLEARUNORDEREDACCESSVIEW", "CLEARDEPTHSTENCILVIEW", "RESOURCEBARRIER", "EXECUTEBUNDLE", "PRESENT",
        "RESOLVEQUERYDATA", "BEGINSUBMISSION", "ENDSUBMISSION", "DECODEFRAME", "PROCESSFRAMES", "ATOMICCOPYBUFFERUINT",
        "ATOMICCOPYBUFFERUINT64", "RESOLVESUBRESOURCEREGION", "WRITEBUFFERIMMEDIATE", "DECODEFRAME1", "SETPROTECTEDRESOURCESESSION",
        "DECODEFRAME2", "PROCESSFRAMES1", "BUILDRAYTRACINGACCELERATIONSTRUCTURE", "EMITRAYTRACINGACCELERATIONSTRUCTUREPOSTBUILDINFO",
        "COPYRAYTRACINGACCELERATIONSTRUCTURE", "DISPATCHRAYS", "INITIALIZEMETACOMMAND", "EXECUTEMETACOMMAND", "ESTIMATEMOTION",
        "RESOLVEMOTIONVECTORHEAP", "SETPIPELINESTATE1", "INITIALIZEEXTENSIONCOMMAND", "EXECUTEEXTENSIONCOMMAND", "DISPATCHMESH",
        "ENCODEFRAME", "RESOLVEENCODEROUTPUTMETADATA"};
    return op < sizeof names / sizeof *names ? names[op] : "UNKNOWN_OP";
}

inline const char *allocation_type_name(uint32_t t)
{
    switch (t) {
    case 0x13: return "COMMAND_QUEUE";
    case 0x14: return "COMMAND_ALLOCATOR";
    case 0x15: return "PIPELINE_STATE";
    case 0x16: return "COMMAND_LIST";
    case 0x17: return "FENCE";
    case 0x18: return "DESCRIPTOR_HEAP";
    case 0x19: return "HEAP";
    case 0x1b: return "QUERY_HEAP";
    case 0x1c: return "COMMAND_SIGNATURE";
    case 0x1d: return "PIPELINE_LIBRARY";
    case 0x22: return "RESOURCE";
    case 0x23: return "PASS";
    case 0x2a: return "STATE_OBJECT";
    case 0x2b: return "METACOMMAND";
    default: return "OTHER";
    }
}

inline bool node_incomplete(uint32_t count, uint32_t completed)
{
    return completed < count;
}

struct Window {
    uint32_t from = 0, to = 0;
};

inline Window op_window(uint32_t count, uint32_t completed)
{
    Window w;
    uint32_t keep = kMaxOps;
    uint32_t anchor = completed > count ? count : completed;
    w.from = anchor > keep * 3 / 4 ? anchor - keep * 3 / 4 : 0;
    w.to = w.from + keep < count ? w.from + keep : count;
    return w;
}

inline std::string op_line(uint32_t index, uint32_t completed, uint32_t op)
{
    char b[160];
    const char *mark = index + 1 == completed ? "  <- last completed" : index == completed ? "  <- first not completed" : "";
    std::snprintf(b, sizeof b, "      %5u %-4s %s%s\n", index, index < completed ? "done" : "todo", op_name(op), mark);
    return b;
}

constexpr uint32_t kBuildOp = 31;

// Where an unfinished command list stopped among its ray tracing builds: the first op the GPU did not complete is
// build #nth of the `of` builds the list records. nth is 0 when that op is not a ray tracing build.
struct BuildStop {
    uint64_t list = 0;
    uint32_t nth = 0, of = 0;
};

template <typename Op> BuildStop build_stop(const Op *ops, uint32_t count, uint32_t completed, uint64_t list)
{
    BuildStop s;
    if (!ops || completed >= count || uint32_t(ops[completed]) != kBuildOp)
        return s;
    s.list = list;
    for (uint32_t i = 0; i < count; i++)
        if (uint32_t(ops[i]) == kBuildOp) {
            s.of++;
            if (i <= completed)
                s.nth++;
        }
    return s;
}

inline std::string node_header(unsigned n, const char *list, const char *queue, uint32_t count, uint32_t completed,
                               uint64_t object = 0)
{
    char b[440], at[40] = "";
    if (object)
        std::snprintf(at, sizeof at, " (0x%llx)", (unsigned long long)object);
    std::snprintf(b, sizeof b, "    node %u: command list %s%s on queue %s, %u of %u op(s) completed%s\n", n,
                  list && *list ? list : "(unnamed)", at, queue && *queue ? queue : "(unnamed)",
                  completed > count ? count : completed, count, node_incomplete(count, completed) ? ", NOT FINISHED" : "");
    return b;
}

inline std::string allocation_line(const char *name, uint32_t type, uint64_t object)
{
    char b[400], at[40] = "";
    if (object)
        std::snprintf(at, sizeof at, " at 0x%llx", (unsigned long long)object);
    std::snprintf(b, sizeof b, "    %-18s %s%s\n", allocation_type_name(type), name && *name ? name : "(unnamed)", at);
    return b;
}

inline std::string page_fault_line(uint64_t va)
{
    char b[120];
    std::snprintf(b, sizeof b, "  page fault at GPU virtual address 0x%llx\n", (unsigned long long)va);
    return b;
}

constexpr int kMaxDebugMessages = 50;
constexpr uint32_t kSdkLayersMissing = 0x887a002b;

inline bool debug_requested(const char *env)
{
    return env && env[0] == '1' && !env[1];
}

inline bool is_error_severity(int severity)
{
    return severity == 0 || severity == 1;
}

inline const char *severity_name(int severity)
{
    switch (severity) {
    case 0: return "CORRUPTION";
    case 1: return "ERROR";
    case 2: return "WARNING";
    case 3: return "INFO";
    default: return "MESSAGE";
    }
}

inline const char *category_name(int category)
{
    static const char *const names[] = {"APPLICATION_DEFINED", "MISCELLANEOUS", "INITIALIZATION", "CLEANUP", "COMPILATION",
                                        "STATE_CREATION", "STATE_SETTING", "STATE_GETTING", "RESOURCE_MANIPULATION", "EXECUTION",
                                        "SHADER"};
    return category >= 0 && category < int(sizeof names / sizeof *names) ? names[category] : "UNKNOWN_CATEGORY";
}

struct DebugBudget {
    int taken = 0;

    bool take(int severity)
    {
        if (!is_error_severity(severity) || taken >= kMaxDebugMessages)
            return false;
        taken++;
        return true;
    }
};

inline std::string debug_line(int index, int severity, int category, uint32_t id, const char *text)
{
    char b[1400];
    std::snprintf(b, sizeof b, "[d3d-debug] #%d %s %s id=%u: %.1200s", index, severity_name(severity), category_name(category), id,
                  text ? text : "");
    std::string s = b;
    while (!s.empty() && (s.back() == '\n' || s.back() == '\r'))
        s.pop_back();
    return s;
}

inline std::string debug_missing_note(uint32_t hr)
{
    char b[300];
    std::snprintf(b, sizeof b,
                  "[d3d-debug] the D3D12 debug layer is not installed (0x%08x; no Graphics Tools and no d3d12SDKLayers.dll next to "
                  "D3D12Core.dll); carrying on without it",
                  hr);
    return b;
}

inline bool contains_nocase(std::string_view hay, std::string_view needle)
{
    if (needle.empty() || hay.size() < needle.size())
        return false;
    for (size_t i = 0; i + needle.size() <= hay.size(); i++) {
        size_t k = 0;
        while (k < needle.size() && (hay[i + k] | 0x20) == (needle[k] | 0x20))
            k++;
        if (k == needle.size())
            return true;
    }
    return false;
}

inline bool report_shows_removal(std::string_view text)
{
    for (const char *h : {"887a0001", "887a0005", "887a0006", "887a0007", "887a0020"})
        if (contains_nocase(text, h))
            return true;
    return contains_nocase(text, "Ne_FatalAbort") && contains_nocase(text, "Render_Check_GfxResult");
}

inline bool log_shows_removal(std::string_view text)
{
    return contains_nocase(text, "the GPU was removed");
}

// crash-<yyyymmdd>-<hhmmss>[-n].txt names sort by time; the newest is the greatest.
inline std::string newest_report(const std::vector<std::string> &names)
{
    std::string best;
    for (const auto &n : names)
        if (n.size() > 4 && n.compare(0, 6, "crash-") == 0 && n.compare(n.size() - 4, 4, ".txt") == 0 && n.substr(0, n.size() - 4) > best.substr(0, best.size() ? best.size() - 4 : 0))
            best = n;
    return best;
}

inline bool should_arm_from(std::string_view checked_name, std::string_view newest, std::string_view text)
{
    if (newest.empty() || newest == checked_name)
        return false;
    return report_shows_removal(text) || log_shows_removal(text);
}

// %LOCALAPPDATA%\\Swofty\\state\\, where the loader log and crash reports go when the game folder refuses a write.
inline std::wstring state_dir(const std::wstring &local_app_data)
{
    return local_app_data.empty() ? std::wstring() : local_app_data + L"\\Swofty\\state\\";
}

// The folders a marker file is written to and read from, in order: the game's swofty folder, then the state folder.
inline std::vector<std::wstring> marker_dirs(const std::wstring &swofty_dir, const std::wstring &local_app_data)
{
    std::vector<std::wstring> dirs{swofty_dir};
    std::wstring st = state_dir(local_app_data);
    if (!st.empty() && st != swofty_dir)
        dirs.push_back(st);
    return dirs;
}

// Index of the first folder try_write accepts, or -1.
template <typename F> int write_first(const std::vector<std::wstring> &dirs, F try_write)
{
    for (size_t i = 0; i < dirs.size(); i++)
        if (try_write(dirs[i]))
            return int(i);
    return -1;
}

}
