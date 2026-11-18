// Host-native unit test for the core's registered HUD leaf table (native/src/core/hud_leaves_logic.hpp):
// registration and type rules, change detection, group atomicity under the per-tick cap, deferral without
// drops, rebuild resend and failed-leaf retry. Built and run by native/test/run_hud_leaves_test.sh.

#include "hud_leaves_logic.hpp"

#include <cstdio>

namespace hl = runtime::hud_leaves;

static int g_fail;

#define CHECK(c)                                                                                                       \
    do {                                                                                                               \
        if (!(c)) {                                                                                                    \
            std::printf("FAIL %s:%d %s\n", __FILE__, __LINE__, #c);                                                    \
            g_fail++;                                                                                                  \
        }                                                                                                              \
    } while (0)

struct Rec {
    std::vector<std::string> log;
    bool ok = true;
    hl::Table::Apply fn()
    {
        return [this](const hl::Leaf &l) {
            if (ok)
                log.push_back(l.path + "=" + (l.type == hl::TEXT ? l.want_text : std::to_string(l.want_num)));
            return ok;
        };
    }
};

static uint32_t reg(hl::Table &t, const char *path, uint32_t type, uint32_t cap = 0)
{
    uint32_t id = ~0u;
    CHECK(t.register_leaf(path, type, cap, "test", id) == hl::OK);
    return id;
}

static void set_int(hl::Table &t, uint32_t id, int32_t v)
{
    const void *p[] = {&v};
    t.set(&id, p, 1);
}

static void set_text(hl::Table &t, uint32_t id, const char *v)
{
    const void *p[] = {v};
    t.set(&id, p, 1);
}

int main()
{
    uint32_t bad;
    hl::Table t;
    CHECK(t.register_leaf("ui_menu_data.other", hl::INT, 0, "x", bad) == hl::INVALID);
    CHECK(t.register_leaf("view_model_data.hud_x_a b", hl::INT, 0, "x", bad) == hl::INVALID);
    CHECK(t.register_leaf("view_model_data.hud_x_a", 9, 0, "x", bad) == hl::INVALID);
    uint32_t a = reg(t, "view_model_data.hud_x_a", hl::INT);
    CHECK(reg(t, "view_model_data.hud_x_a", hl::INT) == a);
    CHECK(t.register_leaf("view_model_data.hud_x_a", hl::BOOL, 0, "x", bad) == hl::INVALID);
    CHECK(t.type_of("view_model_data.hud_x_a") == hl::INT);
    CHECK(t.type_of("view_model_data.hud_x_nope") == -1);

    Rec r;
    set_int(t, a, 5);
    t.drain(1, r.fn());
    CHECK(r.log.size() == 1 && r.log[0] == "view_model_data.hud_x_a=5");
    set_int(t, a, 5);
    t.drain(2, r.fn());
    CHECK(r.log.size() == 1);
    set_int(t, a, 6);
    set_int(t, a, 5);
    t.drain(3, r.fn());
    CHECK(r.log.size() == 1);
    set_int(t, a, 7);
    set_int(t, a, 8);
    t.drain(4, r.fn());
    CHECK(r.log.size() == 2 && r.log[1].ends_with("=8"));

    uint32_t b = reg(t, "view_model_data.hud_x_b", hl::BOOL);
    set_int(t, b, 41);
    t.drain(5, r.fn());
    CHECK(r.log.back().ends_with("_b=1"));

    uint32_t s = reg(t, "view_model_data.hud_x_s", hl::TEXT, 4);
    set_text(t, s, "abcdefgh");
    t.drain(6, r.fn());
    CHECK(r.log.back().ends_with("_s=abcd"));

    hl::Table g;
    g.set_cap(10);
    std::vector<uint32_t> ids;
    for (int i = 0; i < 25; i++)
        ids.push_back(reg(g, ("view_model_data.hud_g_" + std::to_string(i)).c_str(), hl::INT));
    Rec gr;
    for (int row = 0; row < 5; row++) {
        g.begin();
        for (int c = 0; c < 5; c++)
            set_int(g, ids[row * 5 + c], row + 1);
        g.end();
    }
    g.drain(1, gr.fn());
    CHECK(gr.log.size() == 10);
    CHECK(g.stats().queued == 15 && g.stats().applied_last_tick == 10);
    g.drain(2, gr.fn());
    CHECK(gr.log.size() == 20);
    g.drain(3, gr.fn());
    CHECK(gr.log.size() == 25 && g.stats().queued == 0);

    hl::Table o;
    o.set_cap(3);
    std::vector<uint32_t> big;
    for (int i = 0; i < 8; i++)
        big.push_back(reg(o, ("view_model_data.hud_o_" + std::to_string(i)).c_str(), hl::INT));
    o.begin();
    for (auto id : big)
        set_int(o, id, 1);
    o.end();
    Rec orr;
    o.drain(1, orr.fn());
    CHECK(orr.log.size() == 8);

    hl::Table f;
    uint32_t fa = reg(f, "view_model_data.hud_f_a", hl::INT);
    set_int(f, fa, 1);
    Rec fr;
    fr.ok = false;
    f.drain(10, fr.fn());
    CHECK(f.stats().failed == 1);
    fr.ok = true;
    f.drain(500, fr.fn());
    CHECK(fr.log.empty());
    f.drain(1500, fr.fn());
    CHECK(fr.log.size() == 1 && f.stats().failed == 0);

    uint32_t gen = t.generation();
    t.rebuilt();
    CHECK(t.generation() == gen + 1);
    Rec rb;
    t.drain(20, rb.fn());
    CHECK(rb.log.size() == 3);

    Rec rs;
    t.drain(30000, rs.fn());
    t.drain(60000, rs.fn());
    CHECK(rs.log.size() >= 3 && t.stats().resyncs >= 1);

    uint32_t ev[2] = {a, 999};
    int32_t one = 1;
    const void *vals[2] = {&one, &one};
    CHECK(t.set(ev, vals, 2) == hl::INVALID);
    CHECK(t.parents().size() == 1);

    if (g_fail)
        return 1;
    std::printf("hud leaves test: ok\n");
    return 0;
}
