#include "../src/core/save_diag_logic.hpp"

#include <cstdio>

namespace sd = runtime::save_diag;
namespace cs = runtime::career_saves;

static int fails;
static void expect(bool ok, const std::string &what)
{
    printf("%s: %s\n", ok ? "ok  " : "FAIL", what.c_str());
    fails += !ok;
}
static bool has(const std::string &s, const char *part) { return s.find(part) != std::string::npos; }

int main()
{
    expect(sd::utc(0) == "unknown", "no time reads unknown");
    expect(sd::utc(1762000000) == "2025-11-01 12:26:40 UTC", "a unix time formats as UTC: " + sd::utc(1762000000));
    expect(sd::utc(951782400) == "2000-02-29 00:00:00 UTC", "a leap day formats");
    expect(has(sd::off_line(true, "main", ""), "off for profile main: the game reads Steam's saves natively"), "off with a profile");
    expect(has(sd::off_line(false, "", ""), "no launcher_saves") && has(sd::off_line(false, "", ""), "Steam's saves natively"),
           "off without settings still says Steam natively");
    expect(has(sd::off_line(true, "p", "asked once"), "(asked once)"), "off carries why");
    expect(sd::file_line("profile", 1234, 1762000000) == "save file profile: 1234 bytes, modified 2025-11-01 12:26:40 UTC",
           "file line");

    cs::SlotRead s;
    s.id = 3, s.guid = 0xabcdef, s.details = true, s.setup_mode = 2, s.season = 4, s.weekend = 7, s.date = 99, s.name = "Run";
    std::string l = sd::slot_line(2, 0, 1, s, "dc-000b0000");
    expect(has(l, "profile load 2: manager 0 slot 1: gm_driver_career id=3 key=0000000000abcdef career=dc-000b0000"), l);
    expect(has(l, "season=5 weekend=7 date=99 name=\"Run\""), "slot details");
    s.flags = cs::FLAG_MY_TEAM;
    expect(has(sd::slot_line(1, 1, 0, s, ""), "gm_my_team") && has(sd::slot_line(1, 1, 0, s, ""), "career=?"), "my team, unknown career");
    cs::SlotRead h;
    h.details = true, h.flags = cs::FLAG_HIDDEN;
    expect(has(sd::slot_line(1, 0, 0, h, ""), "(not a career slot)"), "hidden slot flagged");
    expect(has(sd::manager_line(1, 0, 3, 0x1234), "manager 0 (vtable +0x1234): 3 slot(s)"), "manager line");
    expect(has(sd::kept_line(1, "gm_driver_career_progressive", 900), "container gm_driver_career_progressive: 900 bytes"), "kept line");
    printf("save diag: %d failed\n", fails);
    return fails != 0;
}
