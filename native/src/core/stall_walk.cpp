#include "stall_walk.hpp"

namespace stall_walk {

namespace {

void keep(const CONTEXT *ctx, int frame, Kept *kept)
{
    if (!kept || kept->at.frame >= 0 || ctx->Rip < kept->lo || ctx->Rip >= kept->hi)
        return;
    runtime::stall_order::Kept &k = kept->at;
    k.frame = frame;
    k.rip = ctx->Rip;
    k.rsp = ctx->Rsp;
    k.rbx = ctx->Rbx;
    k.rbp = ctx->Rbp;
    k.rsi = ctx->Rsi;
    k.rdi = ctx->Rdi;
    k.r12 = ctx->R12;
    k.r13 = ctx->R13;
    k.r14 = ctx->R14;
    k.r15 = ctx->R15;
}

int frames(CONTEXT *ctx, uintptr_t lo, uintptr_t hi, int max, Lookup lookup, Readable readable, uint64_t *rip,
           uint64_t *rsp, Kept *kept)
{
    int n = 0;
    for (int i = 0; i < max && ctx->Rip; i++) {
        rip[n] = ctx->Rip;
        rsp[n] = ctx->Rsp;
        keep(ctx, n, kept);
        n++;
        if (ctx->Rsp < lo || ctx->Rsp >= hi)
            break;
        DWORD64 ib = 0, before = ctx->Rsp;
        PRUNTIME_FUNCTION f = lookup(ctx->Rip, &ib);
        if (f) {
            void *hd = nullptr;
            DWORD64 ef = 0;
            RtlVirtualUnwind(UNW_FLAG_NHANDLER, ib, ctx->Rip, f, ctx, &hd, &ef, nullptr);
        } else {
            if (!readable(ctx->Rsp, 8))
                break;
            ctx->Rip = *reinterpret_cast<DWORD64 *>(ctx->Rsp);
            ctx->Rsp += 8;
        }
        if (ctx->Rsp < before)
            break;
    }
    return n;
}

}

int walk(CONTEXT *ctx, uintptr_t lo, uintptr_t hi, int max, Lookup lookup, Readable readable, uint64_t *rip,
         uint64_t *rsp, bool *faulted, Kept *kept)
{
    *faulted = false;
    if (kept)
        kept->at = {};
    for (int i = 0; i < max; i++)
        rip[i] = rsp[i] = 0;
    __try {
        return frames(ctx, lo, hi, max, lookup, readable, rip, rsp, kept);
    } __except (EXCEPTION_EXECUTE_HANDLER) {
        *faulted = true;
    }
    int n = 0;
    while (n < max && rip[n])
        n++;
    return n;
}

}
