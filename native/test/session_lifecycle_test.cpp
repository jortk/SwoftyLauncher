// Host-native unit test for the session end and unload ordering (native/src/core/session_lifecycle_logic.hpp).
// Built and run by native/test/run_events_test.sh.

#include "session_lifecycle_logic.hpp"

#include <cstdio>
#include <string>

namespace sl = runtime::session_life;

static int failed = 0;

static void expect(bool ok, const char *what)
{
    std::printf("%s: %s\n", ok ? "ok  " : "FAIL", what);
    if (!ok)
        failed++;
}

struct Run {
    sl::Lifecycle l;
    std::string log;
    void start()
    {
        if (l.on_start())
            log += "end(restart) ";
        log += "start ";
    }
    void apply(sl::Emit e)
    {
        if (e.end)
            log += e.reason == sl::END_UNLOAD ? "end(unload) " : "end ";
        if (e.unload)
            log += "unload ";
    }
    void exiting() { apply(l.on_session_exiting()); }
    void unloading() { apply(l.on_unload_entering()); }
};

int main()
{
    {
        Run r;
        r.start();
        r.exiting();
        r.unloading();
        expect(r.log == "start end unload ", "a finished session gives start, end, unload once each");
        r.exiting();
        r.unloading();
        expect(r.log == "start end unload ", "repeated exit and unload flow lines fire nothing more");
    }
    {
        Run r;
        r.start();
        r.exiting();
        r.start();
        r.exiting();
        r.unloading();
        expect(r.log == "start end start end unload ", "a restart through the session exit gives end then start again");
    }
    {
        Run r;
        r.start();
        r.start();
        expect(r.log == "start end(restart) start ", "a start while one is open ends it first");
    }
    {
        Run r;
        r.start();
        r.unloading();
        expect(r.log == "start end(unload) unload ", "a quit that skips the session exit still gives end then unload");
    }
    {
        Run r;
        r.unloading();
        r.exiting();
        expect(r.log.empty(), "an unload with no session started fires nothing");
    }
    {
        Run r;
        r.start();
        r.exiting();
        r.unloading();
        r.start();
        r.exiting();
        r.unloading();
        expect(r.log == "start end unload start end unload ", "a second session gets its own full sequence");
    }
    expect(sl::kind_of("time_trial") == sl::TIME_TRIAL, "a time trial session is TIME_TRIAL");
    expect(sl::kind_of("grand_prix") == sl::OTHER && sl::kind_of("career") == sl::OTHER && sl::kind_of("") == sl::OTHER,
           "grand prix, career and an unread mode are OTHER");
    expect(sl::time_trial_live(sl::TIME_TRIAL) && !sl::time_trial_live(sl::OTHER) && !sl::time_trial_live(sl::NONE),
           "only a time trial session is time trial live");
    expect(sl::session_live(sl::TIME_TRIAL) && sl::session_live(sl::OTHER) && !sl::session_live(sl::NONE), "NONE is no session");
    std::printf("%s\n", failed ? "FAILED" : "all passed");
    return failed ? 1 : 0;
}
