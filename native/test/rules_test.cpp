#include "postfx_logic.hpp"

#include <cmath>
#include <cstdio>
#include <fstream>
#include <sstream>

using namespace postfx;

static int failed;

#define CHECK(c)                                                                                                       \
    do {                                                                                                               \
        if (!(c)) {                                                                                                    \
            std::printf("FAIL %s:%d: %s\n", __FILE__, __LINE__, #c);                                                   \
            failed++;                                                                                                  \
        }                                                                                                              \
    } while (0)

static bool near(double a, double b, double tol = 1e-4)
{
    return std::fabs(a - b) <= tol;
}

static const double FRAME = 10;

struct Rig {
    ActiveSpec spec;
    Plan plan;
    RuleEngine engine;
    Signals sig;

    Rig(const std::string &rules_json, std::vector<std::vector<float>> instances = {{0, 0, 0, 0, 0, 0, 0, 0}})
    {
        std::string text = "{\"instances\":[";
        for (size_t i = 0; i < instances.size(); i++) {
            text += std::string(i ? "," : "") + "{\"effect\":\"fx\",\"label\":\"l" + std::to_string(i) + "\",\"params\":[";
            for (size_t k = 0; k < instances[i].size(); k++)
                text += (k ? "," : "") + std::to_string(instances[i][k]);
            text += "]}";
        }
        text += "],\"rules\":" + rules_json + "}";
        CHECK(parse_active(text, spec));
        EffectDecl fx;
        fx.id = "fx";
        fx.slots = 8;
        PassDecl pd;
        pd.name = "p";
        pd.dxil = "p.dxil";
        fx.passes.push_back(pd);
        static std::vector<EffectDecl> effects;
        effects = {fx};
        plan = make_plan(spec, effects);
        engine.reset(spec.rules, plan);
    }

    void run(double ms, double frame = FRAME)
    {
        for (double t = 0; t < ms - 1e-9; t += frame) {
            engine.step(sig, frame);
            sig.clear_events();
        }
    }
};

static std::string rule(const std::string &when, const std::string &extra, const std::string &set,
                        const std::string &name = "r", int priority = 0, int order = 0, bool event = false)
{
    return "{\"name\":\"" + name + "\",\"priority\":" + std::to_string(priority) + ",\"order\":" + std::to_string(order) +
           ",\"event\":" + (event ? "true" : "false") + ",\"when\":[" + when + "]," + extra + "\"set\":[" + set + "]}";
}

static const char *SPEED_ABOVE = "{\"signal\":\"car.speed\",\"op\":\"above\",\"value\":150}";
static const char *BLEND1 = "{\"instance\":0,\"slot\":1,\"mode\":\"blend\",\"values\":[1]}";

static void own_cases()
{
    {
        ActiveSpec a;
        CHECK(parse_active("{\"instances\":[{\"effect\":\"fx\"}],\"future\":1}", a));
        CHECK(a.rules.empty() && a.instances.size() == 1);
        CHECK(parse_active("{\"rules\":[{\"name\":\"x\",\"unknown\":3,\"when\":[{\"signal\":\"nope\",\"op\":\"true\"}]},"
                           "{\"name\":\"ok\",\"extra\":[1],\"when\":[{\"signal\":\"view.paused\",\"op\":\"true\"}],\"set\":[]}]}",
                           a));
        CHECK(a.rules.size() == 1 && a.rules[0].name == "ok" && a.rules[0].for_ms == 400 && a.rules[0].fade_in_ms == 150 &&
              a.rules[0].fade_out_ms == 300);
    }
    {
        CHECK(std::string(SIGNALS[S_CAR_SURFACE].id) == "car.surface" && SIGNALS[S_SESSION_CHEQUERED].kind == SK_EVENT);
        CHECK(signal_index("session.lap") == S_SESSION_LAP && signal_value_name(S_CAR_SURFACE, 1) == "kerb");
        CHECK(signal_value_name(S_VIEW_CAMERA, 5) == "other" && signal_value_name(S_VIEW_CAMERA, 6).empty());
        CHECK(SIGNALS[S_CAR_SPEED].in_game && !SIGNALS[S_CAR_IMPACT].in_game && SIGNALS[S_SESSION_LAP].in_game);
    }
    {
        Rig r("[" + rule(SPEED_ABOVE, "\"fade_in_ms\":100,\"fade_out_ms\":200,", BLEND1) + "]");
        r.sig.set(S_CAR_SPEED, 200);
        r.run(50);
        CHECK(near(r.engine.state(0).w, 0.5) && near(r.engine.options(0)[1], 0.5));
        r.run(60);
        CHECK(r.engine.state(0).w == 1 && near(r.engine.options(0)[1], 1));
        r.sig.set(S_CAR_SPEED, 100);
        r.run(100);
        CHECK(near(r.engine.state(0).w, 0.5) && near(r.engine.options(0)[1], 0.5));
        r.run(200);
        CHECK(r.engine.state(0).w == 0 && r.engine.options(0)[1] == 0);
    }
    {
        Rig r("[" + rule(SPEED_ABOVE, "\"min_ms\":40,\"hold_ms\":100,\"fade_in_ms\":0,\"fade_out_ms\":0,", BLEND1) + "]");
        r.sig.set(S_CAR_SPEED, 200);
        r.run(30);
        CHECK(!r.engine.state(0).on);
        r.run(10);
        CHECK(r.engine.state(0).on && r.engine.state(0).w == 1);
        r.sig.set(S_CAR_SPEED, 0);
        r.run(100);
        CHECK(r.engine.state(0).on);
        r.run(10);
        CHECK(!r.engine.state(0).on && r.engine.state(0).w == 0);
        r.sig.clear();
        r.sig.set(S_CAR_SPEED, 200);
        r.run(40);
        CHECK(r.engine.state(0).on);
        r.sig.has[S_CAR_SPEED] = false;
        r.run(200);
        CHECK(!r.engine.state(0).on);
    }
    {
        std::string when = "{\"signal\":\"car.surface\",\"op\":\"is\",\"values\":[1,2],\"wheels\":2}";
        Rig r("[" + rule(when, "\"fade_in_ms\":0,\"fade_out_ms\":0,", BLEND1) + "]");
        int one[4] = {1, 0, 0, 0}, two[4] = {1, 2, 0, 0}, none[4] = {0, 0, 3, 4};
        r.sig.set_surface(one);
        r.run(10);
        CHECK(!r.engine.state(0).on);
        r.sig.set_surface(two);
        r.run(10);
        CHECK(r.engine.state(0).on);
        r.sig.set_surface(none);
        r.run(10);
        CHECK(!r.engine.state(0).on);
        r.sig.clear();
        r.run(10);
        CHECK(!r.engine.state(0).on);
    }
    {
        std::string ramp = "\"ramp\":{\"signal\":\"car.speed\",\"from\":250,\"to\":320},\"fade_in_ms\":0,\"fade_out_ms\":0,";
        Rig r("[" + rule("", ramp, BLEND1) + "]");
        r.sig.set(S_CAR_SPEED, 285);
        r.run(10);
        CHECK(near(r.engine.state(0).target, 0.5) && near(r.engine.state(0).a, 0.5) && near(r.engine.options(0)[1], 0.5));
        r.sig.set(S_CAR_SPEED, 100);
        r.run(10);
        CHECK(!r.engine.state(0).on && r.engine.options(0)[1] == 0);
        r.sig.set(S_CAR_SPEED, 400);
        r.run(10);
        CHECK(near(r.engine.options(0)[1], 1));
        r.sig.has[S_CAR_SPEED] = false;
        r.run(10);
        CHECK(r.engine.options(0)[1] == 0);
    }
    {
        std::string when = "{\"signal\":\"car.impact\",\"op\":\"above\",\"value\":3}";
        Rig r("[" + rule(when, "\"for_ms\":100,\"fade_in_ms\":0,\"fade_out_ms\":0,", BLEND1, "hit", 0, 0, true) + "]");
        r.sig.add_event(S_CAR_IMPACT, 2);
        r.run(10);
        CHECK(!r.engine.state(0).on);
        r.sig.add_event(S_CAR_IMPACT, 2);
        r.sig.add_event(S_CAR_IMPACT, 5);
        r.run(10);
        CHECK(r.engine.state(0).on);
        r.run(90);
        CHECK(r.engine.state(0).on);
        r.run(10);
        CHECK(!r.engine.state(0).on);
        Rig any("[" + rule("{\"signal\":\"session.lap\",\"op\":\"event\"}", "\"for_ms\":50,\"fade_in_ms\":0,\"fade_out_ms\":0,",
                           BLEND1, "lap", 0, 0, true) + "]");
        any.sig.add_event(S_SESSION_LAP);
        any.run(10);
        CHECK(any.engine.state(0).on);
        any.run(40);
        CHECK(any.engine.state(0).on);
        any.run(10);
        CHECK(!any.engine.state(0).on);
    }
    {
        std::string a = "{\"signal\":\"car.speed\",\"op\":\"above\",\"value\":100},"
                        "{\"signal\":\"car.speed\",\"op\":\"below\",\"value\":200},"
                        "{\"signal\":\"view.paused\",\"op\":\"true\"}";
        Rig r("[" + rule(a, "\"fade_in_ms\":0,\"fade_out_ms\":0,", BLEND1) + "]");
        r.sig.set(S_CAR_SPEED, 150);
        r.run(10);
        CHECK(!r.engine.state(0).on);
        r.sig.set(S_VIEW_PAUSED, 1);
        r.run(10);
        CHECK(r.engine.state(0).on);
        r.sig.set(S_CAR_SPEED, 250);
        r.run(10);
        CHECK(!r.engine.state(0).on);
    }
    {
        std::string low = rule(SPEED_ABOVE, "\"fade_in_ms\":0,\"fade_out_ms\":0,",
                               "{\"instance\":0,\"slot\":1,\"mode\":\"blend\",\"values\":[1]}", "low", 0, 1);
        std::string high = rule(SPEED_ABOVE, "\"fade_in_ms\":0,\"fade_out_ms\":0,",
                                "{\"instance\":0,\"slot\":1,\"mode\":\"blend\",\"values\":[3]}", "high", 5, 0);
        Rig r("[" + high + "," + low + "]");
        r.sig.set(S_CAR_SPEED, 200);
        r.run(10);
        CHECK(near(r.engine.options(0)[1], 3));
        std::string a = rule(SPEED_ABOVE, "\"fade_in_ms\":0,\"fade_out_ms\":0,",
                             "{\"instance\":0,\"slot\":2,\"mode\":\"blend\",\"values\":[10]}", "a", 0, 0);
        std::string b = rule(SPEED_ABOVE, "\"fade_in_ms\":0,\"fade_out_ms\":0,",
                             "{\"instance\":0,\"slot\":2,\"mode\":\"blend\",\"values\":[20]}", "b", 0, 1);
        Rig o("[" + b + "," + a + "]");
        o.sig.set(S_CAR_SPEED, 200);
        o.run(10);
        CHECK(near(o.engine.options(0)[2], 20));
    }
    {
        std::string sets = "{\"instance\":0,\"slot\":0,\"mode\":\"switch\",\"values\":[1]},"
                           "{\"instance\":0,\"slot\":1,\"mode\":\"round\",\"values\":[10]},"
                           "{\"instance\":0,\"slot\":2,\"mode\":\"blend\",\"values\":[1,2,3]},"
                           "{\"instance\":1,\"enabled\":false}";
        Rig r("[" + rule(SPEED_ABOVE, "\"fade_in_ms\":100,\"fade_out_ms\":100,", sets) + "]",
              {{0, 0, 0, 0, 0, 0, 0, 0}, {0, 0, 0, 0, 0, 0, 0, 0}});
        r.sig.set(S_CAR_SPEED, 200);
        r.run(30);
        double a = r.engine.state(0).a;
        CHECK(near(a, 0.3 * 0.3 * (3 - 0.6)));
        CHECK(r.engine.options(0)[0] == 0 && !r.engine.off(1));
        CHECK(r.engine.options(0)[1] == std::round(10 * float(a)));
        CHECK(near(r.engine.options(0)[2], a) && near(r.engine.options(0)[3], 2 * a) && near(r.engine.options(0)[4], 3 * a));
        r.run(20);
        CHECK(r.engine.options(0)[0] == 1 && r.engine.off(1));
        CHECK(r.engine.options(1)[0] == 0);
    }
    {
        Rig r("[" + rule(SPEED_ABOVE, "\"fade_in_ms\":1000,\"fade_out_ms\":1000,", BLEND1) + "]");
        r.sig.set(S_CAR_SPEED, 200);
        r.engine.step(r.sig, 5000);
        CHECK(near(r.engine.state(0).w, 0.1));
        r.engine.step(r.sig, -50);
        CHECK(near(r.engine.state(0).w, 0.1));
        r.engine.reset(r.spec.rules, r.plan);
        CHECK(!r.engine.state(0).on && r.engine.state(0).w == 0);
    }
    {
        Rig r("[" + rule("{\"signal\":\"car.surface\",\"op\":\"is\",\"values\":[1]}", "", "{\"instance\":9,\"slot\":0,\"mode\":\"blend\",\"values\":[1]}") + "]");
        int k[4] = {1, 1, 1, 1};
        r.sig.set_surface(k);
        r.run(1000);
        CHECK(r.engine.state(0).on && r.engine.options(0)[0] == 0);
    }
}

static const json::Value &child(const json::Value &v, const char *k)
{
    return v[k];
}

static int value_index(int signal, const json::Value &v)
{
    if (v.type == json::Value::Num)
        return int(v.n);
    std::string name = v.str();
    for (size_t i = 0; i < 32; i++) {
        std::string_view n = signal_value_name(signal, i);
        if (n.empty())
            break;
        if (n == name)
            return int(i);
    }
    return -1;
}

static void vector_cases(const char *path)
{
    std::ifstream f(path);
    if (!f) {
        std::printf("no %s; only the built-in cases ran\n", path);
        return;
    }
    std::stringstream ss;
    ss << f.rdbuf();
    json::Value root;
    if (!json::parse(ss.str(), root)) {
        std::printf("FAIL %s is not json\n", path);
        failed++;
        return;
    }
    int cases = 0;
    for (auto &c : child(root, "cases").a) {
        cases++;
        std::string name = c["name"].str();
        ActiveSpec spec;
        const json::Value &base = c["base"];
        Plan plan;
        for (size_t i = 0; i < base.a.size(); i++) {
            PlanInstance pi;
            pi.source = int(i);
            for (size_t k = 0; k < SLOTS; k++)
                pi.options[k] = k < base.a[i].a.size() ? float(base.a[i].a[k].n) : 0.0f;
            plan.instances.push_back(std::move(pi));
        }
        for (auto &r : c["rules"].a) {
            RuleSpec rs;
            if (!parse_rule(r, rs)) {
                std::printf("FAIL %s: rule does not parse\n", name.c_str());
                failed++;
                continue;
            }
            spec.rules.push_back(std::move(rs));
        }
        RuleEngine engine;
        engine.reset(spec.rules, plan);
        size_t n = 0;
        for (auto &fr : c["frames"].a) {
            n++;
            Signals sig;
            for (auto &[id, v] : fr["signals"].o) {
                int s = signal_index(id);
                if (s < 0)
                    continue;
                if (v.type == json::Value::Arr && s == S_CAR_SURFACE) {
                    int w[4] = {-1, -1, -1, -1};
                    for (size_t k = 0; k < 4 && k < v.a.size(); k++)
                        w[k] = value_index(s, v.a[k]);
                    sig.set_surface(w);
                } else if (v.type == json::Value::Arr) {
                    for (auto &m : v.a)
                        sig.add_event(s, m.n);
                } else if (v.type == json::Value::Bool) {
                    sig.set(s, v.b ? 1 : 0);
                } else if (SIGNALS[s].kind == SK_NAME) {
                    int i = value_index(s, v);
                    if (i >= 0)
                        sig.set(s, i);
                } else if (v.type == json::Value::Num) {
                    sig.set(s, v.n);
                }
            }
            if (fr["reset"].type == json::Value::Bool && fr["reset"].b)
                engine.reset(spec.rules, plan);
            engine.step(sig, fr["dt_ms"].type == json::Value::Num ? fr["dt_ms"].n : 1000.0 / 60);
            const json::Value &ex = fr["expect"];
            for (size_t r = 0; r < ex["weights"].a.size() && r < engine.rules(); r++)
                if (!near(engine.state(r).w, ex["weights"].a[r].n)) {
                    std::printf("FAIL %s frame %zu: rule %zu weight %f, expected %f\n", name.c_str(), n, r,
                                engine.state(r).w, ex["weights"].a[r].n);
                    failed++;
                }
            for (size_t i = 0; i < ex["options"].a.size() && i < plan.instances.size(); i++)
                for (size_t k = 0; k < ex["options"].a[i].a.size() && k < SLOTS; k++)
                    if (!near(engine.options(i)[k], ex["options"].a[i].a[k].n)) {
                        std::printf("FAIL %s frame %zu: instance %zu slot %zu is %f, expected %f\n", name.c_str(), n, i, k,
                                    engine.options(i)[k], ex["options"].a[i].a[k].n);
                        failed++;
                    }
            for (size_t i = 0; i < ex["off"].a.size() && i < plan.instances.size(); i++)
                if (engine.off(i) != (ex["off"].a[i].type == json::Value::Bool ? ex["off"].a[i].b : ex["off"].a[i].n != 0)) {
                    std::printf("FAIL %s frame %zu: instance %zu off is %d\n", name.c_str(), n, i, engine.off(i));
                    failed++;
                }
        }
    }
    std::printf("ran %d vector case(s) from %s\n", cases, path);
}

int main(int argc, char **argv)
{
    own_cases();
    if (argc > 1)
        vector_cases(argv[1]);
    if (failed) {
        std::printf("%d failure(s)\n", failed);
        return 1;
    }
    std::printf("rules ok\n");
    return 0;
}
