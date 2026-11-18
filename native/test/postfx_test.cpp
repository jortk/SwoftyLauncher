#include "postfx_logic.hpp"

#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <utility>
#include <vector>

using namespace postfx;

static int failed;

#define CHECK(c)                                                                                                       \
    do {                                                                                                               \
        if (!(c)) {                                                                                                    \
            std::printf("FAIL %s:%d: %s\n", __FILE__, __LINE__, #c);                                                   \
            failed++;                                                                                                  \
        }                                                                                                              \
    } while (0)

// The placement gate the way postfx.cpp runs it, over game frames that rotate through `buffers` back buffers. Each
// frame records and submits the picture's list (the back buffer becomes a render target; in menus the passes run at
// its HUD-less step while the buffer is fresh), then the HUD's list (the buffer becomes a render target again and
// the HUD reads it: the passes run there unless they ran on this buffer lately), and in menus a second list that
// reads it too. Frame generation calls Present1 `presents(f)` times in frame f, at the steps `at(f, k)`. With
// game_clock the frame closes as FrameClock says (the core now); without it every Present closes it (1.9.17).
struct Sim {
    unsigned buffers = 3, frames = 600;
    bool menus = false, game_clock = true, generated = true, flow_only = false;
    unsigned (*presents)(unsigned f) = nullptr;
    unsigned (*at)(unsigned f, unsigned k) = nullptr;

    std::vector<unsigned> run()
    {
        int back[8];
        unsigned placed[8], count = 0;
        bool fresh[8] = {};
        for (auto &p : placed)
            p = LONG_AGO;
        FrameClock clock;
        uint64_t now = 100000, started = 1000;
        std::vector<unsigned> runs(frames, 0);
        auto present = [&] {
            if (!game_clock || clock.present(now, generated, started))
                count++;
        };
        auto submit = [&](int *b) {
            if (game_clock && clock.submit(b, now, started))
                count++;
        };
        for (unsigned f = 0; f < frames; f++) {
            unsigned i = f % buffers, n = presents && !flow_only ? presents(f) : 0;
            int *b = &back[i];
            auto step = [&](unsigned s) {
                for (unsigned k = 0; k < n; k++)
                    if (at(f, k) == s)
                        present();
                now += 2;
            };
            step(0);
            fresh[i] = !placed_lately(count, placed[i]);
            if (menus && fresh[i]) {
                fresh[i] = false;
                placed[i] = count;
                runs[f]++;
            }
            step(1);
            submit(b);
            step(2);
            fresh[i] = !placed_lately(count, placed[i]);
            if (!placed_lately(count, placed[i])) {
                fresh[i] = false;
                placed[i] = count;
                runs[f]++;
            }
            step(3);
            submit(b);
            step(4);
            if (menus && !placed_lately(count, placed[i])) {
                placed[i] = count;
                runs[f]++;
            }
            step(5);
            submit(b);
            now += 4;
        }
        return runs;
    }
};

static unsigned once_per_run(const std::vector<unsigned> &runs, unsigned &skipped, unsigned &doubled)
{
    skipped = doubled = 0;
    unsigned off = 0;
    for (size_t f = 8; f < runs.size(); f++) {
        skipped += runs[f] == 0;
        doubled += runs[f] > 1;
        off += runs[f] != 1;
    }
    return off;
}

static unsigned rnd(unsigned f, unsigned k)
{
    unsigned x = f * 2654435761u + k * 40503u + 0x9e3779b9u;
    x ^= x >> 15;
    x *= 2246822519u;
    x ^= x >> 13;
    return x;
}

// Where one frame's passes run when the picture's list ends inside the HUD-less step, the way postfx.cpp drives
// BackPlace: the picture's list makes the back buffer a render target, the closing list records its step barrier
// before or after the picture's list is submitted (split step when it binds the back buffer), the closing list and
// the HUD's list are submitted in either order, then Present. The closing list draws the rest of the picture first
// when it binds the back buffer, so passes placed right after the picture's list are painted over. The HUD's list
// draws the HUD and ends with the back buffer out of render target; hud_seen is whether the core saw it submitted.
enum Ran : uint8_t { RAN_SPLIT, RAN_AFTER_PICTURE, RAN_AFTER_CLOSE, RAN_BEFORE_HUD, RAN_PRESENT };

struct SplitFrame {
    bool closing_recorded_first = false, bound = true, closing = true, direct = true, hud_first = false,
         reset = false, may_defer = true, hud_seen = true;
};

static std::vector<Ran> split_frame(BackPlace &p, unsigned now, const SplitFrame &s)
{
    std::vector<Ran> ran;
    p.drawn(now);
    bool in_closing = false;
    auto record_closing = [&] {
        if (s.closing && s.bound && s.direct && p.split_step(now)) {
            p.place(now);
            in_closing = true;
            ran.push_back(RAN_SPLIT);
        }
    };
    if (s.closing_recorded_first)
        record_closing();
    if (p.picture_submitted(now, true, s.may_defer) == END_BETWEEN)
        ran.push_back(RAN_AFTER_PICTURE);
    if (!s.closing_recorded_first)
        record_closing();
    bool submitted = s.closing && !s.reset;
    if (in_closing && s.reset) {
        p.unplace();
        ran.pop_back();
    }
    auto submit_hud = [&] {
        if (!s.hud_seen)
            return;
        if (p.target_left(now, true))
            ran.push_back(RAN_BEFORE_HUD);
        p.moved(false);
    };
    if (s.hud_first)
        submit_hud();
    if (submitted && p.step_closed(now, s.direct))
        ran.push_back(RAN_AFTER_CLOSE);
    if (!s.hud_first)
        submit_hud();
    if (p.presented(now))
        ran.push_back(RAN_PRESENT);
    return ran;
}

static bool shows(const std::vector<Ran> &ran, const SplitFrame &s)
{
    return ran.size() == 1 && !(ran[0] == RAN_AFTER_PICTURE && s.closing && s.bound);
}

static bool over_the_hud(const std::vector<Ran> &ran)
{
    for (Ran r : ran)
        if (r == RAN_PRESENT)
            return true;
    return false;
}

// Frames of one race through HudRead the way postfx.cpp drives it: reads(f) is whether the HUD's list of frame f
// reads the finished picture, hud_first(f) whether that list is recorded before the list with the HUD-less step,
// read_late(f) whether it is recorded only after the picture's list is submitted, open(f) whether the picture's list
// ends with the HUD-less step still open, and hud_seen(f) whether the core sees the HUD's list take the back buffer
// out of render target. The picture's submission places the passes or leaves them waiting (picture_wait); waiting
// passes run before the HUD's list, or at Present without it. held is the 1.9.64 gate: the submission held off
// while the read held. Per frame, how often the passes ran and where they ran last.
enum ReadAt : uint8_t { AT_NONE, AT_STEP, AT_READ, AT_PICTURE, AT_BEFORE_HUD, AT_PRESENT };

struct ReadSim {
    bool (*reads)(unsigned f) = nullptr;
    bool (*hud_first)(unsigned f) = nullptr;
    bool (*read_late)(unsigned f) = nullptr;
    bool (*open)(unsigned f) = nullptr;
    bool (*hud_seen)(unsigned f) = nullptr;
    bool game_frames = false, held = false;
    unsigned frames = 400;
    std::vector<unsigned> runs;
    std::vector<bool> at_read;
    std::vector<ReadAt> at;

    void run()
    {
        HudRead latch;
        runs.assign(frames, 0);
        at_read.assign(frames, false);
        at.assign(frames, AT_NONE);
        for (unsigned f = 0; f < frames; f++) {
            unsigned now = 100 + f;
            BackPlace p;
            auto ran = [&](ReadAt where) {
                runs[f]++;
                at[f] = where;
                at_read[f] = at_read[f] || where == AT_READ;
            };
            p.drawn(now);
            auto read = [&] {
                if (!reads(f) || placed_lately(now, p.placed_at))
                    return;
                latch.read(now);
                p.place(now);
                ran(AT_READ);
            };
            auto step = [&] {
                if (latch.holds(now) || !p.fresh)
                    return;
                p.place(now);
                ran(AT_STEP);
            };
            bool late = read_late && read_late(f);
            if (!late && hud_first && hud_first(f))
                read();
            step();
            if (!late && !(hud_first && hud_first(f)))
                read();
            if (held ? !latch.holds(now) : !latch.read_in(now)) {
                PictureWait w = picture_wait(open && open(f), !held && latch.latch(now), game_frames);
                if (p.picture_submitted(now, w.open, w.may_defer) == END_BETWEEN)
                    ran(AT_PICTURE);
            }
            if (late)
                read();
            if (!hud_seen || hud_seen(f)) {
                if (p.target_left(now, true))
                    ran(AT_BEFORE_HUD);
                p.moved(false);
            }
            if (!game_frames && p.presented(now))
                ran(AT_PRESENT);
        }
    }
};

int main()
{
    {
        // The split step wins the race: the closing list records its step barrier before the picture's list is
        // submitted. Before the fix and after, the passes run in the closing list.
        BackPlace p;
        SplitFrame s;
        s.closing_recorded_first = true;
        auto ran = split_frame(p, 10, s);
        CHECK(ran == std::vector<Ran>{RAN_SPLIT} && shows(ran, s));
        s.may_defer = false;
        ran = split_frame(p, 13, s);
        CHECK(ran == std::vector<Ran>{RAN_SPLIT} && shows(ran, s));
    }
    {
        // The race the split step lost (1.9.41): the picture's list is submitted first. Placing the passes right
        // after it gets them painted over by the closing list; waiting for the closing list runs them in it.
        BackPlace p;
        SplitFrame s;
        s.may_defer = false;
        auto ran = split_frame(p, 10, s);
        CHECK(ran == std::vector<Ran>{RAN_AFTER_PICTURE} && !shows(ran, s));
        s.may_defer = true;
        ran = split_frame(p, 13, s);
        CHECK(ran == std::vector<Ran>{RAN_SPLIT} && shows(ran, s));
        CHECK(!p.waits(13) && !p.presented(13));
    }
    {
        // A closing list that does not bind the back buffer leaves it alone: the passes run right after it, in
        // either order of record and submit.
        for (bool first : {false, true}) {
            BackPlace p;
            SplitFrame s;
            s.bound = false;
            s.closing_recorded_first = first;
            auto ran = split_frame(p, 20, s);
            CHECK(ran == std::vector<Ran>{RAN_AFTER_CLOSE} && shows(ran, s));
        }
    }
    {
        // No list closes the step, or only a compute list does: the passes run right before the HUD's list, under
        // the HUD, where 1.9.52 ran them at Present, over it.
        BackPlace p;
        SplitFrame s;
        s.closing = false;
        auto ran = split_frame(p, 30, s);
        CHECK(ran == std::vector<Ran>{RAN_BEFORE_HUD} && shows(ran, s) && !over_the_hud(ran));
        s = SplitFrame{};
        s.direct = false;
        ran = split_frame(p, 33, s);
        CHECK(ran == std::vector<Ran>{RAN_BEFORE_HUD} && shows(ran, s) && !over_the_hud(ran));
    }
    {
        // The HUD's list is submitted before the closing list: the passes run right before the HUD's list, and the
        // closing list finds them placed.
        BackPlace p;
        SplitFrame s;
        s.bound = false;
        s.hud_first = true;
        auto ran = split_frame(p, 40, s);
        CHECK(ran == std::vector<Ran>{RAN_BEFORE_HUD} && shows(ran, s) && !over_the_hud(ran));
    }
    {
        // The closing list the passes were recorded into is reset unsubmitted: they run right before the HUD's list.
        BackPlace p;
        SplitFrame s;
        s.reset = true;
        auto ran = split_frame(p, 50, s);
        CHECK(ran == std::vector<Ran>{RAN_BEFORE_HUD} && shows(ran, s) && !over_the_hud(ran));
    }
    {
        // A frame whose HUD list the core never saw reaches Present with the passes waiting. They run there, over
        // the HUD for that one frame, rather than not at all: skipping them changes the whole picture for a frame,
        // running them changes only the HUD.
        BackPlace p;
        SplitFrame s;
        s.closing = false;
        s.hud_seen = false;
        auto ran = split_frame(p, 53, s);
        CHECK(ran == std::vector<Ran>{RAN_PRESENT} && over_the_hud(ran));
    }
    {
        // The list that takes the back buffer out of render target is not asked twice, not after the passes ran in
        // the closing list, not on a compute queue, and not once another list already took it out.
        BackPlace p;
        p.drawn(56);
        CHECK(p.picture_submitted(56, true, true) == END_DEFER);
        CHECK(!p.target_left(56, false) && p.waits(56));
        CHECK(p.target_left(56, true) && !p.waits(56) && !p.target_left(56, true) && !p.presented(56));
        p.drawn(59);
        CHECK(p.picture_submitted(59, true, true) == END_DEFER);
        p.moved(false);
        CHECK(!p.target_left(59, true) && p.presented(59));
        p.drawn(62);
        p.place(62);
        CHECK(p.picture_submitted(62, true, true) == END_NONE && !p.target_left(62, true));
        BackPlace closed;
        closed.drawn(65);
        CHECK(closed.picture_submitted(65, false, true) == END_BETWEEN && !closed.target_left(65, true));
    }
    {
        // After a pause the passes stay on. 1.9.52 held the HUD's read for 30 frames, so the picture went 30 frames
        // without them after every pause, flashback and menu over the race; 1.9.64 went one frame without them (the
        // latch), which now runs them before the HUD's list.
        ReadSim sim;
        sim.reads = [](unsigned f) { return f >= 100 && f < 200; };
        for (int order = 0; order < 3; order++) {
            sim.hud_first = order == 0   ? nullptr
                            : order == 1 ? +[](unsigned) { return true; }
                                         : +[](unsigned f) { return rnd(f, 7) % 2 == 0; };
            for (bool held : {true, false}) {
                sim.held = held;
                sim.run();
                unsigned without = 0, doubled = 0, left_the_read = 0;
                bool taken = false;
                for (unsigned f = 0; f < sim.frames; f++) {
                    without += sim.runs[f] == 0;
                    doubled += sim.runs[f] > 1;
                    left_the_read += f < 200 && taken && !sim.at_read[f];
                    taken |= sim.at_read[f];
                }
                // the list with the HUD-less step recorded first every frame never leaves the passes to the read
                CHECK(taken == (order != 0));
                CHECK(without == (taken && held ? 1u : 0u));
                CHECK(sim.runs[200] == (taken && held ? 0u : 1u) && sim.runs[201] == 1);
                CHECK(!taken || held || sim.at[200] == AT_BEFORE_HUD);
                CHECK(doubled == 0);
                // once the read runs the passes it stays the one place they run while the HUD reads every frame
                CHECK(left_the_read == 0);
            }
        }
    }
    {
        // A screen whose HUD reads the picture every other frame, or every tenth: no frame runs the passes twice or
        // goes without them (1.9.64 left out every frame after a read without one).
        for (unsigned every : {2u, 3u, 10u, 31u}) {
            static unsigned period;
            period = every;
            ReadSim sim;
            sim.reads = [](unsigned f) { return f % period == 0; };
            for (bool first : {false, true}) {
                sim.hud_first = first ? +[](unsigned) { return true; } : nullptr;
                sim.run();
                unsigned without = 0, doubled = 0;
                for (unsigned f = 1; f < sim.frames; f++) {
                    without += sim.runs[f] == 0;
                    doubled += sim.runs[f] > 1;
                }
                CHECK(doubled == 0 && without == 0);
            }
        }
        HudRead h;
        CHECK(!h.holds(0) && !h.holds(5000) && !h.latch(0) && !h.read_in(0));
        h.read(0u - 1);
        CHECK(h.holds(0u - 1) && h.holds(0) && !h.holds(1));
        CHECK(h.read_in(0u - 1) && !h.latch(0u - 1) && !h.read_in(0) && h.latch(0) && !h.latch(1));
        CHECK(!picture_wait(false, false, false).open && !picture_wait(false, false, false).may_defer);
        CHECK(picture_wait(true, false, false).open && picture_wait(true, false, false).may_defer);
        CHECK(picture_wait(true, false, true).open && !picture_wait(true, false, true).may_defer);
        for (bool step_open : {false, true})
            for (bool game_frames : {false, true})
                CHECK(picture_wait(step_open, true, game_frames).open &&
                      picture_wait(step_open, true, game_frames).may_defer);
    }
    {
        // A race: the HUD reads the picture every frame but skips one now and then (12 times in two minutes of a
        // real one). 1.9.64 left every such frame without the passes; now each runs them once, before the HUD's
        // list, or at Present when the core does not see that list. Only while the game's own frames close the
        // passes' frames (no Present to fall back on) does a frame whose HUD's list the core misses still go
        // without them. Every order of recording, with the HUD's list recorded after the picture's submission too:
        // never twice, and every other frame runs them where 1.9.64 did.
        static auto skips = [](unsigned f) { return f > 2 && rnd(f, 3) % 150 == 0; };
        ReadSim sim;
        sim.frames = 3600;
        sim.reads = [](unsigned f) { return !skips(f); };
        unsigned skipped = 0;
        for (unsigned f = 3; f < sim.frames; f++)
            skipped += skips(f) && !skips(f - 1);
        CHECK(skipped >= 12);
        for (unsigned bits = 0; bits < 32; bits++) {
            sim.hud_first = bits & 1 ? +[](unsigned) { return true; } : +[](unsigned f) { return rnd(f, 5) % 2 == 0; };
            sim.read_late = bits & 2 ? +[](unsigned f) { return rnd(f, 11) % 3 == 0; } : nullptr;
            sim.open = bits & 4 ? +[](unsigned f) { return rnd(f, 13) % 2 == 0; } : nullptr;
            sim.hud_seen = bits & 8 ? +[](unsigned) { return false; } : nullptr;
            sim.game_frames = bits & 16;
            sim.held = true;
            sim.run();
            std::vector<unsigned> held_runs = sim.runs;
            std::vector<ReadAt> held_at = sim.at;
            sim.held = false;
            sim.run();
            unsigned held_without = 0, without = 0, doubled = 0, latched = 0, moved = 0, wrong_place = 0;
            bool lost = sim.hud_seen && sim.game_frames;
            ReadAt fallback = sim.hud_seen ? AT_PRESENT : AT_BEFORE_HUD;
            for (unsigned f = 3; f < sim.frames; f++) {
                held_without += held_runs[f] == 0;
                without += sim.runs[f] == 0;
                doubled += sim.runs[f] > 1 || held_runs[f] > 1;
                bool latch = skips(f) && !skips(f - 1);
                latched += latch && held_runs[f] == 0 && sim.runs[f] == 1;
                wrong_place += latch && sim.runs[f] == 1 && sim.at[f] != fallback;
                moved += !latch && (sim.runs[f] != held_runs[f] || sim.at[f] != held_at[f]);
            }
            CHECK(doubled == 0 && moved == 0 && held_without == skipped);
            CHECK(lost ? without == skipped && latched == 0 : without == 0 && latched == skipped && wrong_place == 0);
        }
    }
    {
        // A step that stayed open past its frame (no Present closed it) no longer waits, and a new use of the back
        // buffer starts closed.
        BackPlace p;
        p.drawn(60);
        CHECK(p.picture_submitted(60, true, true) == END_DEFER && p.waits(60) && p.waits(61) && !p.waits(62));
        CHECK(!p.step_closed(62, true) && !p.presented(62));
        p.drawn(63);
        CHECK(p.fresh && !p.open && !p.waits(63));
        CHECK(p.picture_submitted(63, false, true) == END_BETWEEN && !p.fresh && p.placed_at == 63);
        CHECK(p.picture_submitted(63, true, true) == END_NONE);
    }
    {
        // Every order of record and submit, with or without a closing list, over frames that rotate through
        // three back buffers: once a frame, and shown whenever the closing list repaints the picture.
        BackPlace backs[3];
        unsigned off = 0, over = 0, frames = 0;
        for (unsigned f = 8; f < 8 + 3 * 64; f++) {
            unsigned bits = (f / 3) % 64;
            SplitFrame s;
            s.closing_recorded_first = bits & 1;
            s.bound = bits & 2;
            s.closing = !(bits & 4);
            s.direct = !(bits & 8);
            s.hud_first = bits & 16;
            s.reset = bits & 32;
            auto ran = split_frame(backs[f % 3], f, s);
            off += !shows(ran, s);
            over += over_the_hud(ran);
            frames++;
        }
        // never at Present, over the HUD, while the core sees the HUD's list
        CHECK(frames == 192 && off == 0 && over == 0);
    }

    {
        // Why a frame went without the passes: the kill file, then the shader toggle key, then what stopped the
        // place that tried, then the HUD's read holding the other places off, then whether there was a picture.
        IdleFacts f;
        CHECK(idle_reason(f) == IDLE_NO_PICTURE);
        f.picture = true;
        CHECK(idle_reason(f) == IDLE_UNPLACED);
        f.hud_read = true;
        CHECK(idle_reason(f) == IDLE_HUD_READ);
        f.tried = IDLE_BUSY;
        CHECK(idle_reason(f) == IDLE_BUSY);
        f.tried = IDLE_VIEW;
        CHECK(idle_reason(f) == IDLE_VIEW);
        f.key_off = true;
        CHECK(idle_reason(f) == IDLE_KEY);
        f.killed = true;
        CHECK(idle_reason(f) == IDLE_KILL);
        CHECK(idle_of_passes(1u << IDLE_PASS_OFF) == IDLE_PASS_OFF && idle_of_passes(1u << IDLE_VIEW) == IDLE_VIEW);
        CHECK(idle_of_passes(1u << IDLE_ADDON_KEY) == IDLE_ADDON_KEY && idle_of_passes(1u << IDLE_RULE) == IDLE_RULE);
        CHECK(idle_of_passes(1u << IDLE_OPTION) == IDLE_OPTION && idle_of_passes(0) == IDLE_PASSES);
        CHECK(idle_of_passes((1u << IDLE_PASS_OFF) | (1u << IDLE_VIEW)) == IDLE_PASSES);
        for (unsigned i = IDLE_NONE + 1; i < IDLE_COUNT; i++)
            CHECK(idle_text(Idle(i))[0] != 0);
        CHECK(idle_text(IDLE_NONE)[0] == 0);
    }
    {
        // One line when the passes stop and one when they run again, with the frames between and the reason they
        // stopped for; nothing before their first run, which has its own line, and nothing while nothing changes.
        OnOff o;
        CHECK(o.frame(false, 1, IDLE_NO_PICTURE) == OnOff::SAME && o.frame(false, 2, IDLE_NO_PICTURE) == OnOff::SAME);
        CHECK(o.frame(true, 3, IDLE_NONE) == OnOff::SAME && o.frame(true, 4, IDLE_NONE) == OnOff::SAME);
        CHECK(o.frame(false, 5, IDLE_HUD_READ) == OnOff::STOPPED && o.off_at == 5 && o.why == IDLE_HUD_READ);
        CHECK(o.frame(false, 6, IDLE_KEY) == OnOff::SAME && o.why == IDLE_HUD_READ);
        CHECK(o.frame(true, 7, IDLE_NONE) == OnOff::AGAIN && 7 - o.off_at == 2);
        CHECK(o.frame(true, 8, IDLE_NONE) == OnOff::SAME);
        CHECK(o.frame(false, 9, IDLE_KEY) == OnOff::STOPPED && o.why == IDLE_KEY);
    }
    {
        // The count line keeps coming for the whole session: every minute for the first half hour, then every
        // five minutes, where 1.9.52 stopped after 30 lines.
        CHECK(stat_window_ms(0) == 60000 && stat_window_ms(29) == 60000);
        CHECK(stat_window_ms(30) == 300000 && stat_window_ms(5000) == 300000);
        unsigned lines = 0;
        for (uint64_t at = 0; at < 8ull * 3600 * 1000; at += stat_window_ms(lines))
            lines++;
        CHECK(lines == 30 + (8 * 60 - 30) / 5);
    }

    When any;
    CHECK(!ineligible(any, UNKNOWN, 0, false));
    CHECK(!ineligible(any, CHASE_FAR, PAUSED | REPLAY, true));

    When driving{view_bit("cockpit") | view_bit("tcam") | view_bit("tcam_offset"),
                 flag_bit("paused") | flag_bit("replay") | flag_bit("photo") | flag_bit("menu") | flag_bit("cutscene")};
    CHECK(!ineligible(driving, COCKPIT, 0, true));
    CHECK(!ineligible(driving, TCAM_OFFSET, SPECTATING, true));
    CHECK(ineligible(driving, COCKPIT, 0, false));
    CHECK(ineligible(driving, CHASE_NEAR, 0, true));
    CHECK(ineligible(driving, COCKPIT, PAUSED, true));
    CHECK(ineligible(driving, TCAM, REPLAY | MENU, true));
    CHECK(ineligible(driving, UNKNOWN, 0, true));

    When no_pause{0, flag_bit("paused")};
    CHECK(!ineligible(no_pause, UNKNOWN, PAUSED, false));
    CHECK(ineligible(no_pause, OTHER, PAUSED, true));

    CHECK(view_bit("nope") == 0 && flag_bit("nope") == 0 && view_bit("unknown") == 0);
    CHECK(flag_text(0) == "-" && flag_text(PAUSED | MENU) == "paused+menu");
    CHECK(view_text(TCAM) == "tcam" && view_text(99) == "?");

    CHECK(key_code("B") == 'B' && key_code("b") == 'B' && key_code("7") == '7');
    CHECK(key_code("F1") == 0x70 && key_code("F12") == 0x7B && key_code("F24") == 0x87);
    CHECK(key_code("F25") == 0 && key_code("F0") == 0 && key_code("Fx") == 0);
    CHECK(key_code("Page Up") == 0x21 && key_code("Insert") == 0x2D && key_code("None") == 0 && key_code("") == 0);

    CHECK(format_of("rgba8") == 28 && format_of("rgba16f") == 10 && format_of("r11g11b10f") == 26);
    CHECK(format_of("rgb10a2") == 24 && format_of("r32f") == 41 && format_of("bgra8") == 0);

    Size half = texture_size(0.5, 0, 0, 1908, 1028);
    CHECK(half.w == 954 && half.h == 514);
    Size third = texture_size(1.0 / 3, 0, 0, 1921, 1081);
    CHECK(third.w == 640 && third.h == 360);
    Size tiny = texture_size(0.0001, 0, 0, 1920, 1080);
    CHECK(tiny.w == 1 && tiny.h == 1);
    Size fixed = texture_size(0.5, 256, 128, 1920, 1080);
    CHECK(fixed.w == 256 && fixed.h == 128);

    CHECK(mip_levels(4, 4, 3) == 3 && mip_levels(4, 4, 9) == 3 && mip_levels(4, 4, 0) == 1 && mip_levels(4, 4, 1) == 1);
    CHECK(mip_levels(1920, 1080, 12) == 11 && mip_levels(2048, 1, 99) == 12 && mip_levels(1 << 20, 1, 99) == 12);
    CHECK(mip_levels(1, 1, 5) == 1 && mip_levels(5, 3, 9) == 3 && mip_levels(64, 32, 7) == 7 && mip_levels(64, 32, 8) == 7);
    Size l1 = mip_size(5, 3, 1), l2 = mip_size(5, 3, 2), l9 = mip_size(5, 3, 9), l0 = mip_size(5, 3, 0);
    CHECK(l0.w == 5 && l0.h == 3 && l1.w == 2 && l1.h == 1 && l2.w == 1 && l2.h == 1 && l9.w == 1 && l9.h == 1);
    Size big = mip_size(1920, 1080, 5);
    CHECK(big.w == 60 && big.h == 33);

    float v = -1;
    CHECK(option_value("true", v) && v == 1);
    CHECK(option_value("false", v) && v == 0);
    CHECK(option_value("35.000000", v) && v == 35.0f);
    CHECK(option_value("-2.5", v) && v == -2.5f);
    v = 7;
    CHECK(!option_value("B", v) && v == 7);
    CHECK(!option_value("", v) && !option_value("3x", v));

    CHECK(groups(1920, 8) == 240 && groups(1921, 8) == 241 && groups(5, 16) == 1 && groups(5, 0) == 0);

    struct Res {
        int refs = 1;
        void AddRef() { refs++; }
        void Release() { refs--; }
    };
    Res depth, rebuilt;
    Res *back_use = nullptr, *slot = nullptr;
    hold(back_use, &depth);
    hold(back_use, &depth);
    CHECK(back_use == &depth && depth.refs == 2);
    depth.Release();
    CHECK(depth.refs == 1);
    hold(slot, back_use);
    CHECK(depth.refs == 2);
    hold(back_use, nullptr);
    CHECK(back_use == nullptr && depth.refs == 1);
    hold(slot, &rebuilt);
    CHECK(slot == &rebuilt && depth.refs == 0 && rebuilt.refs == 2);
    hold(slot, nullptr);
    CHECK(rebuilt.refs == 1);

    Written<Res, int> view;
    CHECK(rewrite(view, &depth, 46));
    CHECK(!rewrite(view, &depth, 46));
    CHECK(rewrite(view, &depth, 41) && view.f == 41);
    CHECK(rewrite(view, &rebuilt, 41) && view.r == &rebuilt);
    CHECK(rewrite(view, static_cast<Res *>(nullptr), 41) && !rewrite(view, static_cast<Res *>(nullptr), 41));
    view.set = false;
    CHECK(rewrite(view, static_cast<Res *>(nullptr), 41));

    struct List {};
    List la, lb, many[3];
    SideMarks<List, Res, int, 8, 8> side;
    CHECK(!side.find(&la, false));
    SideMark<List, Res, int> *a = side.find(&la, true);
    CHECK(a && side.find(&la, false) == a && side.find(&lb, true) != a && !side.full());
    bool marked = a->marked || a->depth;
    note(*a, marked, false, true, &rebuilt, 0);
    CHECK(!a->ui && !marked);
    note(*a, marked, true, false, &depth, 16);
    CHECK(a->depth == &depth && a->depth_state == 16 && marked);
    note(*a, marked, false, true, &rebuilt, 0);
    CHECK(a->ui);
    SideMark<List, Res, int> *b = side.find(&lb, false);
    b->marked = true;
    marked = b->marked || b->depth;
    note(*b, marked, false, true, &rebuilt, 0);
    CHECK(b->ui && !b->depth);

    CHECK(!placed_lately(0, LONG_AGO) && !placed_lately(5000, LONG_AGO));
    CHECK(placed_lately(100, 100) && placed_lately(101, 100) && !placed_lately(102, 100) && !placed_lately(103, 100));
    CHECK(placed_lately(0, 0u - 1) && !placed_lately(1, 0u - 1));

    CHECK(!presents_gone(500000, 0, 0));
    CHECK(!presents_gone(10999, 0, 10000) && presents_gone(11000, 0, 10000));
    CHECK(!presents_gone(20500, 20000, 10000) && presents_gone(21000, 20000, 10000));
    CHECK(!presents_gone(9000, 0, 10000) && !presents_gone(5000, 20000, 10000));

    CHECK(presents_own_frames("libxess_fg.dll") && presents_own_frames("LibXeSS_FG.DLL"));
    CHECK(presents_own_frames("libxess_fg.dll (calls Present)") && presents_own_frames("C:\\game\\libxess_fg.dll"));
    CHECK(!presents_own_frames("") && !presents_own_frames("amd_fidelityfx_dx12.dll") &&
          !presents_own_frames("sl.dlss_g.dll") && !presents_own_frames("nvngx_dlssg.dll"));
    CHECK(!presents_own_frames("libxess_fg.dll.bak") && !presents_own_frames("a non-direct present queue") &&
          !presents_own_frames("libxess.dll"));

    {
        int a, b, c;
        FrameClock plain;
        CHECK(plain.present(20000, false, 10000) && plain.present(20016, false, 10000));
        CHECK(!plain.submit(&a, 20020, 10000) && !plain.submit(&b, 20030, 10000) && !plain.flow(20030, 10000));
        CHECK(plain.present(20033, false, 10000) && !plain.flow(21032, 10000) && plain.flow(21033, 10000));

        FrameClock xess;
        CHECK(!xess.present(20000, true, 10000) && !xess.present(20008, true, 10000));
        CHECK(xess.submit(&a, 20010, 10000) && !xess.submit(&a, 20012, 10000) && !xess.submit(nullptr, 20012, 10000));
        CHECK(!xess.present(20016, false, 10000) && !xess.present(20017, true, 10000));
        CHECK(xess.submit(&b, 20020, 10000) && xess.submit(&c, 20040, 10000) && xess.submit(&a, 20060, 10000));
        CHECK(!xess.flow(20060, 10000));
        CHECK(xess.submit(&b, 21016, 10000) && xess.present(21017, false, 10000));
        CHECK(!xess.submit(&c, 21020, 10000) && xess.present(21033, false, 10000));

        FrameClock none;
        CHECK(!none.submit(&a, 10500, 10000) && !none.flow(10999, 10000) && none.flow(11000, 10000));
        CHECK(none.submit(&b, 11000, 10000) && none.submit(&a, 11016, 10000) && !none.submit(&a, 11020, 10000));
        CHECK(none.present(11030, false, 10000) && !none.flow(11040, 10000));
        CHECK(!none.submit(&b, 11050, 10000));
        CHECK(none.submit(&c, 12100, 10000));
    }

    {
        unsigned skipped = 0, doubled = 0;
        Sim plain;
        plain.generated = false;
        plain.presents = [](unsigned) { return 1u; };
        plain.at = [](unsigned, unsigned) { return 5u; };
        CHECK(once_per_run(plain.run(), skipped, doubled) == 0);
        plain.game_clock = false;
        CHECK(once_per_run(plain.run(), skipped, doubled) == 0);
        plain.game_clock = true;
        plain.menus = true;
        CHECK(once_per_run(plain.run(), skipped, doubled) == 0);

        Sim fg_on;
        fg_on.presents = [](unsigned f) { return 1u + rnd(f, 99) % 3; };
        fg_on.at = [](unsigned f, unsigned k) { return rnd(f, k) % 6; };
        fg_on.game_clock = false;
        unsigned off = once_per_run(fg_on.run(), skipped, doubled);
        std::printf("xess fg on, every Present1 closing the frame (1.9.17): %u of %u frames off (%u skipped, %u doubled)\n",
                    off, fg_on.frames - 8, skipped, doubled);
        fg_on.menus = true;
        off = once_per_run(fg_on.run(), skipped, doubled);
        std::printf("xess fg on in menus (1.9.17): %u of %u frames off (%u skipped, %u doubled)\n", off,
                    fg_on.frames - 8, skipped, doubled);
        CHECK(off > 0 && doubled > 0);
        for (unsigned buffers : {3u, 4u})
            for (bool menus : {false, true}) {
                fg_on.game_clock = true;
                fg_on.buffers = buffers;
                fg_on.menus = menus;
                CHECK(once_per_run(fg_on.run(), skipped, doubled) == 0);
            }

        Sim fg_off;
        fg_off.presents = [](unsigned f) { return rnd(f, 77) % 3; };
        fg_off.at = [](unsigned f, unsigned k) { return rnd(f, k + 5) % 6; };
        fg_off.game_clock = false;
        off = once_per_run(fg_off.run(), skipped, doubled);
        std::printf("xess swapchain, frame generation off, Present1 paced on its own thread (1.9.17): %u of %u frames "
                    "off (%u skipped, %u doubled)\n",
                    off, fg_off.frames - 8, skipped, doubled);
        CHECK(off > 0 && skipped > 0);
        fg_off.game_clock = true;
        for (bool menus : {false, true}) {
            fg_off.menus = menus;
            CHECK(once_per_run(fg_off.run(), skipped, doubled) == 0);
        }

        Sim mixed = fg_on;
        mixed.presents = [](unsigned f) { return 2u + rnd(f, 55) % 2; };
        mixed.at = [](unsigned f, unsigned k) { return k == 2 ? 3u : rnd(f, k + 9) % 6; };
        mixed.game_clock = true;
        mixed.menus = false;
        mixed.buffers = 3;
        CHECK(once_per_run(mixed.run(), skipped, doubled) == 0);

        Sim gone;
        gone.flow_only = true;
        gone.game_clock = true;
        gone.generated = false;
        for (bool menus : {false, true}) {
            gone.menus = menus;
            CHECK(once_per_run(gone.run(), skipped, doubled) == 0);
        }
    }

    SideMarks<List, Res, int, 2, 2> cramped;
    CHECK(cramped.find(&many[0], true) && cramped.find(&many[1], true));
    CHECK(!cramped.find(&many[2], false) && !cramped.full());
    CHECK(!cramped.find(&many[2], true) && cramped.full());
    CHECK(cramped.find(&many[0], false) && cramped.find(&many[1], false));


    static_assert(OPTIONS_AT == 8 && PARAMS_AT == 36 && CONSTANT_COUNT == 56 && PARAMS_AT + 16 <= CONSTANT_COUNT);
    float packed[SLOTS];
    pack_slots({1, 2, 3}, packed);
    CHECK(packed[0] == 1 && packed[2] == 3 && packed[3] == 0 && packed[31] == 0);
    std::vector<float> forty;
    for (int i = 0; i < 40; i++)
        forty.push_back(float(i));
    pack_slots(forty, packed);
    CHECK(packed[15] == 15 && packed[16] == 16 && packed[31] == 31);

    const char *fx = R"({"version":1,"effects":[
      {"id":"ambient_light","name":"Ambient Light","addon":"","slots":9,
       "textures":[{"name":"half","scale":0.5,"format":"rgba16f"},
                   {"name":"adapt","width":1,"height":1,"format":"rgba16f","persistent":true},
                   {"name":"dirt","format":"rgba8","file":"shaders/tex/ambient_light-dirt.tex"}],
       "passes":[{"name":"bright","dxil":"shaders/fx/ambient_light-00.dxil","inputs":[],"output":"half","threads":[8,8],"budget_ms":1.5},
                 {"name":"apply","dxil":"shaders/fx/ambient_light-01.dxil","inputs":["half","adapt","dirt","original","previous"],"output":""}]},
      {"id":"grain","name":"Grain","slots":2,"textures":[],
       "passes":[{"name":"grain","dxil":"shaders/fx/grain-00.dxil","inputs":[],"output":""}]},
      {"id":"bad_input","slots":1,"passes":[{"name":"a","dxil":"x.dxil","inputs":["nothing"]}]},
      {"id":"self","slots":1,"textures":[{"name":"t"}],"passes":[{"name":"a","dxil":"y.dxil","inputs":["t"],"output":"t"}]},
      {"id":"writes_file","slots":1,"textures":[{"name":"t","file":"f.tex"}],"passes":[{"name":"a","dxil":"z.dxil","output":"t"}]},
      {"id":"huge","slots":33,"passes":[{"name":"a","dxil":"h.dxil"}]},
      {"name":"no id","passes":[{"name":"a","dxil":"q.dxil"}]},
      {"id":"no_passes","slots":1,"passes":[]}]})";
    std::vector<EffectDecl> effects = parse_effects(fx);
    CHECK(effects.size() == 6);
    CHECK(effects[0].id == "ambient_light" && effects[0].slots == 9 && effects[0].textures.size() == 3);
    CHECK(effects[0].textures[0].scale == 0.5 && effects[0].textures[0].format == 10);
    CHECK(effects[0].textures[1].persistent && effects[0].textures[1].width == 1);
    CHECK(effects[0].textures[2].format == 28 && effects[0].textures[2].file == "shaders/tex/ambient_light-dirt.tex");
    std::vector<EffectDecl> chained = parse_effects(
        R"({"effects":[{"id":"m","slots":1,"textures":[{"name":"a","width":64,"height":32,"mips":7},{"name":"b","mips":99},{"name":"c","mips":0},{"name":"d"}],
        "passes":[{"name":"p","dxil":"p.dxil","output":"a"}]}]})");
    CHECK(chained.size() == 1 && chained[0].textures.size() == 4);
    CHECK(chained[0].textures[0].mips == 7 && chained[0].textures[1].mips == MAX_MIPS && chained[0].textures[2].mips == 1 &&
          chained[0].textures[3].mips == 1);
    CHECK(effects[0].textures[0].mips == 1 && !(chained[0].textures[0] == effects[0].textures[0]));
    CHECK(effects[0].passes[1].inputs.size() == 5 && effects[0].passes[1].threads[0] == 8);
    CHECK(parse_effects("not json").empty() && parse_effects("{}").empty());

    ActiveSpec act;
    CHECK(!parse_active("[", act) && !parse_active("[1]", act));
    const char *active_json = R"({"version":1,"serial":42,"name":"Realistic","source":"addon:x/realistic",
      "forced_by":"","toggle_key":"F9","instances":[
      {"effect":"grain","label":"","params":[0.5,1]},
      {"effect":"ambient_light","label":"wide","params":[0.96,0,1,10,0,4,0,0,0]},
      {"effect":"nope","label":"","params":[]},
      {"effect":"ambient_light","label":"","params":[1,1,1,1,1,1,1,1,1]},
      {"effect":"huge","params":[]},
      {"effect":"self","params":[1]}]})";
    CHECK(parse_active(active_json, act));
    CHECK(act.serial == 42 && act.name == "Realistic" && act.toggle_key == "F9" && act.instances.size() == 6);
    CHECK(key_code(act.toggle_key) == 0x78);
    CHECK(act.instances[1].label == "wide" && act.instances[1].params.size() == 9 && act.instances[1].params[5] == 4.0f);

    Plan plan = make_plan(act, effects);
    CHECK(plan.instances.size() == 3);
    CHECK(plan.dropped.size() == 3);
    CHECK(plan.dropped[0] == "nope: no such effect");
    CHECK(plan.instances[0].effect == "grain" && plan.instances[1].effect == "ambient_light" &&
          plan.instances[1].label == "wide" && plan.instances[2].label.empty());
    CHECK(plan.instances[1].options[0] == 0.96f && plan.instances[1].options[5] == 4.0f && plan.instances[1].options[20] == 0);
    CHECK(plan.textures.size() == 6);
    CHECK(plan.textures[0].name == "1/half" && plan.textures[1].name == "1/adapt" && plan.textures[2].name == "1/dirt");
    CHECK(plan.textures[3].name == "2/half");
    CHECK(plan.passes.size() == 5);
    CHECK(plan.passes[0].instance == 0 && plan.passes[0].n_inputs == 0 && plan.passes[0].output == IN_NONE);
    CHECK(plan.passes[1].instance == 1 && plan.passes[1].output == 0);
    const PlanPass &apply = plan.passes[2];
    CHECK(apply.n_inputs == 5 && apply.inputs[0] == 0 && apply.inputs[1] == 1 && apply.inputs[2] == 2 &&
          apply.inputs[3] == IN_ORIGINAL && apply.inputs[4] == IN_PREVIOUS && apply.output == IN_NONE);
    CHECK(plan.passes[3].instance == 2 && plan.passes[3].output == 3 && plan.passes[4].inputs[0] == 3);
    CHECK(plan.uses_previous());
    CHECK(pass_title("Realistic", "ambient_light", "wide", "apply") ==
          "Realistic \xC2\xB7 ambient_light.wide \xC2\xB7 apply");
    CHECK(pass_title("R", "grain", "", "grain") == "R \xC2\xB7 grain \xC2\xB7 grain");

    ActiveSpec none;
    CHECK(make_plan(none, effects).passes.empty() && !make_plan(none, effects).uses_previous());

    ActiveSpec tweaked = act;
    tweaked.instances[0].params = {0.9f, 0};
    Plan changed = make_plan(tweaked, effects);
    CHECK(plan_shape(changed) == plan_shape(plan));
    CHECK(changed.instances[0].options[0] == 0.9f);
    ActiveSpec reordered = act;
    std::swap(reordered.instances[0], reordered.instances[1]);
    Plan swapped = make_plan(reordered, effects);
    CHECK(plan_shape(swapped) != plan_shape(plan));
    std::vector<int> keep = reuse_textures(plan, swapped);
    CHECK(keep.size() == 6 && keep[0] == -1 && keep[1] == -1 && keep[2] == -1);
    ActiveSpec appended = act;
    appended.instances.insert(appended.instances.begin() + 4, InstanceSpec{"grain", "second", {0, 0}});
    Plan grown = make_plan(appended, effects);
    CHECK(plan_shape(grown) != plan_shape(plan));
    std::vector<int> grown_keep = reuse_textures(plan, grown);
    CHECK(grown_keep.size() == 6 && grown_keep[0] == 0 && grown_keep[1] == 1 && grown_keep[2] == 2 && grown_keep[3] == 3 &&
          grown_keep[5] == 5);
    effects[0].textures[0].scale = 0.25;
    Plan resized = make_plan(act, effects);
    std::vector<int> resized_keep = reuse_textures(plan, resized);
    CHECK(resized_keep[0] == -1 && resized_keep[1] == 1 && resized_keep[3] == -1 && resized_keep[4] == 4);

    uint8_t flat[4 * 4 * 3], quarter[2 * 2 * 3];
    for (auto &b : flat)
        b = 200;
    box_scale_rgb(flat, 4, 4, quarter, 2, 2);
    CHECK(quarter[0] == 200 && quarter[11] == 200);
    uint8_t pair[2 * 1 * 3] = {0, 100, 255, 100, 100, 1}, one[3];
    box_scale_rgb(pair, 2, 1, one, 1, 1);
    CHECK(one[0] == 50 && one[1] == 100 && one[2] == 128);
    uint8_t same[2 * 1 * 3], keepit[2 * 1 * 3] = {1, 2, 3, 4, 5, 6};
    box_scale_rgb(keepit, 2, 1, same, 2, 1);
    CHECK(memcmp(same, keepit, 6) == 0);
    std::vector<uint8_t> src(1920 * 1080 * 3, 90), dst(1280 * 720 * 3);
    box_scale_rgb(src.data(), 1920, 1080, dst.data(), 1280, 720);
    CHECK(dst.front() == 90 && dst.back() == 90 && dst[dst.size() / 2] == 90);
    uint8_t grad[3 * 1 * 3] = {0, 0, 0, 90, 90, 90, 255, 255, 255}, two[2 * 1 * 3];
    box_scale_rgb(grad, 3, 1, two, 2, 1);
    CHECK(two[0] == 30 && two[3] == 200);

    // Data from an addon's code: the addon a DLL belongs to is the folder under the game's swofty\addons\ holding it.
    const std::wstring exe = L"D:\\SteamLibrary\\steamapps\\common\\F1 25\\F1_25.exe";
    const std::wstring addons = L"D:\\SteamLibrary\\steamapps\\common\\F1 25\\swofty\\addons\\";
    CHECK(addon_of_module(addons + L"xsoul.cinematic-replay\\native\\cinecam.dll", exe) == "xsoul.cinematic-replay");
    CHECK(addon_of_module(L"d:\\steamlibrary\\STEAMAPPS\\common\\f1 25\\Swofty\\Addons\\Example.Marker\\x.dll", exe) ==
          "example.marker");
    CHECK(addon_of_module(L"D:/SteamLibrary/steamapps/common/F1 25/swofty/addons/a.b/native/x.dll", exe) == "a.b");
    // a DLL in a deeper folder named like another addon is still its own addon's
    CHECK(addon_of_module(addons + L"evil\\native\\swofty\\addons\\xsoul.cinematic-replay\\x.dll", exe) == "evil");
    // anywhere else, nobody's, whatever the path says
    CHECK(addon_of_module(L"C:\\Games\\swofty\\addons\\xsoul.cinematic-replay\\x.dll", exe).empty());
    CHECK(addon_of_module(L"D:\\SteamLibrary\\steamapps\\common\\F1 25\\swofty\\mods\\a.b\\x.dll", exe).empty());
    CHECK(addon_of_module(addons + L"loose.dll", exe).empty());
    CHECK(addon_of_module(addons + L"\\x.dll", exe).empty());
    CHECK(addon_of_module(L"C:\\Windows\\System32\\kernel32.dll", exe).empty());
    CHECK(addon_of_module(addons + L"a.b\\x.dll", L"F1_25.exe").empty());
    // a game folder inside some other addons folder: only the game's own counts
    CHECK(addon_of_module(L"D:\\addons\\F1 25\\swofty\\addons\\a.b\\x.dll", L"D:\\addons\\F1 25\\F1_25.exe") == "a.b");
    CHECK(addon_of_module(L"D:\\addons\\a.b\\x.dll", L"D:\\addons\\F1 25\\F1_25.exe").empty());

    // The bytes wait for the pass list: any pass's before it (EARLY_PASSES an addon), only the listed passes' after.
    DataStore store;
    const std::string mine = DataStore::key("a.b", "lines"), typo = DataStore::key("a.b", "lnes");
    CHECK(!store.is_listed() && store.set_early(mine, "abcd", 4) && store.set_early(typo, "xy", 2));
    for (size_t i = 2; i < DataStore::EARLY_PASSES; i++)
        CHECK(store.set_early(DataStore::key("a.b", "p" + std::to_string(i)), "z", 1));
    CHECK(!store.set_early(DataStore::key("a.b", "one_more"), "z", 1) && store.size() == DataStore::EARLY_PASSES);
    CHECK(store.set_early(mine, "efgh", 4) && store.set_early(DataStore::key("c.d", "fx"), "z", 1));
    CHECK(store.set_early(typo, nullptr, 0) && !store.find(typo));
    store.listed([&](const std::string &k) { return k == mine; });
    CHECK(store.is_listed() && store.size() == 1 && store.find(mine) && *store.find(mine) == "efgh");
    store.set(mine, nullptr, 0);
    CHECK(store.empty() && !store.find(mine));

    {
        // A frame generation swapchain: the HUD's list is submitted before the list that closes the step, so the
        // passes wait for Present, and Present shows the swapchain's copy of the frame, never the game's back
        // buffer. The passes run on the copy while it holds the waiting picture.
        BackPlace p;
        p.drawn(70);
        CHECK(p.picture_submitted(70, true, true) == END_DEFER);
        p.moved(false);
        CHECK(!p.step_closed(70, true) && p.waits(70));
        CHECK(shown_is_waiting(false, 0, 70, p.opened_at, true));
        CHECK(p.presented(70) && !p.waits(70) && placed_lately(70, p.placed_at));
        // frame generation on: a Present may show a generated frame or an older one
        CHECK(!shown_is_waiting(true, 0, 70, 70, true));
        // passes already recorded since the last Present: the shown frame may be the one that has them
        CHECK(!shown_is_waiting(false, 1, 70, 70, true));
        // the step opened in an earlier frame: its picture was shown by an earlier Present
        CHECK(!shown_is_waiting(false, 0, 71, 70, true));
        // a copy of another size or format would rebuild the passes' textures every frame
        CHECK(!shown_is_waiting(false, 0, 70, 70, false));
        // a core that saw the list taking the back buffer to the HUD runs the passes right before it, so nothing waits
        // for Present and the copy is left alone
        BackPlace q;
        q.drawn(80);
        CHECK(q.picture_submitted(80, true, true) == END_DEFER);
        q.moved(true);
        CHECK(q.target_left(80, true) && !q.waits(80) && !q.presented(80));
    }

    if (failed)
        return 1;
    std::printf("postfx logic: all checks passed\n");
    return 0;
}
