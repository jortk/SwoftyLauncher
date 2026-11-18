#include "toast_logic.hpp"

#include <cstdio>
#include <string>

namespace t = runtime::toast;

static int failed = 0;
static void expect(bool ok, const char *what)
{
    std::printf("%s: %s\n", ok ? "ok  " : "FAIL", what);
    if (!ok)
        failed++;
}

static t::Message msg(const char *id, int64_t created, int64_t position = 3)
{
    t::Message m;
    m.id = id;
    m.lap_ms = 88117;
    m.position = position;
    m.total = 41;
    m.created = created;
    return m;
}

int main()
{
    t::Message m;
    expect(t::parse(R"({"id":"a","board":{"content":"game/f1_2025","track":"bahrain","weather":"dry"},"lap_ms":88117,"position":3,"total":41,"improvement_ms":386,"created":1000})", m) ==
                   t::Status::ok &&
               m.id == "a" && m.track == "bahrain" && m.lap_ms == 88117 && m.position == 3 && m.total == 41 && m.improvement_ms == 386 &&
               !m.record,
           "a full message parses");
    expect(t::parse(R"({"id":"a","lap_ms":1,"position":1})", m) == t::Status::ok && m.record && m.total == 0 && m.improvement_ms == 0,
           "position 1 is a record, total and improvement are optional");
    expect(t::parse(R"({"id":"a","lap_ms":1,"position":5,"record":true})", m) == t::Status::ok && m.record, "an explicit record flag");
    expect(t::parse("[1]", m) == t::Status::not_object && t::parse("", m) == t::Status::not_object, "non-objects are refused");
    expect(t::parse(R"({"lap_ms":1,"position":1})", m) == t::Status::no_id, "no id is refused");
    expect(t::parse(R"({"id":"a","position":1})", m) == t::Status::no_lap, "no lap time is refused");
    expect(t::parse(R"({"id":"a","lap_ms":1})", m) == t::Status::no_position, "no position is refused");

    t::Message f = msg("f", 0);
    f.improvement_ms = 386;
    expect(t::title_of(f) == "NEW PERSONAL BEST" && t::detail_of(f) == "P3 of 41 - 1:28.117  -0.386", "lines for a personal best");
    f.record = true;
    f.total = 0;
    f.improvement_ms = 0;
    expect(t::title_of(f) == "NEW RECORD" && t::detail_of(f) == "P3 - 1:28.117", "record title, no total");
    expect(t::format_gain(386) == "-0.386" && t::format_gain(12005) == "-12.005" && t::format_time(59999) == "0:59.999" &&
               t::format_time(600001) == "10:00.001",
           "time formats");

    for (auto rec : {false, true}) {
        t::Message a = msg("ascii", 0, rec ? 1 : 12);
        a.record = rec;
        a.improvement_ms = 386;
        std::string all = t::title_of(a) + t::detail_of(a);
        bool pure = true;
        for (unsigned char c : all)
            pure = pure && c < 0x80;
        expect(pure, "the toast text is pure ASCII");
    }

    t::Machine mc;
    expect(!mc.active() && !mc.step(0, 100).visible, "idle at first");
    expect(mc.offer(msg("a", 100), 100), "queued");
    expect(!mc.offer(msg("a", 100), 100), "the same id shows once");
    auto s = mc.step(1000, 100);
    expect(s.visible && s.started && s.message.id == "a", "showing starts and asks for the toast once");
    s = mc.step(3000, 100);
    expect(s.visible && !s.started, "holds without posting again");
    s = mc.step(1000 + t::SHOW_MS, 100);
    expect(!s.visible && !mc.active(), "done");
    expect(!mc.offer(msg("a", 100), 100), "a done id is not shown again");

    expect(!mc.offer(msg("old", 100), 111), "older than 10 s is dropped");
    expect(mc.offer(msg("edge", 100), 110), "10 s late is still shown");
    mc.clear();

    t::Machine q;
    q.offer(msg("1", 0), 0);
    q.offer(msg("2", 0), 0);
    expect(q.step(0, 0).message.id == "1", "first first");
    expect(q.step(t::SHOW_MS, 0).message.id == "2", "second after the first is done");
    t::Machine late;
    late.offer(msg("1", 100), 100);
    late.offer(msg("2", 100), 100);
    late.step(0, 100);
    expect(!late.step(t::SHOW_MS, 111).visible, "a queued toast that turned stale is dropped");
    t::Machine cap;
    int accepted = 0;
    for (int i = 0; i < 10; i++)
        accepted += cap.offer(msg(std::to_string(i).c_str(), 0), 0);
    expect(accepted == int(t::MAX_QUEUED), "the queue is bounded");
    t::Machine na;
    expect(na.offer(msg("n", 0), 999999), "created 0 means an unknown age, shown");

    std::printf("%s\n", failed ? "FAILED" : "all passed");
    return failed ? 1 : 0;
}
