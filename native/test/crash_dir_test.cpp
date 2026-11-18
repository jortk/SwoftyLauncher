// Host unit test for the crash report location logic (crash_dir.hpp). run_events_test.sh builds and runs it.
#include "../src/core/crash_dir.hpp"

#include <cstdio>
#include <string>

static int fails;

static void expect(bool ok, const std::string &what)
{
    printf("%s: %s\n", ok ? "ok  " : "FAIL", what.c_str());
    fails += !ok;
}

int main()
{
    expect(crash_dir::fallback(L"C:\\Users\\id\\AppData\\Local\\Swofty") == L"C:\\Users\\id\\AppData\\Local\\Swofty\\state\\",
           "the fallback is the state folder of the data folder, where loader-fallback.log goes");
    expect(crash_dir::fallback(L"C:\\Swofty\\") == L"C:\\Swofty\\state\\", "a trailing backslash is not doubled");
    expect(crash_dir::fallback(L"").empty(), "no data folder, no fallback");
    const std::string game = "C:\\Program Files\\EA Games\\F1 25\\swofty\\";
    expect(crash_dir::logged(game, game, "crash-1.txt") == "swofty\\crash-1.txt", "a report in the game folder is named swofty\\<name>");
    expect(crash_dir::logged("C:\\D\\state\\", game, "crash-1.txt") == "C:\\D\\state\\crash-1.txt",
           "a report in the data folder is named by its full path");
    return fails ? 1 : 0;
}
