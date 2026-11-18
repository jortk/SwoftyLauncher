#pragma once
#include <memory>
#include <string>
#include <utility>
#include <vector>

#include <swofty/json.hpp>

#include "hooks/context.hpp"

// The texts one lng key has, one variant per lang.string record in load order, each with the scope
// its record's within names. choose() returns the text of the last variant whose scope matches the
// current facts and that has a text for the current language, or nothing when none applies.

namespace swofty_core::lang_scope {

struct Variant {
    const char *all = nullptr;
    std::vector<std::pair<std::string, const char *>> per;
    std::shared_ptr<const hooks::Selectors> within;
};

inline bool compile_within(const swofty::json::Value &within, std::shared_ptr<const hooks::Selectors> &out,
                           std::string *why)
{
    out.reset();
    if (within.is_null())
        return true;
    swofty::json::Value list = within.is_array() ? within : swofty::json::Value::array();
    if (!within.is_array())
        list.push_back(within);
    auto sel = std::make_shared<hooks::Selectors>();
    if (!hooks::Selectors::compile(swofty::json::dump(list), *sel, why))
        return false;
    if (!sel->empty())
        out = std::move(sel);
    return true;
}

inline bool scoped(const std::vector<Variant> &variants)
{
    for (auto &v : variants)
        if (v.within)
            return true;
    return false;
}

inline bool same_code(const std::string &a, const char *b)
{
    size_t i = 0;
    for (; i < a.size() && b[i]; i++) {
        char x = a[i], y = b[i];
        if (x >= 'A' && x <= 'Z')
            x = char(x - 'A' + 'a');
        if (y >= 'A' && y <= 'Z')
            y = char(y - 'A' + 'a');
        if (x != y)
            return false;
    }
    return i == a.size() && !b[i];
}

inline const char *text_of(const Variant &v, const char *code)
{
    if (code)
        for (auto &[c, text] : v.per)
            if (same_code(c, code))
                return text;
    return v.all;
}

inline const char *choose(const std::vector<Variant> &variants, const hooks::Facts *facts, const char *code)
{
    for (auto it = variants.rbegin(); it != variants.rend(); ++it) {
        if (it->within && (!facts || !it->within->match(*facts)))
            continue;
        if (const char *text = text_of(*it, code))
            return text;
    }
    return nullptr;
}

}
