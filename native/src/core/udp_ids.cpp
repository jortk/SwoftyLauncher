#define WIN32_LEAN_AND_MEAN
#include <winsock2.h>
#include <ws2tcpip.h>

#include "imports.hpp"
#include "runtime.hpp"
#include "udp_ids_logic.hpp"

#include <swofty/mixin.hpp>

#include <atomic>
#include <cstdio>
#include <map>
#include <mutex>
#include <string>

// UDP telemetry reports the stat ids of added teams and drivers, which overlays that know EA's ids show
// as unknown. The game sends its finished telemetry packets with sendto through its ws2_32 import slots
// (both import descriptors, like the other socket hooks). This hook copies a buffer that parses as an F1
// packet (udp_ids_logic.hpp), replaces the ids swofty/udp_ids.json maps, and sends the copy; every other
// call passes through with its arguments. Datagrams to the LAN game port 21414 and to 10.77.0.0/16 are
// never looked at. SWOFTY_UDP_STOCK_IDS=0 turns it off. With the telemetry feature on, the same hook also hands every
// such packet to the telemetry decoder (telemetry.cpp) before any rewrite. A Session packet of an added track gets
// trackId -1 and is followed by the Swofty Track packet with the track's Swofty track id (udp_ids_logic.hpp,
// session_track); SWOFTY_UDP_STOCK_IDS=0 turns that off too, and SWOFTY_UDP_TRACK_PACKET=0 keeps the trackId rewrite
// but sends no Swofty Track packet, for a tool that fails on a packet id it does not know.

namespace {

namespace ids = runtime::udp_ids;

constexpr uint16_t GAME_PORT = 21414;
constexpr size_t LINES = 12;

using SendToFn = int(WSAAPI *)(SOCKET, const char *, int, int, const sockaddr *, int);

void *detour_target;
void *orig_sendto;
ids::Map map;
std::atomic<bool> armed;
std::atomic<bool> rewriting;
std::atomic<unsigned> said;
std::atomic<int> seen_formats;
std::atomic<uint64_t> odd_sizes[4];
std::atomic<unsigned> trial_said;
std::mutex track_mu;
std::map<int, std::string> tracks;
std::atomic<bool> has_tracks;
std::atomic<bool> track_said;
std::atomic<bool> track_packet{true};

int session(SOCKET s, const char *buf, int len, int flags, const sockaddr *to, int tolen, bool &sent)
{
    auto real = reinterpret_cast<SendToFn>(orig_sendto);
    uint8_t copy[ids::SESSION_SIZE];
    uint8_t extra[ids::TRACK_PACKET_SIZE];
    memcpy(copy, buf, ids::SESSION_SIZE);
    {
        std::lock_guard g(track_mu);
        sent = ids::session_track(map, tracks, copy, ids::SESSION_SIZE, extra);
    }
    if (!sent)
        return 0;
    int r = real(s, reinterpret_cast<const char *>(copy), len, flags, to, tolen);
    DWORD err = GetLastError();
    bool packet = track_packet.load(std::memory_order_relaxed);
    if (r != SOCKET_ERROR && packet)
        real(s, reinterpret_cast<const char *>(extra), int(sizeof extra), flags, to, tolen);
    if (!track_said.exchange(true)) {
        uint32_t id = uint32_t(extra[ids::HEADER]) | uint32_t(extra[ids::HEADER + 1]) << 8 |
                      uint32_t(extra[ids::HEADER + 2]) << 16 | uint32_t(extra[ids::HEADER + 3]) << 24;
        swofty::log("[udp] first Session packet of an added track: statId %u sent as %d, %s %u with id %u (%s)",
                    unsigned(extra[ids::HEADER + 4]), int(int8_t(copy[ids::SESSION_TRACK])),
                    packet ? "Swofty Track packet" : "no Swofty Track packet", unsigned(ids::TRACK_PACKET), id,
                    reinterpret_cast<const char *>(extra + ids::HEADER + 5));
    }
    SetLastError(err);
    return r;
}

bool lan_traffic(const sockaddr *to, int tolen)
{
    if (!to || tolen < int(sizeof(sockaddr_in)) || to->sa_family != AF_INET)
        return false;
    auto *in = reinterpret_cast<const sockaddr_in *>(to);
    uint16_t port = __builtin_bswap16(in->sin_port);
    uint32_t ip = __builtin_bswap32(in->sin_addr.s_addr);
    return port == GAME_PORT || (ip & 0xffff0000) == 0x0a4d0000;
}

void first(int format, int id, const std::vector<ids::Change> &changes)
{
    unsigned bit = 1u << (format - 2024 & 3);
    if (said.fetch_or(bit) & bit)
        return;
    DWORD err = GetLastError();
    std::string what;
    for (size_t i = 0; i < changes.size() && i < 4; i++)
        what += (i ? ", " : "") + std::string(changes[i].driver ? "driver " : "team ") + std::to_string(changes[i].from) +
                " -> " + std::to_string(changes[i].to);
    swofty::log("[udp] first telemetry packet rewritten: format %d, packet %d, %s model: %s", format, id,
                map.model == ids::MODEL_2025 ? "2025" : map.model == ids::MODEL_2026 ? "2026" : "unknown", what.c_str());
    SetLastError(err);
}

void odd(const ids::Layout *l, int len)
{
    int slot = l->format - 2024;
    if (slot < 0 || slot > 2 || l->id > 63)
        return;
    uint64_t bit = uint64_t(1) << l->id;
    if (odd_sizes[slot].fetch_or(bit) & bit)
        return;
    DWORD err = GetLastError();
    swofty::log("[udp] format %d packet %d is %d bytes, not the %d the id rewrite expects; left alone", l->format, l->id,
                len, int(l->size));
    SetLastError(err);
}

void trial(const ids::Layout *l, const uint8_t *p, bool changed)
{
    unsigned bit = 1u << (l->format - 2024 & 3);
    if (l->id != 14 || (trial_said.fetch_or(bit) & bit))
        return;
    DWORD err = GetLastError();
    std::string teams;
    for (size_t i = 0; i < l->max_entries; i++) {
        const uint8_t *at = p + ids::HEADER + i * l->stride + l->team;
        teams += (i ? ", " : "") + std::to_string(l->wide ? at[0] | at[1] << 8 : at[0]);
    }
    swofty::log("[udp] first time trial packet: format %d, team ids %s, %s", l->format, teams.c_str(),
                changed ? "rewritten" : "not rewritten");
    SetLastError(err);
}

int WSAAPI h_sendto(SOCKET s, const char *buf, int len, int flags, const sockaddr *to, int tolen)
{
    auto real = reinterpret_cast<SendToFn>(orig_sendto);
    if (!armed.load(std::memory_order_relaxed) || !buf || len < int(ids::HEADER) + 1 || len > int(ids::MAX_PACKET) ||
        lan_traffic(to, tolen))
        return real(s, buf, len, flags, to, tolen);
    runtime::telemetry_udp(reinterpret_cast<const uint8_t *>(buf), size_t(len));
    if (!rewriting.load(std::memory_order_relaxed))
        return real(s, buf, len, flags, to, tolen);
    if (has_tracks.load(std::memory_order_relaxed) && len == int(ids::SESSION_SIZE) && uint8_t(buf[6]) == ids::SESSION) {
        bool sent = false;
        int r = session(s, buf, len, flags, to, tolen, sent);
        if (sent)
            return r;
    }
    if (!ids::identify(reinterpret_cast<const uint8_t *>(buf), size_t(len))) {
        if (const ids::Layout *l = ids::misfit(reinterpret_cast<const uint8_t *>(buf), size_t(len)))
            odd(l, len);
        return real(s, buf, len, flags, to, tolen);
    }
    uint8_t copy[ids::MAX_PACKET];
    memcpy(copy, buf, size_t(len));
    std::vector<ids::Change> changes;
    int format = 0, id = 0;
    const ids::Layout *l = ids::identify(copy, size_t(len));
    if (!ids::rewrite(map, copy, size_t(len), &changes, &format, &id)) {
        trial(l, reinterpret_cast<const uint8_t *>(buf), false);
        return real(s, buf, len, flags, to, tolen);
    }
    trial(l, reinterpret_cast<const uint8_t *>(buf), true);
    first(format, id, changes);
    return real(s, reinterpret_cast<const char *>(copy), len, flags, to, tolen);
}

bool wanted_off()
{
    char v[16] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_UDP_STOCK_IDS", v, sizeof v);
    if (!n || n >= sizeof v || (strcmp(v, "0") && strcmp(v, "off")))
        return false;
    swofty::log("[udp] stock ids off (SWOFTY_UDP_STOCK_IDS=%s)", v);
    return true;
}

bool track_packet_off()
{
    char v[16] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_UDP_TRACK_PACKET", v, sizeof v);
    if (!n || n >= sizeof v || (strcmp(v, "0") && strcmp(v, "off")))
        return false;
    swofty::log("[udp] Swofty Track packet off (SWOFTY_UDP_TRACK_PACKET=%s)", v);
    return true;
}

void setup()
{
    track_packet = !track_packet_off();
    bool tel = runtime::telemetry_wants_udp();
    bool rewrite = false;
    if (!wanted_off()) {
        std::string text = runtime::read_file(runtime::swofty_root() + L"udp_ids.json", size_t(4) << 20);
        std::string why;
        if (!text.empty() && !ids::load(text, map, why))
            swofty::log("[udp] swofty\\udp_ids.json is unusable (%s); telemetry ids stay as the game sends them",
                        why.c_str());
        else
            rewrite = !text.empty() && (map.any || !map.track_ids.empty());
        std::lock_guard g(track_mu);
        for (auto &[id, key] : map.tracks)
            tracks[id] = key;
        has_tracks = !tracks.empty();
    }
    if (!rewrite && !tel)
        return;
    HMODULE ws = LoadLibraryW(L"ws2_32.dll");
    detour_target = ws ? reinterpret_cast<void *>(GetProcAddress(ws, "sendto")) : nullptr;
    int slots = 0;
    if (ws)
        runtime::each_import(ws, "ws2_32.dll", [&](const runtime::ImportSlot &s) {
            if (!s.fn || s.fn != detour_target)
                return;
            void *cur = reinterpret_cast<void *>(*s.slot);
            if (orig_sendto && orig_sendto != cur) {
                swofty::log("[udp] sendto table %d slot exe+0x%llx holds 0x%llx, not 0x%llx; left alone", s.table,
                            (unsigned long long)s.rva, (unsigned long long)uintptr_t(cur),
                            (unsigned long long)uintptr_t(orig_sendto));
                return;
            }
            void *was = orig_sendto;
            orig_sendto = cur;
            if (!runtime::write_slot(s.slot, reinterpret_cast<void *>(&h_sendto))) {
                orig_sendto = was;
                return;
            }
            slots++;
        });
    if (!slots) {
        swofty::log("[udp] no game import slot for sendto; telemetry ids stay as the game sends them%s",
                    tel ? " and the telemetry feed gets no car data" : "");
        return;
    }
    armed = true;
    rewriting = rewrite;
    if (!rewrite) {
        swofty::log("[udp] sendto hooked for the telemetry feed, %d slot(s); no id rewrite", slots);
        return;
    }
    std::string head = std::to_string(map.teams.size()) + " team id(s), " + std::to_string(map.drivers.size()) +
                       " driver id(s), " + std::to_string(map.track_ids.size()) + " track(s) by Swofty track id";
    swofty::log("[udp] stock ids on: %s, %d sendto slot(s)", head.c_str(), slots);
    std::string line;
    size_t shown = 0;
    for (auto &l : map.lines) {
        if (shown++ == LINES)
            break;
        line += (line.empty() ? "" : "; ") + l;
    }
    if (!line.empty())
        swofty::log("[udp] stock ids: %s%s", line.c_str(), map.lines.size() > LINES ? "; ..." : "");
}

}

namespace runtime {

void udp_ids_setup()
{
    setup();
}

void udp_track(int64_t stat_id, const std::string &addon, const std::string &code)
{
    if (stat_id < 0 || stat_id > 255 || addon.empty() || code.empty())
        return;
    std::lock_guard g(track_mu);
    tracks[int(stat_id)] = addon + "/" + code;
    has_tracks = true;
}

}
