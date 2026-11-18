#pragma once
#include <algorithm>
#include <condition_variable>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <functional>
#include <mutex>
#include <string>
#include <vector>

namespace runtime::log_csv {

enum Type : uint32_t { I32 = 0, F32 = 1, TEXT = 2, BOOL = 3 };

constexpr uint32_t TEXT_BYTES = 48;
constexpr size_t RING_ROWS = 4096;

struct Cell {
    int32_t i = 0;
    float f = 0;
    char text[TEXT_BYTES] = {};
};

// One CSV file's recorder: a row is built on the caller's thread (set cells, commit), copied into a bounded
// ring, and formatted and written by the writer (drain) on another thread. A full ring drops the row and
// counts it; the caller never waits on the writer. commit/set are for one producer thread.
class Recorder {
public:
    Recorder(std::vector<std::string> names, std::vector<uint32_t> types)
        : names_(std::move(names)), types_(std::move(types)), cur_(names_.size()), ring_(RING_ROWS * names_.size())
    {
    }

    bool set_i32(uint32_t c, int32_t v)
    {
        if (c >= cur_.size())
            return false;
        cur_[c].i = v;
        return true;
    }

    bool set_f32(uint32_t c, float v)
    {
        if (c >= cur_.size())
            return false;
        cur_[c].f = v;
        return true;
    }

    bool set_text(uint32_t c, const char *s, uint32_t len)
    {
        if (c >= cur_.size())
            return false;
        len = std::min<uint32_t>(len, TEXT_BYTES - 1);
        std::memcpy(cur_[c].text, s, len);
        cur_[c].text[len] = 0;
        return true;
    }

    bool commit()
    {
        bool ok;
        {
            std::lock_guard<std::mutex> g(lock_);
            ok = count_ < RING_ROWS;
            if (ok) {
                size_t slot = (head_ + count_) % RING_ROWS;
                std::copy(cur_.begin(), cur_.end(), ring_.begin() + ptrdiff_t(slot * cur_.size()));
                count_++;
            } else {
                dropped_++;
            }
        }
        std::fill(cur_.begin(), cur_.end(), Cell{});
        return ok;
    }

    std::string header() const
    {
        std::string out;
        for (size_t i = 0; i < names_.size(); i++)
            out += (i ? "," : "") + names_[i];
        return out + "\n";
    }

    size_t drain(const std::function<void(const std::string &)> &write)
    {
        std::vector<Cell> take;
        size_t rows;
        {
            std::lock_guard<std::mutex> g(lock_);
            rows = count_;
            take.reserve(rows * cur_.size());
            for (size_t k = 0; k < rows; k++) {
                size_t slot = (head_ + k) % RING_ROWS;
                take.insert(take.end(), ring_.begin() + ptrdiff_t(slot * cur_.size()),
                            ring_.begin() + ptrdiff_t((slot + 1) * cur_.size()));
            }
            head_ = (head_ + rows) % RING_ROWS;
            count_ = 0;
        }
        std::string block;
        for (size_t r = 0; r < rows; r++) {
            for (size_t c = 0; c < cur_.size(); c++) {
                const Cell &cell = take[r * cur_.size() + c];
                char b[64];
                if (c)
                    block += ',';
                switch (types_[c]) {
                case F32:
                    std::snprintf(b, sizeof b, "%.6g", double(cell.f));
                    block += b;
                    break;
                case TEXT:
                    block += quote(cell.text);
                    break;
                default:
                    std::snprintf(b, sizeof b, "%d", cell.i);
                    block += b;
                }
            }
            block += '\n';
        }
        if (!block.empty())
            write(block);
        return rows;
    }

    uint64_t dropped() const
    {
        std::lock_guard<std::mutex> g(lock_);
        return dropped_;
    }

    size_t columns() const { return names_.size(); }

    static std::string quote(const char *s)
    {
        if (!std::strpbrk(s, ",\"\n\r"))
            return s;
        std::string out = "\"";
        for (; *s; s++) {
            if (*s == '"')
                out += '"';
            out += *s;
        }
        return out + "\"";
    }

private:
    std::vector<std::string> names_;
    std::vector<uint32_t> types_;
    std::vector<Cell> cur_, ring_;
    mutable std::mutex lock_;
    size_t head_ = 0, count_ = 0;
    uint64_t dropped_ = 0;
};

}
