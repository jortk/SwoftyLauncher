#pragma once
#include <cstdint>
#include <cstdio>
#include <string>

// Pure logic of the hardware settings probe (crash.cpp): whether the player's hardware_settings_config.xml is a
// OneDrive cloud-only file, and the one loader.log line that records it. The game reads that file in its
// InitSequence and aborts when it cannot read it all. No Windows calls, so the unit test runs on the host.

namespace hw_settings {

constexpr uint32_t ATTR_OFFLINE = 0x00001000;
constexpr uint32_t ATTR_RECALL_ON_OPEN = 0x00040000;
constexpr uint32_t ATTR_RECALL_ON_DATA_ACCESS = 0x00400000;

inline std::string cloud_flags(uint32_t attrs)
{
    std::string out;
    auto add = [&](uint32_t bit, const char *name) {
        if (attrs & bit)
            out += (out.empty() ? "" : "+") + std::string(name);
    };
    add(ATTR_RECALL_ON_DATA_ACCESS, "recall_on_data_access");
    add(ATTR_RECALL_ON_OPEN, "recall_on_open");
    add(ATTR_OFFLINE, "offline");
    return out;
}

inline bool cloud_only(uint32_t attrs)
{
    return !cloud_flags(attrs).empty();
}

inline bool under_onedrive(const std::wstring &path)
{
    size_t at = 0;
    while (at < path.size()) {
        size_t end = path.find_first_of(L"\\/", at);
        if (end == std::wstring::npos)
            end = path.size();
        std::wstring part = path.substr(at, end - at);
        if (part.size() >= 8) {
            std::wstring head = part.substr(0, 8);
            for (auto &c : head)
                c = c >= L'A' && c <= L'Z' ? wchar_t(c + 32) : c;
            if (head == L"onedrive")
                return true;
        }
        at = end + 1;
    }
    return false;
}

struct Probe {
    std::string path;
    bool onedrive = false;
    bool exists = false;
    uint64_t size = 0;
    uint32_t attrs = 0;
    bool read_tried = false;
    bool read_ok = false;
    uint32_t read_bytes = 0;
    uint32_t read_error = 0;
};

inline std::string line(const Probe &p)
{
    char hex[16];
    snprintf(hex, sizeof hex, "0x%08x", p.attrs);
    std::string out = "[hwsettings] " + (p.path.empty() ? std::string("(no Documents folder)") : p.path) +
                      ": onedrive=" + (p.onedrive ? "yes" : "no") + " exists=" + (p.exists ? "yes" : "no");
    if (!p.exists)
        return out;
    out += " size=" + std::to_string(p.size) + " attributes=" + hex;
    out += cloud_only(p.attrs) ? " cloud-only=yes (" + cloud_flags(p.attrs) + ")" : " cloud-only=no";
    if (!p.read_tried)
        out += " read test: skipped, the file is cloud-only and reading it would download it";
    else if (p.read_ok)
        out += " read test: ok (" + std::to_string(p.read_bytes) + " bytes)";
    else
        out += " read test: FAILED (Windows error " + std::to_string(p.read_error) + ")";
    return out;
}

}
