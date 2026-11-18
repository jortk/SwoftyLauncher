// Host-native unit test for the core's tile list select guard (native/src/core/tile_list_select_logic.hpp).
// Built and run by native/test/run_tile_list_select_test.sh.

#include "tile_list_select_logic.hpp"

#include <cstdio>
#include <cstring>
#include <map>
#include <vector>

namespace ts = runtime::tile_list_select;

static int failed = 0;

static void expect(bool ok, const char *what)
{
    std::printf("%s: %s\n", ok ? "ok  " : "FAIL", what);
    if (!ok)
        failed++;
}

struct Memory {
    std::map<uint64_t, uint64_t> words;
};

static bool read(uint64_t address, void *out, size_t n, void *user)
{
    Memory *m = static_cast<Memory *>(user);
    if (n != 8)
        return false;
    auto it = m->words.find(address);
    if (it == m->words.end())
        return false;
    memcpy(out, &it->second, 8);
    return true;
}

constexpr uint64_t CHILDREN = 0x2ea1c0640;
constexpr uint64_t VTABLE = 0x1458a0000;

struct Tile {
    uint64_t at = 0;
    uint64_t components = 0;
    uint64_t component = 0;
};

static Tile add_tile(Memory &m, uint64_t slot, uint64_t at, bool with_components)
{
    Tile t{at, with_components ? at + 0x1000 : 0, with_components ? at + 0x2000 : 0};
    m.words[CHILDREN + slot * 8] = at;
    m.words[at + ts::COMPONENTS_AT] = t.components;
    if (with_components) {
        m.words[t.components] = t.component;
        m.words[t.component] = VTABLE;
    }
    return t;
}

static void children_with_one_bare_tile()
{
    Memory m;
    add_tile(m, 0, 0x20590000, true);
    add_tile(m, 1, 0x20599e60, false);
    add_tile(m, 2, 0x205a0000, true);
    expect(ts::check_child(CHILDREN, read, &m) == ts::Child::Usable, "a tile with components runs the game's walk");
    expect(ts::check_child(CHILDREN + 8, read, &m) == ts::Child::NoComponents,
           "the GDPika tile (rdi 0x20599e60, [rdi+0x30] = 0) is skipped as having no component array");
    expect(ts::check_child(CHILDREN + 16, read, &m) == ts::Child::Usable, "the tile after the bare one is walked again");
}

static void unreadable_children()
{
    Memory m;
    expect(ts::check_child(CHILDREN, read, &m) == ts::Child::Unreadable, "an unreadable cursor skips the child");
    m.words[CHILDREN] = 0;
    expect(ts::check_child(CHILDREN, read, &m) == ts::Child::Unreadable, "a null child is skipped");
    m.words[CHILDREN] = 0x2a00002f33000020ull;
    expect(ts::check_child(CHILDREN, read, &m) == ts::Child::Unreadable, "a non-canonical child is skipped");
    m.words[CHILDREN] = 0x20599e60;
    expect(ts::check_child(CHILDREN, read, &m) == ts::Child::Unreadable, "a child whose +0x30 does not read is skipped");
}

static void broken_component_arrays()
{
    Memory m;
    m.words[CHILDREN] = 0x20599e60;
    m.words[0x20599e60 + ts::COMPONENTS_AT] = 0x30000000;
    expect(ts::check_child(CHILDREN, read, &m) == ts::Child::NoComponent, "an unreadable component array is skipped");
    m.words[0x30000000] = 0;
    expect(ts::check_child(CHILDREN, read, &m) == ts::Child::NoComponent, "a null first component is skipped");
    m.words[0x30000000] = 0x30001000;
    expect(ts::check_child(CHILDREN, read, &m) == ts::Child::NoComponent,
           "a first component whose vtable does not read is skipped");
    m.words[0x30001000] = VTABLE;
    expect(ts::check_child(CHILDREN, read, &m) == ts::Child::Usable, "a readable first component runs the game's walk");
    m.words[0x20599e60 + ts::COMPONENTS_AT] = 0x8;
    expect(ts::check_child(CHILDREN, read, &m) == ts::Child::NoComponent, "a component array below user space is skipped");
}

static void positions()
{
    uint64_t end = CHILDREN + 190 * 8;
    ts::Position p = ts::position(CHILDREN + 8, end, CHILDREN);
    expect(p.known && p.index == 1 && p.count == 190, "the log names the tile's index among the list's children");
    p = ts::position(end, end, CHILDREN);
    expect(p.known && p.index == 190 && p.count == 190, "a cursor at the end still reads as a position");
    expect(!ts::position(CHILDREN - 8, end, CHILDREN).known, "a cursor before the children has no position");
    expect(!ts::position(CHILDREN + 4, end, CHILDREN).known, "a misaligned cursor has no position");
    expect(!ts::position(end + 8, end, CHILDREN).known, "a cursor past the end has no position");
    expect(!ts::position(CHILDREN, end, 0).known, "an unread children begin has no position");
}

static void walk(bool guard)
{
    Memory m;
    std::vector<bool> bare = {false, false, true, false, true};
    for (size_t i = 0; i < bare.size(); i++)
        add_tile(m, i, 0x20590000 + i * 0x10000, !bare[i]);
    int visited = 0, faults = 0, skipped = 0;
    for (size_t i = 0; i < bare.size(); i++) {
        uint64_t cursor = CHILDREN + i * 8;
        if (guard && ts::check_child(cursor, read, &m) != ts::Child::Usable) {
            skipped++;
            continue;
        }
        uint64_t child = 0, components = 0;
        read(cursor, &child, 8, &m);
        read(child + ts::COMPONENTS_AT, &components, 8, &m);
        if (!components) {
            faults++;
            break;
        }
        visited++;
    }
    if (guard)
        expect(faults == 0 && skipped == 2 && visited == 3, "with the guard the walk visits every usable tile and skips the bare ones");
    else
        expect(faults == 1 && visited == 2, "without the guard the walk faults on the first bare tile");
}

static void focused_tile()
{
    constexpr uint64_t WIDGET = 0x1436b018, BARE = 0x11ba7660;
    Memory m;
    Tile first = add_tile(m, 0, 0x20590000, true);
    add_tile(m, 1, BARE, false);
    m.words[WIDGET + ts::COMPONENTS_AT] = first.components;
    m.words[WIDGET + ts::CHILDREN_BEGIN_AT] = CHILDREN;
    m.words[WIDGET + ts::CHILDREN_END_AT] = CHILDREN + 2 * 8;
    expect(ts::check_tile(WIDGET, read, &m) == ts::Child::Usable, "a list widget with components passes");
    expect(ts::check_tile(first.at, read, &m) == ts::Child::Usable, "a focused tile with components is written to the selection");
    expect(ts::check_tile(BARE, read, &m) == ts::Child::NoComponents,
           "the GDPika focused tile (rsi 0x11ba7660, [rsi+0x30] = 0) is not written to the selection");
    expect(ts::check_tile(0, read, &m) == ts::Child::Unreadable, "a null node is not usable");
    expect(ts::check_tile(0x30000000, read, &m) == ts::Child::Unreadable, "a node that does not read is not usable");
    ts::Position p = ts::find_child(WIDGET, BARE, read, &m);
    expect(p.known && p.index == 1 && p.count == 2, "the log names the focused tile's index among the list's children");
    expect(!ts::find_child(WIDGET, 0x20600000, read, &m).known, "a node that is not a child has no position");
    expect(!ts::find_child(0, BARE, read, &m).known, "a null widget has no position");
    m.words[WIDGET + ts::CHILDREN_END_AT] = CHILDREN + (ts::MAX_CHILDREN + 1) * 8;
    expect(!ts::find_child(WIDGET, BARE, read, &m).known, "an implausible child count is not walked");
    m.words[WIDGET + ts::CHILDREN_END_AT] = CHILDREN - 8;
    expect(!ts::find_child(WIDGET, BARE, read, &m).known, "children that end before they begin are not walked");
}

static void messages()
{
    expect(!strcmp(ts::why(ts::Child::NoComponents), "has no component array"), "the no-components skip names its cause");
    expect(!strcmp(ts::why(ts::Child::Unreadable), "is not readable"), "the unreadable skip names its cause");
}

int main()
{
    children_with_one_bare_tile();
    unreadable_children();
    broken_component_arrays();
    positions();
    walk(false);
    walk(true);
    focused_tile();
    messages();
    if (failed) {
        std::printf("%d check(s) failed\n", failed);
        return 1;
    }
    std::printf("all tile list select checks passed\n");
    return 0;
}
