// SPDX-License-Identifier: MIT
#pragma once
#include <cstdint>
#include <cstdio>
#include <initializer_list>
#include <string>
#include <string_view>
#include <vector>

#if defined(_WIN32)
#include <windows.h>
#endif
#include <swofty_api.h>

// A CSV recorder for validation logs. The core owns the file and the writer thread; the frame thread only
// copies a fixed row into a ring. A row is dropped and counted when the ring is full, never blocking the
// game. Files go to swofty/addons/<addon id>/diag/<name>-<timestamp>.csv.
//
//   static swofty::diag::Csv csv("tower", {{"t", Csv::f32}, {"pos", Csv::i32}, {"gap", Csv::text}}, 30.0);
//   if (auto row = csv.row()) { row.f32(0, t); row.i32(1, pos); row.text(2, "+1.2"); }   // commits at scope end
//
// hz is a rate cap: row() answers an empty Row when called faster than hz. The table is read from the
// core's SwoftyCoreCsv export, so an older core makes the recorder inert (available() is false).

namespace swofty::diag {

namespace detail {

inline const SwoftyCsv *&csv_override()
{
    static const SwoftyCsv *t = nullptr;
    return t;
}

inline const SwoftyCsv *csv_table()
{
    if (auto *o = csv_override())
        return o;
#if defined(_WIN32)
    static const SwoftyCsv *found = [] {
        HMODULE core = GetModuleHandleW(L"swofty_core.dll");
        auto get = core ? reinterpret_cast<const SwoftyCsv *(*)()>(GetProcAddress(core, "SwoftyCoreCsv")) : nullptr;
        const SwoftyCsv *t = get ? get() : nullptr;
        return t && t->version >= 1 ? t : nullptr;
    }();
    return found;
#else
    return nullptr;
#endif
}

}

class Csv {
public:
    enum Type : uint32_t { i32 = SWOFTY_CSV_I32, f32 = SWOFTY_CSV_F32, text = SWOFTY_CSV_TEXT, boolean = SWOFTY_CSV_BOOL };
    struct Column {
        const char *name;
        Type type;
    };

    class Row {
    public:
        Row() = default;
        Row(const SwoftyCsv *t, uint32_t h, uint32_t n) : t_(t), h_(h), n_(n) {}
        Row(const Row &) = delete;
        Row(Row &&o) noexcept : t_(o.t_), h_(o.h_), n_(o.n_) { o.t_ = nullptr; }
        ~Row()
        {
            if (t_)
                t_->row_commit(h_);
        }
        explicit operator bool() const { return t_ != nullptr; }
        void i32(uint32_t col, int32_t v) { if (t_ && col < n_) t_->row_i32(h_, col, v); }
        void f32(uint32_t col, float v) { if (t_ && col < n_) t_->row_f32(h_, col, v); }
        void boolean(uint32_t col, bool v) { if (t_ && col < n_) t_->row_i32(h_, col, v ? 1 : 0); }
        void text(uint32_t col, std::string_view v)
        {
            if (t_ && col < n_)
                t_->row_text(h_, col, v.data(), uint32_t(v.size()));
        }

    private:
        const SwoftyCsv *t_ = nullptr;
        uint32_t h_ = 0, n_ = 0;
    };

    Csv(std::string_view name, std::initializer_list<Column> cols, double hz = 0) : name_(name), hz_(hz)
    {
        for (auto &c : cols) {
            names_.push_back(c.name);
            types_.push_back(c.type);
        }
    }

    bool available() const { return detail::csv_table() != nullptr; }

    Row row()
    {
        auto *t = detail::csv_table();
        if (!t)
            return {};
        if (!opened_ && !open(t))
            return {};
        if (hz_ > 0) {
            double now = t->clock ? t->clock() : 0;
            if (now < next_)
                return {};
            next_ = now + 1.0 / hz_;
        }
        return Row(t, handle_, uint32_t(names_.size()));
    }

    uint64_t dropped() const
    {
        auto *t = detail::csv_table();
        return t && opened_ ? t->dropped(handle_) : 0;
    }
    void close()
    {
        auto *t = detail::csv_table();
        if (t && opened_)
            t->close(handle_);
        opened_ = false;
    }

private:
    bool open(const SwoftyCsv *t)
    {
        std::vector<const char *> n(names_.size());
        for (size_t i = 0; i < n.size(); i++)
            n[i] = names_[i].c_str();
        handle_ = ~0u;
        if (t->open(name_.c_str(), n.data(), types_.data(), uint32_t(n.size()), &handle_) != SWOFTY_ASSET_OK)
            return false;
        opened_ = true;
        return true;
    }

    std::string name_;
    std::vector<std::string> names_;
    std::vector<uint32_t> types_;
    double hz_ = 0, next_ = 0;
    uint32_t handle_ = ~0u;
    bool opened_ = false;
};

}
