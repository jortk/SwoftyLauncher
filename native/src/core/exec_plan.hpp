#pragma once
#include <algorithm>
#include <climits>
#include <functional>
#include <utility>
#include <vector>

namespace d3d12hooks {

// What the subscribers of one ExecuteCommandLists call want done around the game's lists. Steps after the
// same list run in subscription order; a plan with no steps costs no allocation.
template <class Queue> struct BasicExecPlan {
    using Step = std::function<void(Queue *)>;
    struct Entry {
        unsigned index;
        Step fn;
    };
    static constexpr unsigned END = UINT_MAX;

    std::vector<Entry> steps;

    void after(unsigned i, Step step) { steps.push_back({i, std::move(step)}); }
    void at_end(Step step) { steps.push_back({END, std::move(step)}); }
    bool empty() const { return steps.empty(); }
};

// Submits lists[0..n) through run(q, count, first) in as few segments as the plan allows, with each step
// between the segments it was asked to follow.
template <class Queue, class List, class Run>
void run_plan(BasicExecPlan<Queue> &plan, Queue *q, unsigned n, List *const *lists, Run &&run)
{
    if (plan.steps.empty()) {
        run(q, n, lists);
        return;
    }
    unsigned last = n ? n - 1 : 0;
    for (auto &e : plan.steps)
        e.index = std::min(e.index, last);
    std::stable_sort(plan.steps.begin(), plan.steps.end(),
                     [](const auto &a, const auto &b) { return a.index < b.index; });
    unsigned start = 0;
    for (size_t s = 0; s < plan.steps.size();) {
        unsigned at = plan.steps[s].index;
        if (n && at + 1 > start) {
            run(q, at + 1 - start, lists + start);
            start = at + 1;
        }
        for (; s < plan.steps.size() && plan.steps[s].index == at; s++)
            plan.steps[s].fn(q);
    }
    if (start < n)
        run(q, n - start, lists + start);
}

}
