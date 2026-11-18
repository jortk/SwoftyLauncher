#include "dred.hpp"

#include <d3d12.h>

#include <vector>

#include "dred_logic.hpp"
#include "safetyhook.hpp"

namespace runtime::dred {
namespace {

using CreateDeviceFn = HRESULT(WINAPI *)(IUnknown *, D3D_FEATURE_LEVEL, REFIID, void **);
using DebugInterfaceFn = HRESULT(WINAPI *)(REFIID, void **);

constexpr int kDevices = 8;

bool armed;
HMODULE core_module;
std::vector<std::wstring> marker_folders;
void (*log_fn)(const char *, ...);
ID3D12Device *devices[kDevices];
LONG ndevices;
safetyhook::InlineHook create_hook;
bool debug_armed;
CRITICAL_SECTION debug_lock;
DebugBudget debug_budget;
std::vector<std::string> debug_lines;
ID3D12InfoQueue *polled[kDevices];
LONG npolled;
HANDLE poll_thread;
std::vector<BuildStop> stops;

bool readable(const void *p, size_t n)
{
    auto a = reinterpret_cast<uintptr_t>(p);
    if (!a)
        return false;
    for (uintptr_t q = a & ~0xfffull; q < a + n; q += 0x1000) {
        MEMORY_BASIC_INFORMATION m;
        if (!VirtualQuery(reinterpret_cast<void *>(q), &m, sizeof m) || m.State != MEM_COMMIT ||
            (m.Protect & (PAGE_GUARD | PAGE_NOACCESS)) || !(m.Protect & 0xEE))
            return false;
    }
    return true;
}

std::string name_a(const char *p)
{
    std::string s;
    for (int i = 0; i < 128; i++) {
        if ((i == 0 || ((reinterpret_cast<uintptr_t>(p) + i) & 0xfff) == 0) && !readable(p + i, 1))
            break;
        if (!p[i])
            break;
        s += (unsigned char)p[i] >= 0x20 && (unsigned char)p[i] < 0x7f ? p[i] : '?';
    }
    return s;
}

std::string name_w(const wchar_t *p)
{
    std::string s;
    for (int i = 0; i < 128; i++) {
        if ((i == 0 || ((reinterpret_cast<uintptr_t>(p + i)) & 0xfff) == 0) && !readable(p + i, 2))
            break;
        if (!p[i])
            break;
        s += p[i] >= 0x20 && p[i] < 0x7f ? char(p[i]) : '?';
    }
    return s;
}

std::string pick(const char *a, const wchar_t *w)
{
    std::string s = a ? name_a(a) : std::string();
    return s.empty() && w ? name_w(w) : s;
}

bool caller_is_core(void *ret)
{
    HMODULE m = nullptr;
    return GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                              static_cast<LPCWSTR>(ret), &m) &&
           m == core_module;
}

void record_message(D3D12_MESSAGE_CATEGORY category, D3D12_MESSAGE_SEVERITY severity, D3D12_MESSAGE_ID id, const char *text)
{
    EnterCriticalSection(&debug_lock);
    bool take = debug_budget.take(int(severity));
    int n = debug_budget.taken;
    std::string line = take ? debug_line(n, int(severity), int(category), uint32_t(id), text) : std::string();
    if (take)
        debug_lines.push_back(line);
    LeaveCriticalSection(&debug_lock);
    if (take && log_fn)
        log_fn("%s%s", line.c_str(), n == kMaxDebugMessages ? " (50 messages logged; later ones are dropped)" : "");
}

void WINAPI on_message(D3D12_MESSAGE_CATEGORY category, D3D12_MESSAGE_SEVERITY severity, D3D12_MESSAGE_ID id, LPCSTR text, void *)
{
    record_message(category, severity, id, text);
}

void drain(ID3D12InfoQueue *q)
{
    UINT64 n = q->GetNumStoredMessagesAllowedByRetrievalFilter();
    for (UINT64 i = 0; i < n && debug_budget.taken < kMaxDebugMessages; i++) {
        SIZE_T len = 0;
        if (FAILED(q->GetMessage(i, nullptr, &len)) || !len || len > (1u << 20))
            continue;
        std::vector<char> buf(len);
        auto *m = reinterpret_cast<D3D12_MESSAGE *>(buf.data());
        if (SUCCEEDED(q->GetMessage(i, m, &len)))
            record_message(m->Category, m->Severity, m->ID, m->pDescription);
    }
    q->ClearStoredMessages();
}

void drain_all()
{
    for (LONG i = 0; i < npolled && i < kDevices; i++)
        drain(polled[i]);
}

DWORD WINAPI poll_main(void *)
{
    while (debug_budget.taken < kMaxDebugMessages) {
        Sleep(200);
        drain_all();
    }
    return 0;
}

void attach_info_queue(ID3D12Device *dev)
{
    ID3D12InfoQueue *q = nullptr;
    if (FAILED(dev->QueryInterface(__uuidof(ID3D12InfoQueue), reinterpret_cast<void **>(&q))) || !q) {
        if (log_fn)
            log_fn("[d3d-debug] the device has no ID3D12InfoQueue (the debug layer did not attach to it)");
        return;
    }
    ID3D12InfoQueue1 *q1 = nullptr;
    DWORD cookie = 0;
    if (SUCCEEDED(q->QueryInterface(__uuidof(ID3D12InfoQueue1), reinterpret_cast<void **>(&q1))) && q1 &&
        SUCCEEDED(q1->RegisterMessageCallback(on_message, D3D12_MESSAGE_CALLBACK_FLAG_NONE, nullptr, &cookie))) {
        q1->Release();
        q->Release();
        if (log_fn)
            log_fn("[d3d-debug] message callback registered on device %p; the first %d ERROR/CORRUPTION messages are logged", static_cast<void *>(dev),
                   kMaxDebugMessages);
        return;
    }
    if (q1)
        q1->Release();
    LONG i = InterlockedIncrement(&npolled) - 1;
    if (i >= kDevices) {
        q->Release();
        return;
    }
    polled[i] = q;
    if (!poll_thread)
        poll_thread = CreateThread(nullptr, 0, poll_main, nullptr, 0, nullptr);
    if (log_fn)
        log_fn("[d3d-debug] ID3D12InfoQueue1 is missing (older Windows); polling the message queue of device %p every 200 ms", static_cast<void *>(dev));
}

HRESULT WINAPI hooked_create_device(IUnknown *adapter, D3D_FEATURE_LEVEL level, REFIID riid, void **out)
{
    void *ret = __builtin_return_address(0);
    HRESULT hr = create_hook.unsafe_call<HRESULT>(adapter, level, riid, out);
    if (SUCCEEDED(hr) && out && readable(out, sizeof *out) && *out && !caller_is_core(ret)) {
        ID3D12Device *dev = nullptr;
        if (SUCCEEDED(static_cast<IUnknown *>(*out)->QueryInterface(__uuidof(ID3D12Device), reinterpret_cast<void **>(&dev)))) {
            if (debug_armed)
                attach_info_queue(dev);
            LONG i = InterlockedIncrement(&ndevices) - 1;
            if (i < kDevices)
                devices[i] = dev;
            else
                dev->Release();
        }
    }
    return hr;
}

std::string read_file(const std::wstring &path, size_t cap)
{
    HANDLE f = CreateFileW(path.c_str(), GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE, nullptr, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (f == INVALID_HANDLE_VALUE)
        return {};
    std::string s(cap, '\0');
    DWORD n = 0;
    ReadFile(f, s.data(), DWORD(cap), &n, nullptr);
    CloseHandle(f);
    s.resize(n);
    return s;
}

DWORD write_text(const std::wstring &path, const std::string &text)
{
    HANDLE f = CreateFileW(path.c_str(), GENERIC_WRITE, FILE_SHARE_READ, nullptr, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (f == INVALID_HANDLE_VALUE)
        return GetLastError();
    DWORD w = 0, err = 0;
    if (!WriteFile(f, text.data(), DWORD(text.size()), &w, nullptr))
        err = GetLastError();
    CloseHandle(f);
    return err;
}

bool put_marker(const wchar_t *name, const std::string &text)
{
    DWORD err = 0;
    int at = write_first(marker_folders, [&](const std::wstring &dir) {
        if (dir.size() > 4 && dir.find(L"\\state\\") != std::wstring::npos)
            CreateDirectoryW((dir.substr(0, dir.size() - 7)).c_str(), nullptr), CreateDirectoryW(dir.c_str(), nullptr);
        err = write_text(dir + name, text);
        if (err && log_fn)
            log_fn("[dred] could not write %ls%ls: error %lu", dir.c_str(), name, (unsigned long)err);
        return err == 0;
    });
    return at >= 0;
}

std::string read_marker(const wchar_t *name)
{
    for (const auto &d : marker_folders) {
        std::string s = read_file(d + name, 128);
        if (!s.empty())
            return s;
    }
    return {};
}

bool take_marker(const wchar_t *name)
{
    bool had = false;
    for (const auto &d : marker_folders)
        if (GetFileAttributesW((d + name).c_str()) != INVALID_FILE_ATTRIBUTES) {
            had = true;
            DeleteFileW((d + name).c_str());
        }
    return had;
}

std::wstring widen(const std::string &s)
{
    return std::wstring(s.begin(), s.end());
}

std::string newest_in(const std::wstring &dir)
{
    std::vector<std::string> names;
    WIN32_FIND_DATAW fd;
    HANDLE h = FindFirstFileW((dir + L"crash-*.txt").c_str(), &fd);
    if (h == INVALID_HANDLE_VALUE)
        return {};
    do {
        std::string n;
        for (const wchar_t *c = fd.cFileName; *c; c++)
            n += *c < 0x80 ? char(*c) : '?';
        names.push_back(n);
    } while (FindNextFileW(h, &fd));
    FindClose(h);
    return newest_report(names);
}

bool prior_session_removed(const char **how)
{
    if (take_marker(L"dred.flag")) {
        *how = "dred.flag";
        return true;
    }
    std::string checked = read_marker(L"dred.checked");
    while (!checked.empty() && (checked.back() == '\n' || checked.back() == '\r'))
        checked.pop_back();
    std::wstring found_dir;
    std::string newest;
    for (const auto &d : marker_folders) {
        std::string c = newest_in(d);
        if (c > newest)
            newest = c, found_dir = d;
    }
    if (newest.empty() || newest == checked)
        return false;
    put_marker(L"dred.checked", newest + "\n");
    if (!should_arm_from(checked, newest, read_file(found_dir + widen(newest), 4 << 20)))
        return false;
    *how = "the newest crash report";
    return true;
}

void write_flag()
{
    put_marker(L"dred.flag", "removed\n");
}

template <typename Node> std::string breadcrumbs(const Node *head)
{
    std::vector<const Node *> unfinished, finished;
    unsigned total = 0;
    for (const Node *n = head; n && total < kMaxChain && readable(n, sizeof *n); n = n->pNext, total++) {
        uint32_t done = n->pLastBreadcrumbValue && readable(n->pLastBreadcrumbValue, 4) ? *n->pLastBreadcrumbValue : 0;
        (node_incomplete(n->BreadcrumbCount, done) ? unfinished : finished).push_back(n);
    }
    std::string s;
    char b[160];
    std::snprintf(b, sizeof b, "\n  auto-breadcrumbs: %u command list node(s), %zu not finished; showing %s\n", total,
                  unfinished.size(), "unfinished first");
    s += b;
    if (!total)
        return s + "    none recorded\n";
    unsigned shown = 0;
    for (const auto *list : {&unfinished, &finished})
        for (const Node *n : *list) {
            if (shown == kMaxNodes) {
                s += "    (more nodes not shown)\n";
                return s;
            }
            uint32_t count = n->BreadcrumbCount;
            uint32_t done = n->pLastBreadcrumbValue && readable(n->pLastBreadcrumbValue, 4) ? *n->pLastBreadcrumbValue : 0;
            s += node_header(shown++, pick(n->pCommandListDebugNameA, n->pCommandListDebugNameW).c_str(),
                             pick(n->pCommandQueueDebugNameA, n->pCommandQueueDebugNameW).c_str(), count, done,
                             reinterpret_cast<uint64_t>(n->pCommandList));
            if (list == &unfinished && n->pCommandHistory && readable(n->pCommandHistory, count * sizeof(uint32_t))) {
                BuildStop st = build_stop(n->pCommandHistory, count, done, reinterpret_cast<uint64_t>(n->pCommandList));
                bool seen = false;
                for (const BuildStop &o : stops)
                    seen = seen || (o.list == st.list && o.nth == st.nth && o.of == st.of);
                if (st.nth && !seen)
                    stops.push_back(st);
            }
            Window w = op_window(count, done);
            if (w.from)
                s += "      ... " + std::to_string(w.from) + " earlier op(s) not shown\n";
            if (!n->pCommandHistory || !readable(n->pCommandHistory + w.from, (w.to - w.from) * sizeof(uint32_t))) {
                s += "      command history not readable\n";
                continue;
            }
            for (uint32_t i = w.from; i < w.to; i++)
                s += op_line(i, done, n->pCommandHistory[i]);
            if (w.to < count)
                s += "      ... " + std::to_string(count - w.to) + " later op(s) not shown\n";
        }
    return s;
}

template <typename Node> std::string contexts(const Node *head)
{
    std::string s;
    unsigned seen = 0, shown = 0;
    for (const Node *n = head; n && seen < kMaxChain && readable(n, sizeof *n); n = n->pNext, seen++) {
        if (!n->BreadcrumbContextsCount || !readable(n->pBreadcrumbContexts, n->BreadcrumbContextsCount * sizeof *n->pBreadcrumbContexts))
            continue;
        for (UINT i = 0; i < n->BreadcrumbContextsCount && shown < kMaxOps; i++, shown++) {
            char b[300];
            std::snprintf(b, sizeof b, "    op %u of %s: %s\n", n->pBreadcrumbContexts[i].BreadcrumbIndex,
                          pick(n->pCommandListDebugNameA, n->pCommandListDebugNameW).c_str(),
                          name_w(n->pBreadcrumbContexts[i].pContextString).c_str());
            s += b;
        }
    }
    return shown ? "\n  breadcrumb contexts:\n" + s : s;
}

template <typename Alloc> std::string allocations(const char *title, const Alloc *head, uint64_t object_of(const Alloc *))
{
    std::string s;
    unsigned n = 0, seen = 0;
    for (const Alloc *a = head; a && seen < kMaxChain && readable(a, sizeof *a); a = a->pNext, seen++)
        if (n++ < kMaxAllocations)
            s += allocation_line(pick(a->ObjectNameA, a->ObjectNameW).c_str(), uint32_t(a->AllocationType), object_of(a));
    char b[120];
    std::snprintf(b, sizeof b, "  %s: %u%s\n", title, n, n > kMaxAllocations ? " (first 32 shown)" : "");
    return b + s;
}

uint64_t no_object(const D3D12_DRED_ALLOCATION_NODE *)
{
    return 0;
}

uint64_t object_of(const D3D12_DRED_ALLOCATION_NODE1 *a)
{
    return reinterpret_cast<uint64_t>(a->pObject);
}

std::string removed_device(ID3D12Device *dev, HRESULT reason)
{
    char b[200];
    std::snprintf(b, sizeof b, "  device %p: GetDeviceRemovedReason 0x%08lx (%s)\n", static_cast<void *>(dev), (unsigned long)reason,
                  removal_name(uint32_t(reason)));
    std::string s = b;
    ID3D12DeviceRemovedExtendedData1 *d1 = nullptr;
    if (SUCCEEDED(dev->QueryInterface(__uuidof(ID3D12DeviceRemovedExtendedData1), reinterpret_cast<void **>(&d1)))) {
        D3D12_DRED_AUTO_BREADCRUMBS_OUTPUT1 ab{};
        D3D12_DRED_PAGE_FAULT_OUTPUT1 pf{};
        HRESULT ha = d1->GetAutoBreadcrumbsOutput1(&ab), hp = d1->GetPageFaultAllocationOutput1(&pf);
        if (SUCCEEDED(ha)) {
            s += breadcrumbs(ab.pHeadAutoBreadcrumbNode);
            s += contexts(ab.pHeadAutoBreadcrumbNode);
        } else {
            std::snprintf(b, sizeof b, "\n  auto-breadcrumbs: unavailable (0x%08lx; DRED was not on for this device)\n", (unsigned long)ha);
            s += b;
        }
        if (SUCCEEDED(hp)) {
            s += "\n" + page_fault_line(pf.PageFaultVA);
            s += allocations("existing allocations", pf.pHeadExistingAllocationNode, object_of);
            s += allocations("recently freed allocations", pf.pHeadRecentFreedAllocationNode, object_of);
        } else {
            std::snprintf(b, sizeof b, "\n  page fault output: unavailable (0x%08lx)\n", (unsigned long)hp);
            s += b;
        }
        d1->Release();
        return s;
    }
    ID3D12DeviceRemovedExtendedData *d0 = nullptr;
    if (SUCCEEDED(dev->QueryInterface(__uuidof(ID3D12DeviceRemovedExtendedData), reinterpret_cast<void **>(&d0)))) {
        D3D12_DRED_AUTO_BREADCRUMBS_OUTPUT ab{};
        D3D12_DRED_PAGE_FAULT_OUTPUT pf{};
        if (SUCCEEDED(d0->GetAutoBreadcrumbsOutput(&ab)))
            s += breadcrumbs(ab.pHeadAutoBreadcrumbNode);
        if (SUCCEEDED(d0->GetPageFaultAllocationOutput(&pf))) {
            s += "\n" + page_fault_line(pf.PageFaultVA);
            s += allocations("existing allocations", pf.pHeadExistingAllocationNode, no_object);
            s += allocations("recently freed allocations", pf.pHeadRecentFreedAllocationNode, no_object);
        }
        d0->Release();
        return s;
    }
    return s + "  the device has no ID3D12DeviceRemovedExtendedData (an older Windows, Wine or vkd3d)\n";
}

}

void arm(const std::wstring &swofty_dir, void (*log)(const char *, ...), HMODULE self)
{
    core_module = self;
    log_fn = log;
    wchar_t local[MAX_PATH];
    DWORD ln = GetEnvironmentVariableW(L"LOCALAPPDATA", local, MAX_PATH);
    marker_folders = marker_dirs(swofty_dir, ln && ln < MAX_PATH ? std::wstring(local) : std::wstring());
    char env[16] = {0}, denv[16] = {0};
    GetEnvironmentVariableA("SWOFTY_DRED", env, sizeof env);
    GetEnvironmentVariableA("SWOFTY_D3D_DEBUG", denv, sizeof denv);
    const char *how_prior = "";
    Decision d = decide(env, prior_session_removed(&how_prior));
    bool want_debug = debug_requested(denv);
    if (!d.on && !want_debug) {
        log("[dred] off: %s (SWOFTY_DRED=1 turns it on; it is on by default)", d.why);
        return;
    }
    bool loaded = GetModuleHandleW(L"d3d12.dll") != nullptr;
    HMODULE d3d12 = LoadLibraryW(L"d3d12.dll");
    auto get = d3d12 ? reinterpret_cast<DebugInterfaceFn>(reinterpret_cast<void *>(GetProcAddress(d3d12, "D3D12GetDebugInterface"))) : nullptr;
    if (!get) {
        log("[dred] d3d12.dll has no D3D12GetDebugInterface; carrying on without DRED or the debug layer");
        return;
    }
    if (want_debug) {
        InitializeCriticalSection(&debug_lock);
        ID3D12Debug *dbg = nullptr;
        HRESULT hd = get(__uuidof(ID3D12Debug), reinterpret_cast<void **>(&dbg));
        if (SUCCEEDED(hd) && dbg) {
            dbg->EnableDebugLayer();
            dbg->Release();
            debug_armed = true;
            log("[d3d-debug] on (SWOFTY_D3D_DEBUG=1): debug layer enabled%s",
                loaded ? " (d3d12.dll was already loaded; a device created before this point is not covered)" : "");
        } else {
            log("%s", debug_missing_note(uint32_t(hd)).c_str());
        }
    }
    const char *how = nullptr;
    if (d.on) {
        ID3D12DeviceRemovedExtendedDataSettings1 *s1 = nullptr;
        ID3D12DeviceRemovedExtendedDataSettings *s0 = nullptr;
        HRESULT h1 = get(__uuidof(ID3D12DeviceRemovedExtendedDataSettings1), reinterpret_cast<void **>(&s1));
        if (SUCCEEDED(h1) && s1) {
            s1->SetAutoBreadcrumbsEnablement(D3D12_DRED_ENABLEMENT_FORCED_ON);
            s1->SetPageFaultEnablement(D3D12_DRED_ENABLEMENT_FORCED_ON);
            s1->SetBreadcrumbContextEnablement(D3D12_DRED_ENABLEMENT_FORCED_ON);
            s1->Release();
            how = "breadcrumbs, page faults and breadcrumb contexts";
        } else {
            HRESULT h0 = get(__uuidof(ID3D12DeviceRemovedExtendedDataSettings), reinterpret_cast<void **>(&s0));
            if (SUCCEEDED(h0) && s0) {
                s0->SetAutoBreadcrumbsEnablement(D3D12_DRED_ENABLEMENT_FORCED_ON);
                s0->SetPageFaultEnablement(D3D12_DRED_ENABLEMENT_FORCED_ON);
                s0->Release();
                how = "breadcrumbs and page faults";
            } else {
                log("[dred] on (%s), but the DRED settings interface is missing (0x%08lx, 0x%08lx; Wine/vkd3d or an old Windows); "
                    "carrying on without it",
                    d.why, (unsigned long)h1, (unsigned long)h0);
            }
        }
    }
    if (!how && !debug_armed)
        return;
    auto create = reinterpret_cast<void *>(GetProcAddress(d3d12, "D3D12CreateDevice"));
    bool hooked = false;
    if (create) {
        auto h = safetyhook::InlineHook::create(create, reinterpret_cast<void *>(&hooked_create_device));
        if (h) {
            create_hook = std::move(*h);
            hooked = true;
        }
    }
    armed = how != nullptr;
    if (how)
        log("[dred] on (%s): forced on %s%s; D3D12CreateDevice %s, so a crash report names the removed device's GPU operation", d.why, how,
            loaded ? " (d3d12.dll was already loaded; a device created before this point is not covered)" : "",
            hooked ? "hooked" : "NOT hooked");
    else
        log("[d3d-debug] D3D12CreateDevice %s", hooked ? "hooked" : "NOT hooked, so no message queue is attached");
}

static std::string debug_section()
{
    if (!debug_armed)
        return std::string();
    drain_all();
    EnterCriticalSection(&debug_lock);
    std::string body;
    for (const std::string &l : debug_lines)
        body += "  " + l + "\n";
    LeaveCriticalSection(&debug_lock);
    std::string head = "\n== D3D12 debug layer (SWOFTY_D3D_DEBUG=1)\n";
    return head + (body.empty() ? "  no ERROR or CORRUPTION message was raised\n" : body);
}

static std::string dred_section(bool registers_show_removal);

std::string section(bool registers_show_removal)
{
    return dred_section(registers_show_removal) + debug_section();
}

std::vector<BuildStop> build_stops()
{
    return stops;
}

static std::string dred_section(bool registers_show_removal)
{
    stops.clear();
    std::string head = "\n== GPU device removal (D3D12 DRED)\n";
    int removed = 0;
    std::string body;
    for (LONG i = 0; i < ndevices && i < kDevices; i++) {
        ID3D12Device *dev = devices[i];
        HRESULT reason = dev ? dev->GetDeviceRemovedReason() : S_OK;
        if (FAILED(reason)) {
            removed++;
            body += removed_device(dev, reason);
        }
    }
    if (removed) {
        write_flag();
        return head + body;
    }
    if (!registers_show_removal)
        return armed ? head + "  DRED is on and no device the game created reports removal\n" : std::string();
    write_flag();
    if (armed)
        return head + "  a register holds a DXGI device-removed HRESULT, but no device the game created reports removal\n";
    return head + "  a register holds a DXGI device-removed HRESULT. DRED was off this session, so there is no breadcrumb data.\n"
                  "  DRED is on by default; SWOFTY_DRED=0 turned it off, or this GPU or Windows lacks the interface.\n";
}

}
