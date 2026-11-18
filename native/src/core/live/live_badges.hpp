#pragma once
#include "live_mem.hpp"
#include <algorithm>
#include <vector>
#include <cstring>

namespace swofty_core::live {
// The HUD resolves SaveSlotGameModeComponent from the mode's save-data root.
// Read the type getter's checked LEA/RET metadata instead of invoking game code.
inline bool current_badge_slot(const Mem &mem, uintptr_t game, uintptr_t image_lo, uintptr_t image_hi, uint64_t &slot)
{
    uintptr_t root = 0, range[2]{};
    if (!game || !mem.get(game + 0xf08, root) || !root || (root & 7) ||
        !mem.read(root + 0x238, range, sizeof range) || range[1] < range[0] ||
        (range[1] - range[0]) % 8 || (range[1] - range[0]) / 8 > 128 || (range[0] & 7)) return false;
    std::vector<uintptr_t> components{root};
    for (uintptr_t at = range[0]; at < range[1]; at += 8) {
        uintptr_t component = 0;
        if (!mem.get(at, component)) return false;
        components.push_back(component);
    }
    bool found = false;
    for (uintptr_t component : components) {
        uintptr_t vt = 0, fn = 0, label = 0;
        uint8_t code[8]{};
        if (!component || (component & 7) || !mem.get(component + 0x18, vt) || !vt ||
            !mem.get(vt + 0x28, fn) || !fn || (image_hi && (fn < image_lo || fn > image_hi - 8)) ||
            !mem.read(fn, code, sizeof code) || code[0] != 0x48 || code[1] != 0x8d || code[2] != 0x05 || code[7] != 0xc3)
            continue;
        int32_t displacement = 0;
        std::memcpy(&displacement, code + 3, sizeof displacement);
        uintptr_t descriptor = fn + 7 + displacement;
        if ((image_hi && (descriptor < image_lo || descriptor > image_hi - 16)) ||
            !mem.get(descriptor + 8, label)) continue;
        char name[32]{};
        if (!mem.read(label, name, sizeof name) || std::memcmp(name, "SaveSlotGameModeComponent", 26)) continue;
        uintptr_t current = 0;
        if (found || !mem.get(component + 0x280, current) || !current || (current & 7) || !mem.get(current, slot)) return false;
        found = true;
    }
    return found;
}

// PlayerCustomisationInfoSystem's saved assignments: a mode list containing
// slot-keyed trees. Matches the HUD's GetAssignedBadge input, not the entrant's
// fallback. Corroborated against Steam 26566308's live mode 79 / slot 11.
inline bool assigned_badge(const Mem &mem, uintptr_t system, uint32_t mode, uint64_t slot, uint32_t &badge)
{
    uintptr_t head = 0, node = 0;
    uint64_t count = 0;
    if (!system || !mem.get(system + 0x120, head) || !head || (head & 7) ||
        !mem.get(system + 0x128, count) || count > 128 || !mem.get(head, node))
        return false;
    std::vector<uintptr_t> seen;
    bool found = false;
    for (uint64_t i = 0; i < count; ++i) {
        if (!node || (node & 7) || node == head || std::find(seen.begin(), seen.end(), node) != seen.end())
            return false;
        seen.push_back(node);
        uint32_t id = 0;
        if (!mem.get(node + 0x10, id)) return false;
        if (id == mode) {
            if (found) return false;
            uintptr_t tree = 0, at = 0;
            uint64_t size = 0;
            if (!mem.get(node + 0x20, tree) || !tree || (tree & 7) || !mem.get(node + 0x28, size) ||
                size > 4096 || !mem.get(tree + 8, at)) return false;
            std::vector<uintptr_t> visited;
            for (uint64_t j = 0; at && at != tree && j < size; ++j) {
                if ((at & 7) || std::find(visited.begin(), visited.end(), at) != visited.end()) return false;
                visited.push_back(at);
                uint8_t nil = 0;
                uint64_t key = 0;
                if (!mem.get(at + 0x19, nil)) return false;
                if (nil) break;
                if (!mem.get(at + 0x20, key)) return false;
                if (key == slot) {
                    if (!mem.get(at + 0x28, badge)) return false;
                    found = true;
                    break;
                }
                if (!mem.get(at + (slot < key ? 0 : 0x10), at)) return false;
            }
            if (!found) return false;
        }
        if (!mem.get(node, node)) return false;
    }
    return node == head && found;
}
}
