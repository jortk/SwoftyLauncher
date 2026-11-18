#pragma once

#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>

// Pure logic of the fast-fail crash reports (crash.cpp): which fast-fail code each hooked path raises, how the
// report's reason line words it, and the byte pattern match that finds the exe's own copies of the CRT paths
// on a build with no known address. No Windows calls, so the unit test runs on the host.

namespace fast_fail {

constexpr uint32_t kStackBufferOverrun = 0xC0000409;
constexpr uint32_t kFailFastException = 0xC0000602;
constexpr uint32_t kInvalidCRuntimeParameter = 0xC0000417;

enum class Source { GsFailure, InvalidParameter, Abort, RaiseFailFast };

struct Outcome {
    bool has_code;
    uint32_t code;
    uint32_t exit_status;
};

inline const char *code_name(uint32_t code)
{
    switch (code) {
    case 0: return "FAST_FAIL_LEGACY_GS_VIOLATION";
    case 1: return "FAST_FAIL_VTGUARD_CHECK_FAILURE";
    case 2: return "FAST_FAIL_STACK_COOKIE_CHECK_FAILURE";
    case 3: return "FAST_FAIL_CORRUPT_LIST_ENTRY";
    case 4: return "FAST_FAIL_INCORRECT_STACK";
    case 5: return "FAST_FAIL_INVALID_ARG";
    case 6: return "FAST_FAIL_GS_COOKIE_INIT";
    case 7: return "FAST_FAIL_FATAL_APP_EXIT";
    case 8: return "FAST_FAIL_RANGE_CHECK_FAILURE";
    case 9: return "FAST_FAIL_UNSAFE_REGISTRY_ACCESS";
    case 10: return "FAST_FAIL_GUARD_ICALL_CHECK_FAILURE";
    case 11: return "FAST_FAIL_GUARD_WRITE_CHECK_FAILURE";
    case 12: return "FAST_FAIL_INVALID_FIBER_SWITCH";
    case 13: return "FAST_FAIL_INVALID_SET_OF_CONTEXT";
    case 14: return "FAST_FAIL_INVALID_REFERENCE_COUNT";
    case 18: return "FAST_FAIL_INVALID_JUMP_BUFFER";
    case 19: return "FAST_FAIL_MRDATA_MODIFIED";
    case 21: return "FAST_FAIL_INVALID_EXCEPTION_CHAIN";
    case 22: return "FAST_FAIL_CRYPTO_LIBRARY";
    case 28: return "FAST_FAIL_INVALID_BUFFER_ACCESS";
    case 29: return "FAST_FAIL_INVALID_BALANCED_TREE";
    case 35: return "FAST_FAIL_UNEXPECTED_HEAP_EXCEPTION";
    case 37: return "FAST_FAIL_GUARD_JUMPTABLE";
    case 38: return "FAST_FAIL_INVALID_LONGJUMP_TARGET";
    }
    return "FAST_FAIL_UNKNOWN";
}

inline const char *source_name(Source s)
{
    switch (s) {
    case Source::GsFailure: return "__report_gsfailure (a stack cookie check failed)";
    case Source::InvalidParameter: return "the CRT's invalid-parameter terminate path";
    case Source::Abort: return "abort() in the CRT";
    case Source::RaiseFailFast: return "RaiseFailFastException";
    }
    return "?";
}

inline Outcome outcome(Source s, bool fast_fail_available, uint32_t raised_status)
{
    switch (s) {
    case Source::GsFailure:
        return {true, 2, kStackBufferOverrun};
    case Source::InvalidParameter:
        return fast_fail_available ? Outcome{true, 5, kStackBufferOverrun}
                                   : Outcome{false, 0, kInvalidCRuntimeParameter};
    case Source::Abort:
        return {true, 7, kStackBufferOverrun};
    case Source::RaiseFailFast:
        return {false, 0, raised_status ? raised_status : kFailFastException};
    }
    return {false, 0, 0};
}

inline bool is_fast_fail_status(uint32_t status)
{
    return status == kStackBufferOverrun || status == kFailFastException ||
           status == kInvalidCRuntimeParameter;
}

inline std::string reason(Source s, const Outcome &o, const std::string &at)
{
    char b[512];
    if (o.has_code)
        snprintf(b, sizeof b, "fast-fail %s (%u) from %s; the process would end with exit code 0x%08x; at ",
                 code_name(o.code), o.code, source_name(s), o.exit_status);
    else
        snprintf(b, sizeof b, "fast-fail from %s; the process would end with exit code 0x%08x; at ", source_name(s),
                 o.exit_status);
    return b + at;
}

inline std::string exception_line(uint32_t status, bool has_param, uint64_t param)
{
    if (!is_fast_fail_status(status))
        return {};
    char b[160];
    if (status == kStackBufferOverrun && has_param)
        snprintf(b, sizeof b, "fast-fail code %llu: %s", (unsigned long long)param, code_name(uint32_t(param)));
    else if (status == kFailFastException)
        snprintf(b, sizeof b, "RaiseFailFastException with no exception record of its own");
    else if (status == kInvalidCRuntimeParameter)
        snprintf(b, sizeof b, "the CRT's invalid-parameter fallback, used when fast-fail is unavailable");
    else
        snprintf(b, sizeof b, "fast-fail");
    return b;
}

struct Pattern {
    std::vector<uint8_t> bytes;
    std::vector<uint8_t> mask;
};

inline Pattern parse_pattern(const char *text)
{
    Pattern p;
    for (const char *c = text; *c;) {
        while (*c == ' ')
            c++;
        if (!*c)
            break;
        if (c[0] == '?') {
            p.bytes.push_back(0);
            p.mask.push_back(0);
            while (*c && *c != ' ')
                c++;
            continue;
        }
        char *end;
        p.bytes.push_back(uint8_t(strtoul(c, &end, 16)));
        p.mask.push_back(0xFF);
        c = end;
    }
    return p;
}

inline bool matches(const uint8_t *at, const Pattern &p)
{
    for (size_t i = 0; i < p.bytes.size(); i++)
        if ((at[i] & p.mask[i]) != p.bytes[i])
            return false;
    return true;
}

inline size_t first_fixed(const Pattern &p)
{
    size_t i = 0;
    while (i < p.mask.size() && !p.mask[i])
        i++;
    return i;
}

}
