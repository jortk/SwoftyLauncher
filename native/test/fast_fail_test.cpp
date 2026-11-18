// Host unit test for the fast-fail crash report logic (fast_fail_logic.hpp). run_fast_fail_test.sh builds and runs it.
#include "../src/core/fast_fail_logic.hpp"

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
    using fast_fail::Source;
    expect(std::string(fast_fail::code_name(2)) == "FAST_FAIL_STACK_COOKIE_CHECK_FAILURE", "code 2 is the stack cookie check");
    expect(std::string(fast_fail::code_name(5)) == "FAST_FAIL_INVALID_ARG", "code 5 is an invalid argument");
    expect(std::string(fast_fail::code_name(7)) == "FAST_FAIL_FATAL_APP_EXIT", "code 7 is a fatal app exit");
    expect(std::string(fast_fail::code_name(999)) == "FAST_FAIL_UNKNOWN", "an unlisted code is unknown");

    auto gs = fast_fail::outcome(Source::GsFailure, true, 0);
    expect(gs.has_code && gs.code == 2 && gs.exit_status == 0xC0000409, "a failed stack cookie ends with 0xc0000409 and code 2");
    auto gs_old = fast_fail::outcome(Source::GsFailure, false, 0);
    expect(gs_old.code == 2 && gs_old.exit_status == 0xC0000409, "without fast-fail the cookie path still raises 0xc0000409");
    auto inv = fast_fail::outcome(Source::InvalidParameter, true, 0);
    expect(inv.has_code && inv.code == 5 && inv.exit_status == 0xC0000409, "an invalid parameter fast-fails with code 5");
    auto inv_old = fast_fail::outcome(Source::InvalidParameter, false, 0);
    expect(!inv_old.has_code && inv_old.exit_status == 0xC0000417, "without fast-fail an invalid parameter ends with 0xc0000417");
    auto ab = fast_fail::outcome(Source::Abort, true, 0);
    expect(ab.has_code && ab.code == 7 && ab.exit_status == 0xC0000409, "abort fast-fails with code 7");
    auto rf = fast_fail::outcome(Source::RaiseFailFast, true, 0);
    expect(!rf.has_code && rf.exit_status == 0xC0000602, "RaiseFailFastException without a record ends with 0xc0000602");
    auto rf2 = fast_fail::outcome(Source::RaiseFailFast, true, 0xC0000005);
    expect(rf2.exit_status == 0xC0000005, "RaiseFailFastException ends with its record's code");

    expect(fast_fail::is_fast_fail_status(0xC0000409) && fast_fail::is_fast_fail_status(0xC0000602) &&
               fast_fail::is_fast_fail_status(0xC0000417),
           "the three fast-fail exit statuses are recognised");
    expect(!fast_fail::is_fast_fail_status(0xC0000005) && !fast_fail::is_fast_fail_status(0), "an access violation is not one");

    expect(fast_fail::reason(Source::GsFailure, gs, "base+0x1234") ==
               "fast-fail FAST_FAIL_STACK_COOKIE_CHECK_FAILURE (2) from __report_gsfailure (a stack cookie check failed); the "
               "process would end with exit code 0xc0000409; at base+0x1234",
           "the gs failure reason names the function, the code and the exit code");
    expect(fast_fail::reason(Source::RaiseFailFast, rf, "x") ==
               "fast-fail from RaiseFailFastException; the process would end with exit code 0xc0000602; at x",
           "a reason without a code omits it");
    expect(fast_fail::exception_line(0xC0000409, true, 7) == "fast-fail code 7: FAST_FAIL_FATAL_APP_EXIT",
           "the exception section names parameter 0");
    expect(fast_fail::exception_line(0xC0000005, true, 7).empty(), "no fast-fail line for other exceptions");

    auto p = fast_fail::parse_pattern("48 83 EC 28 E8 ?? ?? ?? ?? 48 85 C0");
    expect(p.bytes.size() == 12 && p.mask[5] == 0 && p.mask[4] == 0xFF, "a pattern parses its wildcards");
    expect(fast_fail::first_fixed(p) == 0 && fast_fail::first_fixed(fast_fail::parse_pattern("?? 48")) == 1,
           "the first fixed byte is found");
    const uint8_t code[] = {0x90, 0x48, 0x83, 0xEC, 0x28, 0xE8, 0x11, 0x22, 0x33, 0x44, 0x48, 0x85, 0xC0};
    expect(fast_fail::matches(code + 1, p), "a pattern matches across its wildcards");
    expect(!fast_fail::matches(code, p), "a pattern does not match at another offset");
    return fails ? 1 : 0;
}
