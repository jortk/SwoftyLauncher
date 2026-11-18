#pragma once

#include <cstdint>
#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

// The active presence's user id and the game's serialised form of an id (the `L bytes` inside a ghost lap description).
//
// The presence user object (record+0x1c0, vtable base+0x5a5b900) hands out its id through its vtable slot 7 (FUN_14411e3c0,
// out = a 0x40 byte id struct): type byte 1; value0 = object[+0x28] ^ object[+0x20] (the id; a Steam presence holds the SteamID64 here);
// value1 and value2 come from two platform service strings (strtoull), zero when the service gave none; flag39 = 0 when
// object[+0x18] == 0 else 1 (and then the object gives no values: the getter's second branch leaves value0..2 zero); flag38 = 0. Ids of type 1 are compared by Ne_UserId_Equals (FUN_144122660): same type, same flag39 and
// the same value0; value1 and value2 are not compared. A leaderboard row becomes the same id through FUN_143282d40: Platform 1
// gives type 1 with value0 = the decimal PlatformID string and flag39 = 0.
//
// The serialised id (FUN_1441227c0 writer, FUN_144123100 reader), bit i = byte i/8, bit i%8, 64 bit values little endian:
//   bit 0            mode: 0 plain (1 makes the reader expect a type tag in front of every field and the description is rejected)
//   4 bits           type (reader refuses > 7)
//   1 bit            value0 present (written when value0 != 0); then 64 bits
//   1 bit            values 1 and 2 present (written when either != 0); then 64 bits + 64 bits
//   1 bit            flag39; types other than 3 stop here, type 3 has one more bit (flag38)
namespace runtime::presence_id {

struct Id {
    uint8_t type = 0;
    uint64_t value[3] = {};
    bool flag39 = false, flag38 = false;
};

struct Object {
    uint64_t at18 = 0, at20 = 0, at28 = 0;
};

inline Id from_presence_object(const Object &o)
{
    Id id;
    id.type = 1;
    id.flag39 = o.at18 != 0;
    id.value[0] = id.flag39 ? 0 : o.at28 ^ o.at20;
    return id;
}

inline void put_bits(std::vector<uint8_t> &out, size_t &pos, uint64_t v, int n)
{
    for (int i = 0; i < n; i++, pos++) {
        if ((pos >> 3) >= out.size())
            out.push_back(0);
        if ((v >> i) & 1)
            out[pos >> 3] |= uint8_t(1u << (pos & 7));
    }
}

inline std::vector<uint8_t> serialise(const Id &id)
{
    std::vector<uint8_t> out;
    size_t pos = 0;
    put_bits(out, pos, 0, 1);
    put_bits(out, pos, id.type, 4);
    bool v0 = id.value[0] != 0;
    put_bits(out, pos, v0, 1);
    if (v0)
        put_bits(out, pos, id.value[0], 64);
    bool pair = id.value[1] != 0 || id.value[2] != 0;
    put_bits(out, pos, pair, 1);
    if (pair) {
        put_bits(out, pos, id.value[1], 64);
        put_bits(out, pos, id.value[2], 64);
    }
    put_bits(out, pos, id.flag39, 1);
    if (id.type == 3)
        put_bits(out, pos, id.flag38, 1);
    return out;
}

inline bool get_bits(const std::vector<uint8_t> &in, size_t &pos, int n, uint64_t &v)
{
    v = 0;
    if (pos + size_t(n) > in.size() * 8)
        return false;
    for (int i = 0; i < n; i++, pos++)
        if ((in[pos >> 3] >> (pos & 7)) & 1)
            v |= uint64_t(1) << i;
    return true;
}

inline bool deserialise(const std::vector<uint8_t> &in, Id &id)
{
    id = Id{};
    size_t pos = 0;
    uint64_t v;
    if (!get_bits(in, pos, 1, v) || v != 0 || !get_bits(in, pos, 4, v) || v > 7)
        return false;
    id.type = uint8_t(v);
    if (id.type == 0)
        return false;
    if (!get_bits(in, pos, 1, v))
        return false;
    if (v && !get_bits(in, pos, 64, id.value[0]))
        return false;
    if (!get_bits(in, pos, 1, v))
        return false;
    if (v && (!get_bits(in, pos, 64, id.value[1]) || !get_bits(in, pos, 64, id.value[2])))
        return false;
    if (!get_bits(in, pos, 1, v))
        return false;
    id.flag39 = v != 0;
    if (id.type == 3) {
        if (!get_bits(in, pos, 1, v))
            return false;
        id.flag38 = v != 0;
    }
    return true;
}

inline bool same_user(const Id &a, const Id &b)
{
    if (a.type != b.type)
        return false;
    if (a.type == 3)
        return a.flag38 == b.flag38 && (a.flag38 || a.value[0] == b.value[0]);
    return a.flag39 == b.flag39 && a.value[0] == b.value[0];
}

inline std::string base64(const std::vector<uint8_t> &b)
{
    static const char *t = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    std::string s;
    for (size_t i = 0; i < b.size(); i += 3) {
        uint32_t w = uint32_t(b[i]) << 16 | (i + 1 < b.size() ? uint32_t(b[i + 1]) << 8 : 0) | (i + 2 < b.size() ? b[i + 2] : 0);
        s += t[w >> 18 & 63];
        s += t[w >> 12 & 63];
        s += i + 1 < b.size() ? t[w >> 6 & 63] : '=';
        s += i + 2 < b.size() ? t[w & 63] : '=';
    }
    return s;
}

inline int platform_of(uint8_t type)
{
    return type == 1 ? 1 : 0;
}

inline std::string hex(const std::vector<uint8_t> &b)
{
    std::string s;
    char h[4];
    for (uint8_t c : b) {
        snprintf(h, sizeof h, "%02x", c);
        s += h;
    }
    return s;
}

}
