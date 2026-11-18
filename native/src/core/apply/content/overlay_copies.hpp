#pragma once

#include <cstdint>
#include <map>
#include <mutex>
#include <string>

// The team logo build's copies of launcher overlay archives (swofty/assets/resources/N.erp), handed from the logo
// build to the resource overlay, which registers a copy under the copy's own path in place of its overlay archive.

namespace swofty_core::apply::content::logo_deliver {

struct Copy {
    std::string path;
    uint64_t size = 0;
};

// The built copies of overlay archives, by the overlay's lower-case game path. The logo build offers them; the
// resource overlay takes them once, when it hands its archives to the manifest. An offer after that is refused
// unless it names the copies that were taken.
class OverlayCopies {
public:
    bool offer(const std::map<std::string, Copy> &copies)
    {
        std::lock_guard g(mu_);
        if (taken_) {
            if (copies.size() != held_.size())
                return false;
            for (auto &[overlay, c] : copies) {
                auto it = held_.find(overlay);
                if (it == held_.end() || it->second.path != c.path || it->second.size != c.size)
                    return false;
            }
            return true;
        }
        held_ = copies;
        return true;
    }

    std::map<std::string, Copy> take()
    {
        std::lock_guard g(mu_);
        taken_ = true;
        return held_;
    }

private:
    std::mutex mu_;
    std::map<std::string, Copy> held_;
    bool taken_ = false;
};

inline OverlayCopies &overlay_copies()
{
    static OverlayCopies copies;
    return copies;
}

inline std::string overlay_key(std::string path)
{
    for (char &c : path)
        c = c == '\\' ? '/' : c >= 'A' && c <= 'Z' ? char(c + 32) : c;
    while (!path.empty() && path.front() == '/')
        path.erase(path.begin());
    return path;
}

}
