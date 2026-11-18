#define WIN32_LEAN_AND_MEAN
#include <winsock2.h>
#include <windows.h>
#include <math.h>
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct NeStr NeStr;
typedef struct { int32_t type, pad; NeStr *str; } Action;
typedef struct { uint8_t *begin, *end, *cap; } Vec;
typedef struct { void *vt; char title[64], description[64]; int title_localised, description_localised; } VmNode;
typedef struct { const char *name; uint32_t index; } AdderCall;

void *g_registry = (void *)0x5157;
uint64_t out_r12, out_r15;
__declspec(dllexport) volatile int64_t probe[48];
void *fake_cmd_global;
const char fake_cmd_name[] = "select_pressed";
static uint8_t list[0x100];
static AdderCall calls[32];
static int ncalls, fails, live_strings, build_leak;
static void *career_vt_storage[40];
void *other_vt_storage[40];
#define CAREER_VT (career_vt_storage + 1)
#define OTHER_VT (other_vt_storage + 1)

void fake_builder(void *list, const uint8_t *flags, uint32_t start);
uint64_t fake_handle_command(void *vm, void *cmd);
int fake_caller(void);
int fake_patchable(void);
__declspec(dllexport) void *fake_exports[] = {(void *)fake_handle_command};
uint64_t handle_command_impl(void *vm, void *cmd) { (void)vm, (void)cmd; return 1; }
int fake_callee(void) { return 1; }
void fake_base_populate(uint8_t *item, void *node, uint32_t a7);
uint32_t fake_nestr_from(NeStr **out, const char *s, uint32_t flags, uintptr_t len);

static const char *chars(const NeStr *s) { return s ? (const char *)s + 9 : ""; }

static void nestr_release(NeStr **s)
{
    if (*s && --*(uint16_t *)((uint8_t *)*s + 4) == 0) {
        free(*s);
        live_strings--;
    }
    *s = NULL;
}

uint32_t nestr_impl(NeStr **out, const char *s)
{
    if (s && *s && *out && !strcmp(chars(*out), s))
        return 1;
    NeStr *old = *out;
    *out = NULL;
    if (s && *s) {
        size_t n = strlen(s);
        uint8_t *p = calloc(1, 16 + n);
        uint32_t h = 5381;
        for (size_t i = 0; i < n; i++)
            h = h * 33 + (uint8_t)s[i];
        memcpy(p, &h, 4);
        *(uint16_t *)(p + 4) = 1;
        memcpy(p + 9, s, n);
        *out = (NeStr *)p;
        live_strings++;
    }
    nestr_release(&old);
    return 1;
}

static VmNode *node_of(void *parent, void *reg)
{
    if (reg != g_registry) {
        printf("FAIL: registry %p != %p\n", reg, g_registry);
        fails++;
    }
    return (VmNode *)((uint8_t *)parent - 8);
}

void *fake_vm_set_string(void *reg, void *parent, const char *name, NeStr **value, int overwrite)
{
    VmNode *n = node_of(parent, reg);
    if (!strcmp(name, "title"))
        snprintf(n->title, sizeof n->title, "%s", chars(*value));
    else if (!strcmp(name, "description"))
        snprintf(n->description, sizeof n->description, "%s", chars(*value));
    (void)overwrite;
    return &g_registry;
}

void *fake_vm_set_bool(void *reg, void *parent, const char *name, uint8_t *value, int overwrite)
{
    VmNode *n = node_of(parent, reg);
    if (!strcmp(name, "title_localised"))
        n->title_localised = *value;
    else if (!strcmp(name, "description_localised"))
        n->description_localised = *value;
    (void)overwrite;
    return &g_registry;
}

void populate_impl(uint8_t *item, void *node, uint32_t a7)
{
    VmNode *n = calloc(1, sizeof *n);
    n->title_localised = n->description_localised = 1;
    *(VmNode **)(item + 0x20) = n;
    (void)node, (void)a7;
}

static void career_populate(uint8_t *item, void *node, uint32_t a7)
{
    fake_base_populate(item, node, a7);
    NeStr *t = NULL;
    fake_nestr_from(&t, "lng_mode_career", 0, 0);
    fake_vm_set_string(g_registry, *(uint8_t **)(item + 0x20) + 8, "title", &t, 1);
    nestr_release(&t);
}

static Action *id_action(uint8_t *item, Action *out)
{
    out->type = 1;
    out->str = *(NeStr **)(item + 0x10);
    if (out->str)
        ++*(uint16_t *)((uint8_t *)out->str + 4);
    return out;
}

static uintptr_t stub_zero(void) { return 0; }

static void add(void *l, uint32_t index, const char *name, void **vt)
{
    Vec *v = (Vec *)((uint8_t *)l + 0x78);
    uint8_t *tile = calloc(1, 0xd8), *item = calloc(1, 0x148);
    *(void ***)item = vt;
    fake_nestr_from((NeStr **)(item + 0x10), name, 0, 0);
    *(uint8_t **)(tile + 0xd0) = item;
    ((void **)v->end)[0] = tile + 8;
    ((void **)v->end)[1] = tile;
    v->end += 16;
    calls[ncalls++] = (AdderCall){name, index & 0xff};
}

void career_adder(void *l, uint32_t index) { add(l, index, "career", CAREER_VT); }
void movie_adder(void *l, uint32_t index) { add(l, index, "zenith_button", OTHER_VT); }
void world_adder(void *l, uint32_t index) { add(l, index, "f1_world", OTHER_VT); }

__asm__(
    ".section .xtls,\"xr\"\n"
    ".globl fake_builder\nfake_builder:\n"
    ".byte 0x41,0x57,0x41,0x56,0x41,0x55,0x41,0x54,0x56,0x57,0x55,0x53\n"
    ".byte 0x48,0x81,0xEC,0x88,0x02,0x00,0x00\n"
    ".byte 0xC5,0xF8,0x29,0xB4,0x24,0x70,0x02,0x00,0x00\n"
    ".byte 0x4C,0x89,0x8C,0x24,0xE8,0x02,0x00,0x00\n"
    "mov %rcx, %r14\nmov %rdx, %rsi\nmov %r8d, %r15d\n"
    ".byte 0x45,0x8D,0x67,0x01,0x4C,0x89,0xF1,0x44,0x89,0xE2,0xE8\n.long career_adder - (. + 4)\n"
    ".byte 0xF6,0x46,0x05,0x04,0x74,0x12\n"
    ".byte 0x41,0x80,0xC7,0x02,0x4C,0x89,0xF1,0x44,0x89,0xFA,0xE8\n.long movie_adder - (. + 4)\n"
    ".byte 0x45,0x89,0xFC\n"
    ".byte 0x45,0x8D,0x7C,0x24,0x01,0x4C,0x89,0xF1,0x44,0x89,0xFA,0xE8\n.long world_adder - (. + 4)\n"
    "mov %r12, out_r12(%rip)\nmov %r15, out_r15(%rip)\n"
    "vmovaps 0x270(%rsp), %xmm6\nadd $0x288, %rsp\n"
    "pop %rbx\npop %rbp\npop %rdi\npop %rsi\npop %r12\npop %r13\npop %r14\npop %r15\nret\n"

    ".globl fake_base_populate\nfake_base_populate:\n"
    ".byte 0x41,0x57,0x41,0x56,0x41,0x55,0x41,0x54,0x56,0x57,0x55,0x53\n"
    ".byte 0x48,0x81,0xEC,0x88,0x00,0x00,0x00\n"
    ".byte 0xC5,0xF8,0x29,0x74,0x24,0x70,0x44,0x89,0x44,0x24,0x6C\n"
    "call populate_impl\njmp 1f\n"
    ".org fake_base_populate+0x53, 0xCC\nmov g_registry(%rip), %rcx\n"
    ".org fake_base_populate+0x69, 0xCC\ncall fake_vm_set_string\n"
    ".org fake_base_populate+0x28e, 0xCC\ncall fake_vm_set_bool\n"
    "1:\nvmovaps 0x70(%rsp), %xmm6\nadd $0x88, %rsp\n"
    "pop %rbx\npop %rbp\npop %rdi\npop %rsi\npop %r12\npop %r13\npop %r14\npop %r15\nret\n"

    ".globl fake_nestr_from\nfake_nestr_from:\n"
    ".byte 0x41,0x57,0x41,0x56,0x41,0x54,0x56,0x57,0x55,0x53,0x48,0x83,0xEC,0x50,0x44,0x89,0xC6,0x48,0x89,0xCF\n"
    "call nestr_impl\nadd $0x50, %rsp\n"
    "pop %rbx\npop %rbp\npop %rdi\npop %rsi\npop %r12\npop %r14\npop %r15\nret\n"

    ".globl fake_handle_command\nfake_handle_command:\n"
    ".byte 0x41,0x57,0x41,0x56,0x56,0x57,0x53,0x48,0x83,0xEC,0x40,0x48,0x89,0xD7,0x48,0x89,0xCE\n"
    "mov fake_cmd_global(%rip), %rax\nlea fake_cmd_name(%rip), %rcx\n"
    "mov %rsi, %rcx\nmov %rdi, %rdx\ncall handle_command_impl\nadd $0x40, %rsp\n"
    "pop %rbx\npop %rdi\npop %rsi\npop %r14\npop %r15\nret\n"

    ".globl fake_vt_ref\nfake_vt_ref:\n"
    ".byte 0x48,0xB8,0x53,0x57,0x46,0x54,0x56,0x54,0x41,0x42\nlea other_vt_storage+8(%rip), %rax\nret\n"

    ".globl fake_caller\nfake_caller:\n"
    ".byte 0x48,0xB8,0x53,0x57,0x46,0x54,0x43,0x41,0x4C,0x4C\nsub $0x28, %rsp\ncall fake_callee\nadd $0x28, %rsp\nret\n"

    ".globl fake_patchable\nfake_patchable:\n"
    ".byte 0x48,0xB8,0x53,0x57,0x46,0x54,0x50,0x41,0x54,0x43\nmov $1, %eax\nret\n"

    ".globl fake_owned_state\nfake_owned_state:\n"
    ".byte 0x41,0x56,0x56,0x57,0x55,0x53,0x48,0x83,0xEC,0x20,0x48,0x8B,0x01,0x48,0x8B,0x71,0x08\n"
    ".byte 0xFF,0x50,0x08,0x8B,0x90,0xA0,0x00,0x00,0x00\n"
    "mov %edx, %ecx\ncall owned_impl\nadd $0x20, %rsp\n"
    "pop %rbx\npop %rbp\npop %rdi\npop %rsi\npop %r14\nret\n"

    // Reproduces the Vm_Update_FlowStateMachine prologue so the events module's flow-tick hook
    // resolves and the test can drive the per-frame tick; the body just restores and returns.
    ".globl fake_flow\nfake_flow:\n"
    ".byte 0x41,0x57,0x41,0x56,0x41,0x55,0x41,0x54,0x56,0x57,0x55,0x53\n"
    ".byte 0x48,0x81,0xEC,0x58,0x02,0x00,0x00\n"
    ".byte 0xC5,0xF8,0x29,0xB4,0x24,0x40,0x02,0x00,0x00\n"
    ".byte 0xC5,0xF9,0x28,0xF2\n"
    "vmovaps 0x240(%rsp), %xmm6\nadd $0x258, %rsp\n"
    "pop %rbx\npop %rbp\npop %rdi\npop %rsi\npop %r12\npop %r13\npop %r14\npop %r15\nret\n"
    ".section .data1,\"dwx\"\n"
    ".globl vm_thunk_register\nvm_thunk_register:\n"
    "sub $0x28, %rsp\ncall *__imp_RegisterClassExW(%rip)\nadd $0x28, %rsp\nret\n"
    ".text\n");
ATOM vm_thunk_register(const WNDCLASSEXW *wc);
void fake_flow(void);

__asm__(
    ".section .xtls,\"xr\"\n"
    ".globl fake_stage_site\nfake_stage_site:\n"
    ".byte 0x48,0xB8,0x53,0x57,0x46,0x54,0x53,0x49,0x54,0x45\nmov $7, %eax\nret\n"
    ".globl fake_stage_wrap\nfake_stage_wrap:\n"
    ".byte 0x48,0xB8,0x53,0x57,0x46,0x54,0x57,0x52,0x41,0x50\nmov $9, %eax\nret\n"
    ".globl fake_stage_plain\nfake_stage_plain:\n"
    ".byte 0x48,0xB8,0x53,0x57,0x46,0x54,0x50,0x4C,0x41,0x4E\nmov $5, %eax\nret\n"
    ".text\n");
int fake_stage_site(void);
int fake_stage_wrap(void);
int fake_stage_plain(void);

struct FakeLogGate {
    uint32_t hashes[373];
    uint8_t enabled, pad[3];
};
__declspec(dllexport) struct FakeLogGate fake_log_gate;
__declspec(dllexport) volatile int64_t gamelog_body_calls;
uint8_t fake_rsi_area[0x600];
const char fake_chan_warn[] = "TEST_WARN", fake_chan_verbose[] = "TEST_VERBOSE", fake_chan_ui[] = "UI_FLOW";
const char fake_fmt_warn[] = "fake gate message from %s", fake_fmt_verbose[] = "fake verbose message from %s";
const char fake_fmt_ui[] = "fake ui info from %s";
const char fake_arg_a[] = "site_a", fake_arg_b[] = "site_b", fake_arg_c[] = "site_c";
void fake_log_site_a(void);
void fake_log_site_b(void);
void fake_log_site_c(void);
void fake_log_thunk(const char *channel, const char *fmt, ...);
const char fake_fmt_bad[] = "fake replay record %s";
const char fake_fmt_deadlock[] =
    "The signal timed out, this is unexpected and may indicate a deadlock, see trace above for Extra Info";
int64_t fake_format_string(NeStr **out, const char *fmt, ...);

void fake_log_body_impl(void) { gamelog_body_calls++; }

int64_t format_impl(NeStr **out, const char *fmt, va_list ap)
{
    char buf[0x200];
    int n = vsnprintf(buf, sizeof buf, fmt, ap);
    nestr_impl(out, buf);
    return n;
}

__asm__(
    ".section .xtls,\"xr\"\n"
    ".globl fake_log_site_a\nfake_log_site_a:\n"
    "push %rsi\npush %rdi\nsub $0x28, %rsp\nlea fake_rsi_area(%rip), %rsi\n"
    ".byte 0x80,0x3D\n.long fake_log_gate+0x5d4-(.+5)\n.byte 0x01\n"
    ".byte 0x75,0x74,0xB8,0x5F,0x01,0x00,0x00\n"
    ".byte 0x48,0x8D,0x0D\n.long fake_log_gate-(.+4)\n"
    ".byte 0x66,0x66,0x66,0x66,0x66,0x66,0x2E,0x0F,0x1F,0x84,0x00,0x00,0x00,0x00,0x00\n"
    ".byte 0x8B,0x14,0x81,0x85,0xD2,0x74,0x52,0x81,0xFA,0xCB,0x88,0x7C,0x7C,0x74,0x30\n"
    ".byte 0x83,0xC0,0x19,0x48,0x69,0xD0,0x43,0x43,0x66,0x5F,0x48,0xC1,0xEA,0x20,0x41,0x89,0xC0,0x41,0x29,0xD0,0x41,0xD1,0xE8,0x41,0x01,0xD0,0x41,0xC1,0xE8,0x08,0x41,0x69,0xD0,0x75,0x01,0x00,0x00,0x29,0xD0,0x3D,0x5F,0x01,0x00,0x00,0x75,0xC3,0xEB,0x1A\n"
    ".byte 0x48,0x8D,0x0D\n.long fake_chan_warn-(.+4)\n"
    ".byte 0x48,0x8D,0x15\n.long fake_fmt_warn-(.+4)\n"
    ".byte 0x4C,0x8D,0x05\n.long fake_arg_a-(.+4)\n"
    ".byte 0xE8\n.long fake_log_thunk-(.+4)\n"
    ".byte 0x48,0x8B,0xBE,0x98,0x04,0x00,0x00\n"
    "add $0x28, %rsp\npop %rdi\npop %rsi\nret\n"
    ".globl fake_log_site_b\nfake_log_site_b:\n"
    "push %rsi\npush %rdi\nsub $0x28, %rsp\nlea fake_rsi_area(%rip), %rsi\n"
    ".byte 0x80,0x3D\n.long fake_log_gate+0x5d4-(.+5)\n.byte 0x01\n"
    ".byte 0x75,0x74,0xB8,0x3A,0x01,0x00,0x00\n"
    ".byte 0x48,0x8D,0x0D\n.long fake_log_gate-(.+4)\n"
    ".byte 0x66,0x66,0x66,0x66,0x66,0x66,0x2E,0x0F,0x1F,0x84,0x00,0x00,0x00,0x00,0x00\n"
    ".byte 0x8B,0x14,0x81,0x85,0xD2,0x74,0x52,0x81,0xFA,0x11,0x11,0x11,0x11,0x74,0x30\n"
    ".byte 0x83,0xC0,0x11,0x48,0x69,0xD0,0x43,0x43,0x66,0x5F,0x48,0xC1,0xEA,0x20,0x41,0x89,0xC0,0x41,0x29,0xD0,0x41,0xD1,0xE8,0x41,0x01,0xD0,0x41,0xC1,0xE8,0x08,0x41,0x69,0xD0,0x75,0x01,0x00,0x00,0x29,0xD0,0x3D,0x3A,0x01,0x00,0x00,0x75,0xC3,0xEB,0x1A\n"
    ".byte 0x48,0x8D,0x0D\n.long fake_chan_verbose-(.+4)\n"
    ".byte 0x48,0x8D,0x15\n.long fake_fmt_verbose-(.+4)\n"
    ".byte 0x4C,0x8D,0x05\n.long fake_arg_b-(.+4)\n"
    ".byte 0xE8\n.long fake_log_thunk-(.+4)\n"
    ".byte 0x48,0x8B,0xBE,0x90,0x04,0x00,0x00\n"
    "add $0x28, %rsp\npop %rdi\npop %rsi\nret\n"
    ".globl fake_log_site_c\nfake_log_site_c:\n"
    "push %rsi\npush %rdi\nsub $0x28, %rsp\nlea fake_rsi_area(%rip), %rsi\n"
    ".byte 0x80,0x3D\n.long fake_log_gate+0x5d4-(.+5)\n.byte 0x01\n"
    ".byte 0x75,0x74,0xB8,0xFF,0x00,0x00,0x00\n"
    ".byte 0x48,0x8D,0x0D\n.long fake_log_gate-(.+4)\n"
    ".byte 0x66,0x66,0x66,0x66,0x66,0x66,0x2E,0x0F,0x1F,0x84,0x00,0x00,0x00,0x00,0x00\n"
    ".byte 0x8B,0x14,0x81,0x85,0xD2,0x74,0x52,0x81,0xFA,0x22,0x22,0x22,0x22,0x74,0x30\n"
    ".byte 0x83,0xC0,0x15,0x48,0x69,0xD0,0x43,0x43,0x66,0x5F,0x48,0xC1,0xEA,0x20,0x41,0x89,0xC0,0x41,0x29,0xD0,0x41,0xD1,0xE8,0x41,0x01,0xD0,0x41,0xC1,0xE8,0x08,0x41,0x69,0xD0,0x75,0x01,0x00,0x00,0x29,0xD0,0x3D,0xFF,0x00,0x00,0x00,0x75,0xC3,0xEB,0x1A\n"
    ".byte 0x48,0x8D,0x0D\n.long fake_chan_ui-(.+4)\n"
    ".byte 0x48,0x8D,0x15\n.long fake_fmt_ui-(.+4)\n"
    ".byte 0x4C,0x8D,0x05\n.long fake_arg_c-(.+4)\n"
    ".byte 0xE8\n.long fake_log_thunk-(.+4)\n"
    ".byte 0x48,0x8B,0xBE,0x88,0x04,0x00,0x00\n"
    "add $0x28, %rsp\npop %rdi\npop %rsi\nret\n"
    ".globl fake_log_thunk\nfake_log_thunk:\n"
    ".byte 0xE9\n.long fake_log_body-(.+4)\n"
    ".byte 0xCC,0xCC,0xCC,0xCC,0xCC,0xCC,0xCC,0xCC,0xCC,0xCC,0xCC\n"

    ".globl fake_format_string\nfake_format_string:\n"
    ".byte 0x41,0x57,0x41,0x56,0x56,0x57,0x55,0x53,0x48,0x81,0xEC,0x48,0x02,0x00,0x00\n"
    ".byte 0x48,0x89,0xD7,0x48,0x89,0xCE,0x4C,0x89,0x84,0x24,0x90,0x02,0x00,0x00\n"
    "mov %r9, 0x298(%rsp)\nmov %rsi, %rcx\nmov %rdi, %rdx\nlea 0x290(%rsp), %r8\ncall format_impl\n"
    "add $0x248, %rsp\npop %rbx\npop %rbp\npop %rdi\npop %rsi\npop %r14\npop %r15\nret\n"

    ".section .data1,\"dwx\"\n"
    ".globl fake_log_body\nfake_log_body:\n"
    "sub $0x28, %rsp\ncall fake_log_body_impl\nadd $0x28, %rsp\nret\n"
    ".text\n");

static void expect(int ok, const char *what)
{
    printf("%s: %s\n", ok ? "ok  " : "FAIL", what);
    fails += !ok;
}

static void run_build(int movie, uint32_t start)
{
    uint8_t flags[16] = {0};
    flags[5] = movie ? 4 : 0;
    Vec *v = (Vec *)(list + 0x78);
    if (!v->begin)
        v->begin = v->end = calloc(64, 16);
    v->end = v->begin;
    ncalls = 0;
    int strings_before = live_strings;
    fake_builder(list, flags, start);
    printf("build movie=%d start=%u -> r12=%llu r15=%llu, %d adder calls:", movie, start,
           (unsigned long long)out_r12, (unsigned long long)out_r15, ncalls);
    for (int i = 0; i < ncalls; i++)
        printf(" %s=%u", calls[i].name, calls[i].index);
    printf("\n");
    for (uint8_t *e = v->begin; e < v->end; e += 16) {
        uint8_t *tile = ((uint8_t **)e)[1], *item = *(uint8_t **)(tile + 0xd0);
        void **vt = *(void ***)item;
        ((void (*)(uint8_t *, void *, uint32_t))vt[4])(item, tile + 8, 0);
        Action a = {0};
        ((Action * (*)(uint8_t *, Action *)) vt[5])(item, &a);
        VmNode *n = *(VmNode **)(item + 0x20);
        printf("  item %-14s title='%s' loc=%d desc='%s' dloc=%d action={%d,'%s'}\n", chars(*(NeStr **)(item + 0x10)),
               n ? n->title : "", n ? n->title_localised : -1, n ? n->description : "", n ? n->description_localised : -1,
               a.type, chars(a.str));
        nestr_release(&a.str);
    }
    build_leak = live_strings - strings_before - ncalls;
    printf("  live strings %d -> %d for %d new item id(s): %d leaked reference(s)\n", strings_before, live_strings,
           ncalls, build_leak);
}

static int movie_built(void)
{
    for (int i = 0; i < ncalls; i++)
        if (!strcmp(calls[i].name, "zenith_button"))
            return 1;
    return 0;
}

static int closes;

static LRESULT CALLBACK wndproc(HWND h, UINT m, WPARAM w, LPARAM l)
{
    closes += m == WM_CLOSE;
    return DefWindowProcW(h, m, w, l);
}

static const char *const setting_commands[] = {
    "swofty_set:example.sample-mod:verbose:1",
    "swofty_set:example.sample-mod:greeting_count:9",
    "swofty_set:example.sample-mod:offset:7",
    "swofty_set:example.sample-mod:flavour:2",
    "swofty_set:example.sample-mod:mismatched:4",
    "swofty_set:community.grid-2026:team_livery:1",
    "swofty_set:example.sample-mod:verbose:0",
    "swofty_enable:community.grid-2026:0",
    "select_pressed",
    NULL,
    "swofty_restart",
};

static uint64_t send_command(const char *name)
{
    NeStr *s = NULL;
    if (name)
        nestr_impl(&s, name);
    uint64_t r = fake_handle_command(0, &s);
    nestr_release(&s);
    return r;
}

static const char *probe_str(int i) { return (const char *)(intptr_t)probe[i]; }

static int reads_are(void (*read)(void), int64_t verbose, int64_t count, int64_t offset, const char *flavour)
{
    read();
    printf("      read: verbose=%lld greeting_count=%lld offset=%lld/1000 flavour='%s' mismatched=%lld undeclared=%lld "
           "audi_name=%lld\n",
           (long long)probe[24], (long long)probe[25], (long long)probe[26], probe_str(27) ? probe_str(27) : "(null)",
           (long long)probe[28], (long long)probe[29], (long long)probe[30]);
    return probe[24] == verbose && probe[25] == count && probe[26] == offset && probe_str(27) &&
           !strcmp(probe_str(27), flavour) && probe[28] == 1 && probe[29] == 42 && probe[30] == 0;
}

unsigned owned_calls;
void *unlock_vt[11];

uint32_t owned_impl(uint32_t key)
{
    owned_calls++;
    return key == 0x7f88c309 ? 1 : 3;
}

uint8_t *unlock_key(uint8_t *item) { return item + 0x18; }

uint32_t fake_owned_state(void *item);

static void test_items(int on)
{
    const char *items_on = getenv("SWOFTY_TEST_ITEMS");
    if (!items_on || !*items_on)
        return;
    static uint8_t items[3][0x130];
    const uint32_t keys[3] = {0xf01d03c7, 0x7f88c309, 0x5b3c2a10};
    unlock_vt[1] = (void *)unlock_key;
    for (int i = 0; i < 3; i++) {
        *(void ***)items[i] = unlock_vt;
        memcpy(items[i] + 0x18 + 0xa0, &keys[i], 4);
    }
    unsigned before = owned_calls;
    uint32_t mod = fake_owned_state(items[0]), again = fake_owned_state(items[0]);
    uint32_t stock_owned = fake_owned_state(items[1]), stock_locked = fake_owned_state(items[2]);
    printf("      owned state: mod %u/%u, stock owned %u, stock locked %u, game check ran %u time(s)\n", mod, again,
           stock_owned, stock_locked, owned_calls - before);
    expect(mod == (on ? 1u : 3u) && again == mod,
           on ? "items: owned_items key 0xf01d03c7 reported owned" : "items: no owned_items, the mod key gets the game's answer");
    expect(stock_owned == 1 && stock_locked == 3, "items: stock keys keep the game's answer (owned 1, locked 3)");
    expect(owned_calls - before == (on ? 2u : 4u), "items: the game's check runs for every key not on the list");
}

// Game_ErsSystem_HandleVehicleNewLap, copied byte for byte from the Steam build (0x1425180d0..0x14251819e), so the
// core's ERS new-lap guard resolves its two sites here and the test can post a vehicle_newlap through it.
__asm__(
    ".section .xtls,\"xr\"\n"
    ".globl fake_ers_newlap\nfake_ers_newlap:\n"
    ".byte 0x48,0x8B,0x41,0x30,0x4C,0x8B,0x41,0x38,0x4C,0x39,0xC0,0x74,0x2C,0x0F,0xB6,0x52,0x10,0x66,0x66,0x66,0x66,0x66,0x66,0x2E\n"
    ".byte 0x0F,0x1F,0x84,0x00,0x00,0x00,0x00,0x00,0x4C,0x8B,0x08,0x4D,0x8B,0x49,0x28,0x41,0x38,0x91,0xE8,0x00,0x00,0x00,0x74,0x0C\n"
    ".byte 0x48,0x83,0xC0,0x08,0x4C,0x39,0xC0,0x75,0xE7,0x31,0xC0,0xC3,0x49,0x8B,0x81,0xA8,0x05,0x00,0x00,0x48,0x85,0xC0,0x74,0xF1\n"
    ".byte 0x80,0x79,0x21,0x00,0x0F,0xB6,0x51,0x22,0x41,0xB8,0x03,0x00,0x00,0x00,0x44,0x0F,0x44,0xC2,0x80,0x79,0x20,0x00,0xB9,0x02\n"
    ".byte 0x00,0x00,0x00,0x41,0x0F,0x44,0xC8,0xC5,0xFA,0x10,0x80,0xC8,0x19,0x00,0x00,0xC5,0xFA,0x11,0x80,0xDC,0x19,0x00,0x00,0xC5\n"
    ".byte 0xF8,0x10,0x80,0xCC,0x19,0x00,0x00,0xC5,0xF8,0x11,0x80,0xE0,0x19,0x00,0x00,0xC5,0xF8,0x57,0xC0,0xC5,0xF8,0x11,0x80,0xC8\n"
    ".byte 0x19,0x00,0x00,0xC7,0x80,0xD8,0x19,0x00,0x00,0x00,0x00,0x00,0x00,0x80,0xF9,0x03,0x75,0x07,0xC5,0xFA,0x10,0x40,0x58,0xEB\n"
    ".byte 0x1A,0xFE,0xC9,0xC5,0xFA,0x10,0x40,0x50,0x80,0xF9,0x01,0x77,0x0E,0x80,0xB8,0x61,0x1A,0x00,0x00,0x00,0x74,0x05,0xC5,0xFA\n"
    ".byte 0x58,0x40,0x54,0xC5,0xFA,0x11,0x80,0xF4,0x19,0x00,0x00,0x31,0xC0,0xC3\n"
    ".text\n");
uint64_t fake_ers_newlap(void *self, void *msg);

static void test_ers_newlap(void)
{
    static uint8_t self[0x40], msg[0x18], blank[0x30], bound[2][0x30], vehicle[2][0x5b0], ers[0x1a70];
    void *entries[4] = {blank, NULL, bound[0], bound[1]};
    for (int i = 0; i < 2; i++) {
        *(void **)(bound[i] + 0x28) = vehicle[i];
        vehicle[i][0xe8] = i ? 24 : 5;
    }
    *(void **)(vehicle[1] + 0x5a8) = ers;
    *(void **)(self + 0x30) = entries;
    *(void **)(self + 0x38) = entries + 4;
    self[0x22] = 1;
    float lap = 7.0f, base = 2.0f, extra = 0.5f, got = 0.0f, prev = 0.0f, now = 1.0f;
    memcpy(ers + 0x19c8, &lap, 4);
    memcpy(ers + 0x50, &base, 4);
    memcpy(ers + 0x54, &extra, 4);
    ers[0x1a61] = 1;
    msg[0x10] = 9;
    uint64_t r = fake_ers_newlap(self, msg);
    memcpy(&now, ers + 0x19c8, 4);
    expect(r == 0 && now == 7.0f, "ers: a vehicle_newlap for a vehicle no participant has walks past the empty slots and "
                                  "changes nothing");
    msg[0x10] = 24;
    r = fake_ers_newlap(self, msg);
    memcpy(&got, ers + 0x19f4, 4);
    memcpy(&prev, ers + 0x19dc, 4);
    memcpy(&now, ers + 0x19c8, 4);
    printf("      vehicle_newlap for 24: returned %llu, lap budget %.2f, last lap %.2f, this lap %.2f\n",
           (unsigned long long)r, got, prev, now);
    expect(r == 0 && got == 2.5f && prev == 7.0f && now == 0.0f,
           "ers: past a participant without a vehicle and a null participant, the car's lap energy counters reset");
}

// Game_TimeTrialLeaderboard_SelectBoard's board search and threshold read, copied byte for byte from the Steam build
// (0x142fce988..0x142fcea17, the one direct call to Ne_Compare_StringNoCase goes to fake_lb_compare) with the cleanup
// entry 0x142fced92..0x142fced9e, so core's time trial leaderboard guard resolves its two sites here. The frame is
// reduced to what those instructions use: rdi = the screen model, [rsp+0x38] = the board key. The return value is
// esi (2 when a board was found, 0 from the guard) in the second byte and the rank index in the first.
__asm__(
    ".section .xtls,\"xr\"\n"
    ".globl fake_lb_select\n"
    ".def fake_lb_select; .scl 2; .type 32; .endef\n"
    ".seh_proc fake_lb_select\n"
    "fake_lb_select:\n"
    "push %rbx\n.seh_pushreg %rbx\n"
    "push %rdi\n.seh_pushreg %rdi\n"
    "push %rsi\n.seh_pushreg %rsi\n"
    "push %rbp\n.seh_pushreg %rbp\n"
    "push %r14\n.seh_pushreg %r14\n"
    "push %r15\n.seh_pushreg %r15\n"
    "sub $0x88,%rsp\n.seh_stackalloc 0x88\n"
    ".seh_endprologue\n"
    "mov %rcx,%rbx\n"
    "mov %rcx,%rdi\n"
    "mov $2,%esi\n"
    "lea fake_lb_key(%rip),%rax\n"
    "mov %rax,0x38(%rsp)\n"
    ".byte 0x48,0x8B,0x83,0x78,0x04,0x00,0x00,0x48,0x8B,0x58,0x10,0x31,0xED,0x48,0x85,0xDB,0x74,0x47,0x4C,0x8B,0x73,0x18,0x4C,0x3B\n"
    ".byte 0x73,0x20,0x74,0x3D,0x4C,0x8B,0x7C,0x24,0x38,0x49,0x83,0xC6,0x08,0xEB,0x14,0x90,0x49,0x8B,0x4E,0xF8,0x4C,0x3B,0x73,0x20\n"
    ".byte 0x74,0x29,0x49,0x83,0xC6,0x08,0x48,0x85,0xC9,0x75,0x20,0x49,0x8B,0x46,0xF8,0x48,0x8B,0x48,0x68,0x4C,0x89,0xFA\n"
    "call fake_lb_compare\n"
    ".byte 0x85,0xC0,0x74,0xD9,0x31,0xC9,0x4C,0x3B,0x73,0x20,0x75,0xDB,0xEB,0x02,0x31,0xC9,0x48,0x8B,0x81,0x98,0x00,0x00,0x00,0x48\n"
    ".byte 0x8B,0x91,0xA0,0x00,0x00,0x00,0x48,0x39,0xD0,0x74,0x1A,0x4C,0x8B,0x81,0x88,0x00,0x00,0x00,0x31,0xED,0x90,0x4C,0x39,0x00\n"
    ".byte 0x77,0x0B,0xFF,0xC5,0x48,0x83,0xC0,0x08,0x48,0x39,0xD0,0x75,0xF0,0x0F,0xB6,0x87,0x4A,0x0E,0x00,0x00\n"
    "jmp 1f\n"
    ".byte 0x48,0x8B,0x44,0x24,0x60,0x48,0x89,0x87,0x78,0x08,0x00,0x00,0x48,0x8D,0x3D,0xA3,0x40,0x64,0x02\n"
    "1:\n"
    "mov %esi,%eax\n"
    "shl $8,%eax\n"
    "add %ebp,%eax\n"
    "add $0x88,%rsp\n"
    "pop %r15\n"
    "pop %r14\n"
    "pop %rbp\n"
    "pop %rsi\n"
    "pop %rdi\n"
    "pop %rbx\n"
    "ret\n"
    ".seh_endproc\n"
    ".section .drectve\n"
    ".ascii \" -export:fake_lb_select\"\n"
    ".text\n");
uint64_t fake_lb_select(void *screen);

char fake_lb_key[16] = "LBV0105dry";

__attribute__((used, noinline)) int fake_lb_compare(const char *a, const char *b)
{
    return _stricmp(a, b);
}

static void lb_screen(uint8_t *screen, void **boards, int count)
{
    static uint8_t reader[0x480], list[0x40];
    *(void **)(screen + 0x478) = reader;
    *(void **)(reader + 0x10) = list;
    *(void ***)(list + 0x18) = boards;
    *(void ***)(list + 0x20) = boards + count;
}

static void test_leaderboard_guard(void)
{
    static uint8_t screen[0x900], other[0xb0], wanted[0xb0];
    static uint64_t thresholds[3] = {10, 20, 30};
    static char other_name[] = "LBV0107wet", wanted_name[] = "lbv0105DRY";
    void *boards[2] = {other, wanted};
    *(char **)(other + 0x68) = other_name;
    *(char **)(wanted + 0x68) = wanted_name;
    *(uint64_t *)(wanted + 0x88) = 25;
    *(uint64_t **)(wanted + 0x98) = thresholds;
    *(uint64_t **)(wanted + 0xa0) = thresholds + 3;
    lb_screen(screen, boards, 2);
    uint64_t r = fake_lb_select(screen);
    printf("      leaderboard: board found, returned 0x%llx\n", (unsigned long long)r);
    expect(r == 0x202, "leaderboard: a board whose name matches the key runs the game's code: two thresholds passed");
    lb_screen(screen, boards, 1);
    r = fake_lb_select(screen);
    printf("      leaderboard: no board for the key, returned 0x%llx\n", (unsigned long long)r);
    expect(r == 0, "leaderboard: no board carries the key, the guard returns 0 through the game's cleanup");
    lb_screen(screen, boards, 0);
    r = fake_lb_select(screen);
    expect(r == 0, "leaderboard: an empty board list, the guard returns 0 through the game's cleanup");
}

// Game_Load_MyTeamVehicleBases's loop over the saved My Team vehicle bases (0x142507ad0..0x1425098ef), with the byte
// runs core's My Team vehicle base guard resolves copied byte for byte from the Steam build: the row lookup's return
// into the AI config call (0x1425082f5..0x142508305) and the step after a base is stored (0x1425098cd..0x1425098ef).
// The save reads are replaced: the list is {u16 count +0x10, struct mtv_base *bases +0x18}. The frame keeps the
// game's slots: [rsp+0x30] the list, [rsp+0x38] the base index, [rsp+0x44] the saved stat_id, [rsp+0xb8] the
// file_name. Like the game, rdi and r12 are reused before the AI config call and restored before the store.
// fake_mtv_find stands in for Gdb_Find_RowByLabel, fake_mtv_ai for Sess_Load_VehicleAiConfig and fake_mtv_store for
// Sess_VehicleTable_EmplaceAt. It returns the number of bases stored.
__asm__(
    ".section .xtls,\"xr\"\n"
    ".globl fake_mtv_load\n"
    ".def fake_mtv_load; .scl 2; .type 32; .endef\n"
    ".seh_proc fake_mtv_load\n"
    "fake_mtv_load:\n"
    "push %rbx\n.seh_pushreg %rbx\n"
    "push %rbp\n.seh_pushreg %rbp\n"
    "push %rdi\n.seh_pushreg %rdi\n"
    "push %rsi\n.seh_pushreg %rsi\n"
    "push %r12\n.seh_pushreg %r12\n"
    "push %r13\n.seh_pushreg %r13\n"
    "push %r14\n.seh_pushreg %r14\n"
    "push %r15\n.seh_pushreg %r15\n"
    "sub $0xc8,%rsp\n.seh_stackalloc 0xc8\n"
    ".seh_endprologue\n"
    "mov %rcx,%rdi\n"
    "mov %rdi,0x30(%rsp)\n"
    "xor %r12d,%r12d\n"
    "cmpw $0,0x10(%rdi)\n"
    "je 9f\n"
    "1:\n"
    "mov %r12,0x38(%rsp)\n"
    "mov 0x18(%rdi),%rax\n"
    "mov %r12,%rcx\n"
    "shl $4,%rcx\n"
    "add %rcx,%rax\n"
    "movzwl (%rax),%ebp\n"
    "mov %ebp,0x44(%rsp)\n"
    "mov 8(%rax),%rcx\n"
    "mov %rcx,0xb8(%rsp)\n"
    "xor %ebp,%ebp\n"
    "xor %edi,%edi\n"
    "mov 0xb8(%rsp),%r8\n"
    "lea fake_mtv_row(%rip),%r12\n"
    "mov %r12,%rdx\n"
    "xor %ecx,%ecx\n"
    "call fake_mtv_find\n"
    ".byte 0x48,0x8D,0x8C,0x24,0x50,0x1A,0x00,0x00,0x4C,0x89,0xE2\n"
    "call fake_mtv_ai\n"
    "mov 0x30(%rsp),%rdi\n"
    "mov 0x38(%rsp),%r12\n"
    "mov %r12,%rcx\n"
    ".byte 0x48,0x8D,0x94,0x24,0x50,0x0D,0x00,0x00\n"
    "call fake_mtv_store\n"
    ".byte 0x49,0xFF,0xC4,0x0F,0xB7,0x47,0x10,0x49,0x39,0xC4,0x0F,0x82\n"
    ".long 1b - (. + 4)\n"
    ".byte 0xE9\n"
    ".long 9f - (. + 4)\n"
    "9:\n"
    "mov fake_mtv_stored(%rip),%eax\n"
    "add $0xc8,%rsp\n"
    "pop %r15\n"
    "pop %r14\n"
    "pop %r13\n"
    "pop %r12\n"
    "pop %rsi\n"
    "pop %rdi\n"
    "pop %rbp\n"
    "pop %rbx\n"
    "ret\n"
    ".seh_endproc\n"
    ".section .drectve\n"
    ".ascii \" -export:fake_mtv_load -export:fake_mtv_ai\"\n"
    ".text\n");
uint32_t fake_mtv_load(void *list);

struct mtv_base {
    uint16_t stat_id;
    uint8_t pad[6];
    const char *file_name;
};

uint8_t fake_mtv_row[0x210];
uint32_t fake_mtv_stored;
static const char *const mtv_rows[] = {"myteam_ferrari", "myteam_mercedes", "myteam_honda", "myteam_renault"};
static struct mtv_base mtv_bases[] = {{0x4001, {0}, "myteam_ferrari"},
                                      {0x4000, {0}, "myteam_mes_porsche_2026"},
                                      {0x4002, {0}, "myteam_mercedes"},
                                      {0x4005, {0}, "myteam_mes_porsche_2026"}};
static uint8_t mtv_list[0x20], mtv_record[0x48];
static uint64_t mtv_miss = 0x08000f000800000eull, mtv_hit;
static uint16_t mtv_stored_ids[8];
static int mtv_ai_calls;

__attribute__((used, noinline)) void fake_mtv_find(void *cursor, uint8_t *row, const char *label)
{
    (void)cursor;
    memset(row, 0, sizeof fake_mtv_row);
    for (int i = 0; i < (int)(sizeof mtv_rows / sizeof *mtv_rows); i++)
        if (!strcmp(label, mtv_rows[i])) {
            snprintf((char *)row, 0x100, "entrants.vehicles.vehicle_%d", i);
            memcpy(row + 0x100, "vehicle", 8);
            *(int32_t *)(row + 0x200) = i;
            row[0x204] = 1;
            return;
        }
    memcpy(row, "Invalid", 8);
    memcpy(row + 0x100, "Invalid", 8);
    *(int32_t *)(row + 0x200) = -1;
}

static int mtv_keep_rows, mtv_last_keep;

__attribute__((used, noinline)) void fake_mtv_ai(void *out, uint8_t *row)
{
    (void)out;
    mtv_last_keep = !strcmp((const char *)row, "entrants.vehicles") && !strncmp((const char *)row + 0x100, "vehicle_", 8);
    if (mtv_last_keep)
        mtv_keep_rows++;
    const volatile uint64_t *slot = row[0x204] == 1 ? &mtv_hit : &mtv_miss;
    uint64_t rec = *slot;
    if (rec && *(const volatile uint64_t *)(rec + 0x40) != 1)
        mtv_ai_calls++;
}

static uint8_t mtv_cars[4][0xd10];

__attribute__((used, noinline)) uint8_t *fake_mtv_store(uint64_t index, void *record)
{
    (void)record;
    if (fake_mtv_stored < sizeof mtv_stored_ids / sizeof *mtv_stored_ids)
        mtv_stored_ids[fake_mtv_stored] = mtv_bases[index].stat_id & 0x3fff;
    fake_mtv_stored++;
    if (index >= 4)
        return NULL;
    uint8_t *car = mtv_cars[index];
    memset(car, 0, sizeof mtv_cars[index]);
    *(uint32_t *)car = (uint32_t)(mtv_bases[index].stat_id & 0x3fff) << 16 | 1;
    snprintf((char *)car + 0x70, 0x100, "%s", mtv_last_keep ? "myteam_mercedes_2026" : mtv_bases[index].file_name);
    snprintf((char *)car + 0x2a0, 0x100, "saved display %u", (unsigned)index);
    car[0xce9] = (uint8_t)(0x15 + index);
    *(uint32_t *)(car + 0xcf2) = 0x00a40000u + (uint32_t)index;
    return car;
}

// Game_Save_MyTeamVehicleBases's read of the car being written (0x1425070b3..0x1425070c9), copied byte for byte from the
// Steam build, so core's My Team vehicle keep resolves site_Game_MyTeamVehicleBases_SaveRecord here: r14 the car,
// [rsp+0x38] the child bag call's error (0). fake_mtv_write reads the entry's fields through r14 as the writer does.
__asm__(
    ".section .xtls,\"xr\"\n"
    ".globl fake_mtv_save\n"
    ".def fake_mtv_save; .scl 2; .type 32; .endef\n"
    ".seh_proc fake_mtv_save\n"
    "fake_mtv_save:\n"
    "push %r14\n.seh_pushreg %r14\n"
    "push %rbp\n.seh_pushreg %rbp\n"
    "sub $0x48,%rsp\n.seh_stackalloc 0x48\n"
    ".seh_endprologue\n"
    "mov %rcx,%r14\n"
    "movl $0,0x38(%rsp)\n"
    ".byte 0x8B,0x6C,0x24,0x38,0x85,0xED,0x0F,0x85\n"
    ".long 9f - (. + 4)\n"
    ".byte 0x41,0x8B,0x06,0xC1,0xE0,0x0E,0x66,0x41,0x0B,0x46,0x02\n"
    "mov %r14,%rcx\n"
    "call fake_mtv_write\n"
    "9:\n"
    "add $0x48,%rsp\n"
    "pop %rbp\n"
    "pop %r14\n"
    "ret\n"
    ".seh_endproc\n"
    ".section .drectve\n"
    ".ascii \" -export:fake_mtv_save\"\n"
    ".text\n");
void fake_mtv_save(uint8_t *car);

static char mtv_written_name[0x100];
static uint8_t mtv_written_engine;
static uint32_t mtv_written_base, mtv_written_handle;

__attribute__((used, noinline)) void fake_mtv_write(const uint8_t *car)
{
    mtv_written_handle = *(const uint32_t *)car;
    snprintf(mtv_written_name, sizeof mtv_written_name, "%s", (const char *)car + 0x70);
    mtv_written_engine = car[0xce9];
    mtv_written_base = *(const uint32_t *)(car + 0xcf2);
}

static void test_myteam_vehicle_guard(int on)
{
    SetErrorMode(SEM_NOGPFAULTERRORBOX);
    mtv_hit = (uint64_t)(uintptr_t)mtv_record;
    *(uint64_t *)(mtv_record + 0x40) = 2;
    *(uint16_t *)(mtv_list + 0x10) = (uint16_t)(sizeof mtv_bases / sizeof *mtv_bases);
    *(struct mtv_base **)(mtv_list + 0x18) = mtv_bases;
    fake_mtv_stored = 0;
    mtv_ai_calls = 0;
    printf("      myteam vehicle %s: loading 4 saved bases, 2 of them on vehicle myteam_mes_porsche_2026, which has no row\n",
           on ? "on" : "off");
    fflush(stdout);
    uint32_t n = fake_mtv_load(mtv_list);
    printf("      myteam vehicle %s: the load returned, %u bases stored (stat ids %u, %u), %d AI configs built\n", on ? "on" : "off",
           n, mtv_stored_ids[0], mtv_stored_ids[1], mtv_ai_calls);
    fflush(stdout);
    expect(n == 2 && mtv_stored_ids[0] == 1 && mtv_stored_ids[1] == 2,
           "myteam vehicle: the bases on myteam_ferrari and myteam_mercedes are stored, the two on the missing row are skipped");
    expect(mtv_ai_calls == 2, "myteam vehicle: only the bases with a row build an AI config");
}

// Career_ScheduleComponent_Load's custom_track_list loop (0x141272c60..0x141272d3f) and the calendar read of
// Career_ScheduleComponent_Save (0x141272150..0x141272161), with the byte runs core's career calendar track list guard
// resolves copied byte for byte from the Steam build. The save bag reads are replaced: the stored array is
// {u16 count +0x10, u8 *ids +0x18}. rsi = the schedule component, rdi = the array, r14 = component + 0x2a8; the push
// goes to fake_tl_push. Both return the calendar's size in bytes.
__asm__(
    ".section .xtls,\"xr\"\n"
    ".globl fake_tl_load\n"
    ".def fake_tl_load; .scl 2; .type 32; .endef\n"
    ".seh_proc fake_tl_load\n"
    "fake_tl_load:\n"
    "push %rbx\n.seh_pushreg %rbx\n"
    "push %rbp\n.seh_pushreg %rbp\n"
    "push %rdi\n.seh_pushreg %rdi\n"
    "push %rsi\n.seh_pushreg %rsi\n"
    "push %r12\n.seh_pushreg %r12\n"
    "push %r13\n.seh_pushreg %r13\n"
    "push %r14\n.seh_pushreg %r14\n"
    "push %r15\n.seh_pushreg %r15\n"
    "sub $0x48,%rsp\n.seh_stackalloc 0x48\n"
    ".seh_endprologue\n"
    "mov %rcx,%rsi\n"
    "mov %rdx,%rdi\n"
    "cmpw $0,0x10(%rdi)\n"
    "je 9f\n"
    "lea 0x2a8(%rsi),%r14\n"
    "xor %r15d,%r15d\n"
    "jmp 2f\n"
    "1:\n"
    ".byte 0x49,0xFF,0xC7,0x0F,0xB7,0x47,0x10,0x49,0x39,0xC7,0x4C,0x8B,0x6C,0x24,0x38,0x0F,0x83\n"
    ".long 9f - (. + 4)\n"
    "2:\n"
    "mov 0x18(%rdi),%rax\n"
    "add %r15,%rax\n"
    "mov 0x278(%rsi),%rcx\n"
    "mov 0xf58(%rcx),%rcx\n"
    "mov 0x138(%rcx),%rcx\n"
    "mov 0x8(%rcx),%rbx\n"
    "mov 0x10(%rcx),%rbp\n"
    "cmp %rbp,%rbx\n"
    "je 1b\n"
    "movzbl (%rax),%r13d\n"
    "jmp 4f\n"
    "3:\n"
    ".byte 0x48,0x83,0xC3,0x28,0x48,0x39,0xEB,0x0F,0x84\n"
    ".long 1b - (. + 4)\n"
    "4:\n"
    ".byte 0x44,0x38,0x6B,0x08,0x75,0xED,0x4C,0x89,0xF1,0x48,0x89,0xDA\n"
    "call fake_tl_push\n"
    ".byte 0xEB,0xE0\n"
    "9:\n"
    "mov 0x2b8(%rsi),%rax\n"
    "sub 0x2b0(%rsi),%rax\n"
    "add $0x48,%rsp\n"
    "pop %r15\n"
    "pop %r14\n"
    "pop %r13\n"
    "pop %r12\n"
    "pop %rsi\n"
    "pop %rdi\n"
    "pop %rbp\n"
    "pop %rbx\n"
    "ret\n"
    ".seh_endproc\n"
    ".globl fake_tl_save\n"
    ".def fake_tl_save; .scl 2; .type 32; .endef\n"
    ".seh_proc fake_tl_save\n"
    "fake_tl_save:\n"
    "push %rsi\n.seh_pushreg %rsi\n"
    "push %rdi\n.seh_pushreg %rdi\n"
    "push %r12\n.seh_pushreg %r12\n"
    "push %r14\n.seh_pushreg %r14\n"
    "sub $0x28,%rsp\n.seh_stackalloc 0x28\n"
    ".seh_endprologue\n"
    "mov %rcx,%r14\n"
    "lea fake_tl_bag(%rip),%rdi\n"
    "xor %esi,%esi\n"
    ".byte 0x66,0x89,0x77,0x12,0x4D,0x8B,0xA6,0xB0,0x02,0x00,0x00,0x4D,0x39,0xA6,0xB8,0x02,0x00,0x00\n"
    "mov 0x2b8(%r14),%rax\n"
    "sub %r12,%rax\n"
    "add $0x28,%rsp\n"
    "pop %r14\n"
    "pop %r12\n"
    "pop %rdi\n"
    "pop %rsi\n"
    "ret\n"
    ".seh_endproc\n"
    ".section .drectve\n"
    ".ascii \" -export:fake_tl_load -export:fake_tl_save\"\n"
    ".text\n");
uint64_t fake_tl_load(void *component, void *array);
uint64_t fake_tl_save(void *component);

uint8_t fake_tl_bag[0x20];

enum { TL_ENTRY = 0x28, TL_SEASONS = 10, TL_IDS = 24, TL_CAP = 6000 };
static uint8_t tl_rows[TL_SEASONS * TL_IDS + 1][TL_ENTRY];
static uint8_t tl_component[0x440], tl_career[0x1000], tl_events[0x140], tl_list[0x18], tl_array[0x20];
static uint8_t tl_calendar[TL_CAP * TL_ENTRY];
static uint8_t tl_ids[65536];
static int tl_full;

__attribute__((used, noinline)) void fake_tl_push(uint8_t *vec, const uint8_t *row)
{
    uint8_t **end = (uint8_t **)(vec + 0x10);
    if (*end >= tl_calendar + sizeof tl_calendar) {
        tl_full = 1;
        return;
    }
    memcpy(*end, row, TL_ENTRY);
    *end += TL_ENTRY;
}

static void tl_setup(void)
{
    for (int s = 0; s < TL_SEASONS; s++)
        for (int id = 0; id < TL_IDS; id++) {
            uint8_t *row = tl_rows[s * TL_IDS + id];
            row[8] = (uint8_t)id;
            row[0x10] = (uint8_t)s;
        }
    tl_rows[TL_SEASONS * TL_IDS][8] = 40;
    *(uint8_t **)(tl_component + 0x278) = tl_career;
    *(uint8_t **)(tl_career + 0xf58) = tl_events;
    *(uint8_t **)(tl_events + 0x138) = tl_list;
    *(uint8_t **)(tl_list + 8) = tl_rows[0];
    *(uint8_t **)(tl_list + 0x10) = tl_rows[0] + sizeof tl_rows;
    *(uint8_t **)(tl_component + 0x2b0) = tl_calendar;
    *(uint8_t **)(tl_component + 0x2b8) = tl_calendar;
    *(uint64_t *)(tl_component + 0x2c0) = TL_CAP;
    *(uint32_t *)(tl_component + 0x2c8) = 5;
}

static size_t tl_load(size_t stored)
{
    *(uint8_t **)(tl_component + 0x2b8) = tl_calendar;
    *(uint16_t *)(tl_array + 0x10) = (uint16_t)stored;
    *(uint8_t **)(tl_array + 0x18) = tl_ids;
    return fake_tl_load(tl_component, tl_array) / TL_ENTRY;
}

static size_t tl_save(void)
{
    size_t n = fake_tl_save(tl_component) / TL_ENTRY;
    for (size_t i = 0; i < n && i < sizeof tl_ids; i++)
        tl_ids[i] = tl_calendar[i * TL_ENTRY + 8];
    return n;
}

static int tl_rows_once(size_t n)
{
    for (size_t i = 0; i < n; i++)
        for (size_t j = i + 1; j < n; j++)
            if (!memcmp(tl_calendar + i * TL_ENTRY, tl_calendar + j * TL_ENTRY, TL_ENTRY))
                return 0;
    return 1;
}

static void test_track_list_guard(int on)
{
    static const uint8_t calendar[5] = {0, 3, 7, 12, 23};
    tl_setup();
    memcpy(tl_ids, calendar, sizeof calendar);
    size_t stored = sizeof calendar, sizes[5] = {0};
    for (int cycle = 0; cycle < (on ? 5 : 3); cycle++) {
        sizes[cycle] = tl_load(stored);
        stored = tl_save();
        printf("      track list %s: cycle %d loaded %zu calendar events, saved %zu\n", on ? "on" : "off", cycle + 1, sizes[cycle], stored);
    }
    if (!on) {
        expect(sizes[0] == 50 && sizes[1] == 500 && sizes[2] == 5000 && !tl_full,
               "track list off: 10 seasons sharing ids multiply the calendar 10x per save and load (50, 500, 5000)");
        return;
    }
    expect(sizes[0] == 5 && sizes[1] == 5 && sizes[2] == 5 && sizes[3] == 5 && sizes[4] == 5,
           "track list on: 10 seasons sharing ids keep the calendar at its 5 events over 5 save and load cycles");
    expect(tl_rows_once(sizes[4]) && tl_calendar[0x10] == 0, "track list on: each entry loads one event, the first season's row");
    static const uint8_t twice[4] = {0, 40, 40, 7};
    memcpy(tl_ids, twice, sizeof twice);
    *(uint32_t *)(tl_component + 0x2c8) = 4;
    size_t doubled = tl_load(4);
    printf("      track list on: a calendar with a track twice (one event row) loaded %zu calendar events\n", doubled);
    expect(doubled == 4 && tl_calendar[1 * TL_ENTRY + 8] == 40 && tl_calendar[2 * TL_ENTRY + 8] == 40,
           "track list on: a track twice in one season with one event row loads both races");
    expect(tl_save() == 4 && tl_load(4) == 4, "track list on: the double header saves and loads with both races");
    *(uint32_t *)(tl_component + 0x2c8) = 5;
    for (size_t i = 0; i < 65535; i++)
        tl_ids[i] = calendar[i * 5 / 65535];
    size_t healed = tl_load(65535);
    printf("      track list on: a 65535-entry custom_track_list loaded %zu calendar events\n", healed);
    expect(healed == 5 && tl_rows_once(healed) && !tl_full, "track list on: a blown-up save (65,535 entries) loads back to the season's 5 events");
    for (int i = 0; i < 5; i++)
        if (tl_calendar[i * TL_ENTRY + 8] != calendar[i])
            healed = 0;
    expect(healed == 5, "track list on: the healed calendar keeps the season's order");
    expect(tl_save() == 5 && tl_load(5) == 5, "track list on: the healed calendar saves and loads unchanged");
}

// Sess_Build_SessionVehicleClassLists's first walk of the session entrants (0x14098f6f0..0x14098f736), copied byte for
// byte from the Steam build, so core's My Team vehicle heal resolves site_Sess_SessionEnter_VehicleLookup here: rbx the
// entrants system, rsi the entrant, r12 the end, rdi the list holder +0x18 ([rdi+8] the first entrant), r13 the class
// table. fake_sv_find stands in for Sess_ResolveEntityHandle and fake_sv_class for 0x142506d20. After the walk,
// fake_sv_classify reads [vehicle+0xcea] of every entrant without a zero check, as 0x142458710 does at 0x142458792.
// fake_mtv_replace carries Ent_EntrantsSystem_ReplaceVehicle's prologue so core resolves it, and stores a copy of the
// source vehicle in the kind 1 table the way the game's record build does (handle at +0, the data from +0x10).
__asm__(
    ".section .xtls,\"xr\"\n"
    ".globl fake_sv_classes\n"
    ".def fake_sv_classes; .scl 2; .type 32; .endef\n"
    ".seh_proc fake_sv_classes\n"
    "fake_sv_classes:\n"
    "push %rbx\n.seh_pushreg %rbx\n"
    "push %rbp\n.seh_pushreg %rbp\n"
    "push %rdi\n.seh_pushreg %rdi\n"
    "push %rsi\n.seh_pushreg %rsi\n"
    "push %r12\n.seh_pushreg %r12\n"
    "push %r13\n.seh_pushreg %r13\n"
    "push %r14\n.seh_pushreg %r14\n"
    "push %r15\n.seh_pushreg %r15\n"
    "sub $0x88,%rsp\n.seh_stackalloc 0x88\n"
    ".seh_endprologue\n"
    "mov %rcx,%rbx\n"
    "mov %rdx,%rdi\n"
    "mov 0x20(%rdi),%rsi\n"
    "mov 0x28(%rdi),%r12\n"
    "cmp %r12,%rsi\n"
    "je 2f\n"
    "add $0x18,%rdi\n"
    "lea fake_sv_class_table(%rip),%r13\n"
    "lea 0x30(%rsp),%r14\n"
    "lea 0x58(%rsp),%r15\n"
    "jmp 1f\n"
    "0:\n"
    ".byte 0x89,0x44,0x24,0x58,0x4C,0x89,0xF1,0x4C,0x89,0xFA\n"
    "call fake_sv_class\n"
    ".byte 0x48,0x81,0xC6,0x88,0x00,0x00,0x00,0x4C,0x39,0xE6,0x74,0x2B\n"
    "1:\n"
    ".byte 0x48,0x8D,0x56,0x5C,0x48,0x89,0xD9\n"
    "call fake_sv_find\n"
    ".byte 0x48,0x85,0xC0,0x74,0xE3,0x48,0x0F,0xBE,0x88,0xEA,0x0C,0x00,0x00,0xB8,0xFF,0xFF,0xFF,0xFF,0x48,0x83,0xF9,0x03,0x77,0xC1\n"
    ".byte 0x41,0x8B,0x44,0x8D,0x00,0xEB,0xBA\n"
    "mov %rbx,%rcx\n"
    "lea -0x18(%rdi),%rdx\n"
    "call fake_sv_classify\n"
    "2:\n"
    "add $0x88,%rsp\n"
    "pop %r15\n"
    "pop %r14\n"
    "pop %r13\n"
    "pop %r12\n"
    "pop %rsi\n"
    "pop %rdi\n"
    "pop %rbp\n"
    "pop %rbx\n"
    "ret\n"
    ".seh_endproc\n"
    ".globl fake_mtv_replace\n"
    ".def fake_mtv_replace; .scl 2; .type 32; .endef\n"
    ".seh_proc fake_mtv_replace\n"
    "fake_mtv_replace:\n"
    "push %r15\n.seh_pushreg %r15\n"
    "push %r14\n.seh_pushreg %r14\n"
    "push %r13\n.seh_pushreg %r13\n"
    "push %r12\n.seh_pushreg %r12\n"
    "push %rsi\n.seh_pushreg %rsi\n"
    "push %rdi\n.seh_pushreg %rdi\n"
    "push %rbp\n.seh_pushreg %rbp\n"
    "push %rbx\n.seh_pushreg %rbx\n"
    "sub $0xd38,%rsp\n.seh_stackalloc 0xd38\n"
    ".seh_endprologue\n"
    "mov %edx,0x28(%rsp)\n"
    "call fake_mtv_replace_impl\n"
    "add $0xd38,%rsp\n"
    "pop %rbx\n"
    "pop %rbp\n"
    "pop %rdi\n"
    "pop %rsi\n"
    "pop %r12\n"
    "pop %r13\n"
    "pop %r14\n"
    "pop %r15\n"
    "ret\n"
    ".seh_endproc\n"
    ".section .drectve\n"
    ".ascii \" -export:fake_sv_classes -export:fake_mtv_replace\"\n"
    ".text\n");
void fake_sv_classes(void *entrants, void *holder);

#define SV_CAR 0xd10
#define SV_STOCK 6
#define SV_CUSTOM 4
static uint8_t sv_cars[SV_STOCK][SV_CAR], sv_custom_cars[SV_CUSTOM][SV_CAR];
static uint64_t sv_stock_slots[SV_STOCK][2], sv_custom_slots[SV_CUSTOM][2];
static uint64_t sv_stock_table[3], sv_custom_table[3];
static uint8_t sv_entrants_system[0x360];
static uint8_t sv_list[4][0x88];
static uint64_t sv_holder[6];
static const char *const sv_labels[SV_STOCK] = {"ferrari_2026", "mercedes_2026", "myteam_mercedes", "myteam_mes_porsche_2026",
                                               "myteam_honda_2026", "myteam_mercedes_2026"};
const int32_t fake_sv_class_table[4] = {0, 1, 2, 3};
static int sv_classes, sv_classified, sv_replaced;

__attribute__((used, noinline)) uint64_t fake_sv_find(const uint8_t *entrants, const uint8_t *handle)
{
    uint8_t kind = handle[0];
    uint16_t id = *(const uint16_t *)(handle + 2);
    if (kind >= 3)
        return 0;
    const uint64_t *table = *(uint64_t *const *)(entrants + 0x328 + kind * 0x10);
    if (!table || id >= (table[2] - table[1]) / 16)
        return 0;
    return ((const uint64_t *)(uintptr_t)table[1])[id * 2 + 1];
}

__attribute__((used, noinline)) void fake_sv_class(void *out, const int32_t *cls)
{
    (void)out;
    (void)cls;
    sv_classes++;
}

__attribute__((used, noinline)) void fake_sv_classify(const uint8_t *entrants, const uint64_t *holder)
{
    for (const uint8_t *e = (const uint8_t *)(uintptr_t)holder[4]; e != (const uint8_t *)(uintptr_t)holder[5]; e += 0x88) {
        uint64_t v = fake_sv_find(entrants, e + 0x5c);
        volatile int8_t cls = *(volatile const int8_t *)(uintptr_t)(v + 0xcea);
        (void)cls;
        sv_classified++;
    }
}

__attribute__((used, noinline)) int fake_mtv_replace_impl(uint8_t *entrants, uint32_t handle, const uint8_t *vehicle)
{
    uint8_t kind = handle & 0xff;
    uint16_t id = handle >> 16;
    if (!kind)
        return 5;
    uint64_t *table = *(uint64_t **)(entrants + 0x328 + kind * 0x10);
    if (!table || kind != 1 || id >= SV_CUSTOM)
        return 1;
    uint8_t *car = sv_custom_cars[id];
    memset(car, 0, SV_CAR);
    *(uint32_t *)car = handle;
    memcpy(car + 0x10, vehicle, SV_CAR - 0x10);
    uint64_t *slots = (uint64_t *)(uintptr_t)table[1];
    size_t n = (table[2] - table[1]) / 16;
    if (id >= n)
        table[2] = table[1] + (uint64_t)(id + 1) * 16;
    slots[id * 2] = (uint64_t)(uintptr_t)table;
    slots[id * 2 + 1] = (uint64_t)(uintptr_t)car;
    sv_replaced++;
    return id >= n ? 2 : 0;
}

static void test_myteam_vehicle_heal(int on)
{
    SetErrorMode(SEM_NOGPFAULTERRORBOX);
    memset(sv_custom_slots, 0, sizeof sv_custom_slots);
    for (int i = 0; i < SV_STOCK; i++) {
        memset(sv_cars[i], 0, SV_CAR);
        *(uint32_t *)sv_cars[i] = (uint32_t)i << 16;
        snprintf((char *)sv_cars[i] + 0x70, 0x100, "%s", sv_labels[i]);
        sv_cars[i][0xcea] = 1;
        sv_stock_slots[i][0] = (uint64_t)(uintptr_t)sv_stock_table;
        sv_stock_slots[i][1] = (uint64_t)(uintptr_t)sv_cars[i];
    }
    sv_stock_table[1] = (uint64_t)(uintptr_t)sv_stock_slots;
    sv_stock_table[2] = sv_stock_table[1] + sizeof sv_stock_slots;
    sv_custom_table[1] = (uint64_t)(uintptr_t)sv_custom_slots;
    sv_custom_table[2] = sv_custom_table[1] + 16;
    *(uint64_t **)(sv_entrants_system + 0x328) = sv_stock_table;
    *(uint64_t **)(sv_entrants_system + 0x338) = sv_custom_table;
    uint32_t handles[4] = {0u << 16, 1u << 16, 2u << 16 | 1, 2u << 16 | 1};
    for (int i = 0; i < 4; i++)
        *(uint32_t *)(sv_list[i] + 0x5c) = handles[i];
    sv_holder[4] = (uint64_t)(uintptr_t)sv_list[0];
    sv_holder[5] = (uint64_t)(uintptr_t)sv_list[4];
    sv_classes = sv_classified = sv_replaced = 0;
    printf("      myteam heal %s: entering a session whose My Team car (custom vehicle 2) the save lost\n", on ? "on" : "off");
    fflush(stdout);
    fake_sv_classes(sv_entrants_system, sv_holder);
    const char *label = (const char *)sv_custom_cars[2] + 0x70;
    printf("      myteam heal %s: the session entered, %d classes, %d entrants classified, %d rebuilt, custom vehicle 2 is '%s'\n",
           on ? "on" : "off", sv_classes, sv_classified, sv_replaced, label);
    fflush(stdout);
    expect(sv_replaced == 1, "myteam heal: the lost car is rebuilt once for both of its entrants");
    expect(!strcmp(label, "myteam_mercedes_2026"), "myteam heal: a 2026 session rebuilds it from myteam_mercedes_2026");
    expect(sv_classes == 4 && sv_classified == 4, "myteam heal: every entrant gets its class");
}

static void test_myteam_vehicle_keep(int on)
{
    SetErrorMode(SEM_NOGPFAULTERRORBOX);
    mtv_hit = (uint64_t)(uintptr_t)mtv_record;
    *(uint64_t *)(mtv_record + 0x40) = 2;
    *(uint16_t *)(mtv_list + 0x10) = (uint16_t)(sizeof mtv_bases / sizeof *mtv_bases);
    *(struct mtv_base **)(mtv_list + 0x18) = mtv_bases;
    fake_mtv_stored = 0;
    mtv_ai_calls = 0;
    mtv_keep_rows = 0;
    printf("      myteam keep %s: loading 4 saved bases, 2 of them on myteam_mes_porsche_2026 (the test car, no row)\n", on ? "on" : "off");
    fflush(stdout);
    uint32_t n = fake_mtv_load(mtv_list);
    printf("      myteam keep %s: the load returned, %u bases stored, %d AI configs built, %d from a stock row\n", on ? "on" : "off", n,
           mtv_ai_calls, mtv_keep_rows);
    for (int i = 0; i < 4; i++) {
        uint8_t *car = mtv_cars[i];
        fake_mtv_save(car);
        printf("      myteam keep %s: base %d live '%s' engine %u base %08x, saved as '%s' engine %u base %08x\n", on ? "on" : "off", i,
               (const char *)car + 0x70, car[0xce9], *(uint32_t *)(car + 0xcf2), mtv_written_name, mtv_written_engine, mtv_written_base);
    }
    fflush(stdout);
    expect(n == 4 && mtv_ai_calls == 4 && mtv_keep_rows == 2, "myteam keep: every base is stored, the two test cars from a stock row");
    expect(!strcmp((const char *)mtv_cars[1] + 0x70, "myteam_mercedes_2026") && mtv_cars[1][0xce9] == 2 &&
               *(uint32_t *)(mtv_cars[1] + 0xcf2) == 0x02160000u,
           "myteam keep: the test car loads as the stock placeholder");
    fake_mtv_save(mtv_cars[1]);
    expect(!strcmp(mtv_written_name, "myteam_mes_porsche_2026") && mtv_written_engine == 0x16 && mtv_written_base == 0x00a40001u &&
               mtv_written_handle == 1u,
           "myteam keep: the save writes the test car as saved");
    fake_mtv_save(mtv_cars[0]);
    expect(!strcmp(mtv_written_name, "myteam_ferrari") && mtv_written_base == 0x00a40000u, "myteam keep: another car is written as it is");
    snprintf((char *)mtv_cars[3] + 0x70, 0x100, "%s", "myteam_renault");
    fake_mtv_save(mtv_cars[3]);
    expect(!strcmp(mtv_written_name, "myteam_renault"), "myteam keep: a placeholder replaced in place is written as it is");
}

static void crash_leaderboard_guard(void)
{
    static uint8_t screen[0x900];
    SetErrorMode(SEM_NOGPFAULTERRORBOX);
    lb_screen(screen, NULL, 0);
    printf("      leaderboard: View Leaderboards with an empty board list\n");
    fflush(stdout);
    fake_lb_select(screen);
    printf("      leaderboard: the view returned\n");
    fflush(stdout);
}

// Game_PracticeTrackAcclimatisation_RefreshState, copied byte for byte from the Steam build (0x14299e3a0..0x14299e8a6)
// with its unwind data, except that its one direct call (to 0x1429a02d0) goes to fake_acclim_gates. Core's track
// acclimatisation guard resolves it here, and the test refreshes a programme with targets and one without.
__asm__(
    ".section .xtls,\"xr\"\n"
    ".globl fake_acclim_refresh\n"
    ".def fake_acclim_refresh; .scl 2; .type 32; .endef\n"
    ".seh_proc fake_acclim_refresh\n"
    "fake_acclim_refresh:\n"
    ".byte 0x41,0x57\n.seh_pushreg %r15\n"
    ".byte 0x41,0x56\n.seh_pushreg %r14\n"
    ".byte 0x41,0x54\n.seh_pushreg %r12\n"
    ".byte 0x56\n.seh_pushreg %rsi\n"
    ".byte 0x57\n.seh_pushreg %rdi\n"
    ".byte 0x55\n.seh_pushreg %rbp\n"
    ".byte 0x53\n.seh_pushreg %rbx\n"
    ".byte 0x48,0x83,0xEC,0x20\n.seh_stackalloc 32\n"
    ".seh_endprologue\n"
    ".byte 0x48,0x89,0xCE,0x48,0x8B,0x81,0x68,0x03,0x00,0x00,0x48,0x85,0xC0,0x74,0x09,0x0F,0xB6,0x80,0xA7,0x00,0x00,0x00,0xEB,0x02\n"
    ".byte 0x31,0xC0,0x88,0x86,0x00,0x08,0x00,0x00,0x0F,0xB6,0x96,0x36,0x03,0x00,0x00,0x8B,0x8E,0x28,0x03,0x00,0x00,0x44,0x8D,0x14\n"
    ".byte 0x11,0x44,0x03,0x96,0x08,0x08,0x00,0x00,0x4C,0x8B,0x86,0x58,0x03,0x00,0x00,0x48,0x8B,0x86,0x50,0x03,0x00,0x00,0x49,0x29\n"
    ".byte 0xC0,0x49,0xC1,0xE8,0x04,0x41,0x69,0xD8,0x87,0xB5,0x6F,0x58,0xFF,0xCB,0x8B,0xAE,0x04,0x08,0x00,0x00,0x01,0xDD,0x44,0x39\n"
    ".byte 0xD5,0x44,0x0F,0x42,0xD5,0x44,0x8B,0x8E,0x30,0x03,0x00,0x00,0x44,0x8B,0x9E,0xE4,0x07,0x00,0x00,0x44,0x0F,0xB6,0x86,0x34\n"
    ".byte 0x03,0x00,0x00,0x45,0x84,0xC0,0x74,0x1F,0x45,0x85,0xC9,0x75,0x1A,0x41,0x8D,0x7A,0xFF,0x45,0x31,0xE4,0x41,0x39,0xFB,0x73\n"
    ".byte 0x1D,0x40,0xB7,0x01,0x41,0xBC,0x01,0x00,0x00,0x00,0x45,0x31,0xFF,0xEB,0x3C,0x45,0x31,0xE4,0x0F,0xB6,0xBE,0x82,0x03,0x00\n"
    ".byte 0x00,0x44,0x08,0xC7,0xEB,0x07,0x80,0xBE,0x82,0x03,0x00,0x00,0x00,0xBF,0x00,0x00,0x00,0x00,0x41,0xBF,0x00,0x00,0x00,0x00\n"
    ".byte 0x75,0x19,0xC5,0xFA,0x10,0x86,0x38,0x03,0x00,0x00,0xC5,0xF8,0x2E,0x86,0xE8,0x04,0x00,0x00,0x41,0x0F,0x92,0xC7,0x45,0x31\n"
    ".byte 0xE4,0x31,0xFF,0x45,0x89,0xD6,0x45,0x29,0xE6,0x44,0x39,0xF5,0x44,0x0F,0x42,0xF5,0x44,0x29,0xE1,0x39,0xCB,0x0F,0x42,0xCB\n"
    ".byte 0x80,0xBE,0x30,0x08,0x00,0x00,0x00,0x0F,0xB6,0x9E,0x80,0x03,0x00,0x00,0x0F,0x84,0x4F,0x01,0x00,0x00,0x84,0xDB,0x74,0x41\n"
    ".byte 0x8B,0x9E,0x7C,0x03,0x00,0x00,0xF7,0xC3,0xFD,0xFF,0xFF,0xFF,0x75,0x2C,0x45,0x31,0xFF,0x83,0xFB,0x02,0x41,0x0F,0x94,0xC7\n"
    ".byte 0x49,0xC1,0xE7,0x05,0x4A,0x8B,0x9C,0x3E,0xD0,0x03,0x00,0x00,0x4A,0x2B,0x9C,0x3E,0xC8,0x03,0x00,0x00,0x48,0xC1,0xEB,0x04\n"
    ".byte 0x69,0xDB,0x39,0x8E,0xE3,0x38,0x85,0xDB,0x75,0x07,0x80,0x8E,0xFC,0x07,0x00,0x00,0x08,0xC6,0x86,0x80,0x03,0x00,0x00,0x00\n"
    ".byte 0x89,0x8E,0xE0,0x07,0x00,0x00,0x44,0x89,0xB6,0xE4,0x07,0x00,0x00,0xC6,0x86,0x30,0x08,0x00,0x00,0x01,0x45,0x39,0xD3,0x41\n"
    ".byte 0x0F,0x94,0xC2,0x44,0x39,0x8E,0xE8,0x07,0x00,0x00,0x74,0x28,0x41,0x08,0xFA,0x74,0x23,0xC6,0x86,0x30,0x08,0x00,0x00,0x01\n"
    ".byte 0x84,0xD2,0x74,0x11,0x89,0xC9,0x48,0x69,0xC9,0x70,0x03,0x00,0x00,0x44,0x8B,0x4C,0x08,0x04,0x41,0xFF,0xC9,0x44,0x89,0x8E\n"
    ".byte 0xE8,0x07,0x00,0x00,0xC5,0xFA,0x10,0x86,0x34,0x08,0x00,0x00,0x45,0x84,0xC0,0x74,0x16,0xC5,0xF0,0x57,0xC9,0xC5,0xF8,0x2E\n"
    ".byte 0xC1,0x76,0x0C,0xC7,0x86,0x34,0x08,0x00,0x00,0x00,0x00,0x00,0x00,0xEB,0x0A,0xC5,0xF0,0x57,0xC9,0xC5,0xF8,0x2E,0xC8,0x72\n"
    ".byte 0x0F,0x48,0x89,0xF1\n"
    "call fake_acclim_gates\n"
    ".byte 0x48,0x8B,0x86,0x50,0x03,0x00,0x00,0x8B,0x8E,0xE0,0x07,0x00,0x00,0x48,0x69,0xC9,0x70,0x03,0x00,0x00,0x48,0x8D,0x14,0x08\n"
    ".byte 0x48,0x83,0xC2,0x04,0x44,0x8B,0x4C,0x08,0x04,0x45,0x31,0xD2,0x4D,0x85,0xC9,0x74,0x76,0x44,0x8B,0x9C,0x08,0xC4,0x00,0x00\n"
    ".byte 0x00,0x41,0xF6,0xC3,0x02,0x74,0x0D,0x41,0x83,0xE3,0x18,0x41,0xB0,0x01,0x41,0x83,0xFB,0x08,0x75,0x6A,0x4C,0x8D,0x1C,0x01\n"
    ".byte 0x49,0x81,0xC3,0x94,0x01,0x00,0x00,0x49,0x8D,0x79,0xFF,0x45,0x31,0xC0,0x66,0x66,0x2E,0x0F,0x1F,0x84,0x00,0x00,0x00,0x00\n"
    ".byte 0x00,0x4C,0x39,0xC7,0x74,0x3E,0x41,0x8B,0x1B,0x49,0x81,0xC3,0xD0,0x00,0x00,0x00,0x49,0xFF,0xC0,0xF6,0xC3,0x02,0x74,0xE9\n"
    ".byte 0x83,0xE3,0x18,0x83,0xFB,0x08,0x74,0xE1,0xEB,0x25,0x45,0x39,0xD3,0x40,0x0F,0x94,0xC5,0x84,0xDB,0x0F,0x85,0xA2,0xFE,0xFF\n"
    ".byte 0xFF,0x41,0x08,0xEF,0x0F,0x85,0xF9,0xFE,0xFF,0xFF,0xE9,0x94,0xFE,0xFF,0xFF,0x45,0x31,0xC0,0xEB,0x0A,0x4D,0x89,0xC8,0x4D\n"
    ".byte 0x39,0xC8,0x41,0x0F,0x92,0xC0,0x4C,0x8D,0x0C,0x08,0x49,0x81,0xC1,0x62,0x03,0x00,0x00,0xC7,0x86,0xC4,0x07,0x00,0x00,0x00\n"
    ".byte 0x00,0x00,0x00,0x83,0x3A,0x00,0x74,0x3A,0x41,0x80,0x39,0x00,0x41,0x0F,0x95,0xC2,0x45,0x08,0xC2,0x41,0xBA,0x02,0x00,0x00\n"
    ".byte 0x00,0x75,0x27,0x44,0x8B,0x9C,0x08,0xC4,0x00,0x00,0x00,0x41,0xBA,0x01,0x00,0x00,0x00,0x41,0xF6,0xC3,0x02,0x74,0x13,0x41\n"
    ".byte 0x83,0xE3,0x24,0x45,0x31,0xD2,0x41,0x83,0xFB,0x04,0x41,0x0F,0x94,0xC2,0x41,0x83,0xCA,0x04,0x44,0x39,0x96,0xC0,0x07,0x00\n"
    ".byte 0x00,0x74,0x29,0x44,0x8B,0x9C,0x08,0xC4,0x00,0x00,0x00,0x41,0xF7,0xD3,0x41,0xF6,0xC3,0x18,0x75,0x0A,0xC7,0x86,0xC4,0x07\n"
    ".byte 0x00,0x00,0x02,0x00,0x00,0x00,0x44,0x89,0x96,0xC0,0x07,0x00,0x00,0xC6,0x86,0x30,0x08,0x00,0x00,0x01,0xC7,0x86,0xCC,0x07\n"
    ".byte 0x00,0x00,0x00,0x00,0x00,0x00,0x45,0x31,0xD2,0x83,0x3A,0x02,0x72,0x3A,0x41,0x80,0x39,0x00,0x41,0x0F,0x95,0xC2,0x45,0x08\n"
    ".byte 0xC2,0x41,0xBA,0x02,0x00,0x00,0x00,0x75,0x27,0x44,0x8B,0x9C,0x08,0x94,0x01,0x00,0x00,0x41,0xBA,0x01,0x00,0x00,0x00,0x41\n"
    ".byte 0xF6,0xC3,0x02,0x74,0x13,0x41,0x83,0xE3,0x24,0x45,0x31,0xD2,0x41,0x83,0xFB,0x04,0x41,0x0F,0x94,0xC2,0x41,0x83,0xCA,0x04\n"
    ".byte 0x44,0x39,0x96,0xC8,0x07,0x00,0x00,0x74,0x29,0x44,0x8B,0x9C,0x08,0x94,0x01,0x00,0x00,0x41,0xF7,0xD3,0x41,0xF6,0xC3,0x18\n"
    ".byte 0x75,0x0A,0xC7,0x86,0xCC,0x07,0x00,0x00,0x02,0x00,0x00,0x00,0x44,0x89,0x96,0xC8,0x07,0x00,0x00,0xC6,0x86,0x30,0x08,0x00\n"
    ".byte 0x00,0x01,0xC7,0x86,0xD4,0x07,0x00,0x00,0x00,0x00,0x00,0x00,0x45,0x31,0xD2,0x83,0x3A,0x03,0x72,0x3A,0x41,0x80,0x39,0x00\n"
    ".byte 0x41,0x0F,0x95,0xC2,0x45,0x08,0xC2,0x41,0xBA,0x02,0x00,0x00,0x00,0x75,0x27,0x44,0x8B,0x9C,0x08,0x64,0x02,0x00,0x00,0x41\n"
    ".byte 0xBA,0x01,0x00,0x00,0x00,0x41,0xF6,0xC3,0x02,0x74,0x13,0x41,0x83,0xE3,0x24,0x45,0x31,0xD2,0x41,0x83,0xFB,0x04,0x41,0x0F\n"
    ".byte 0x94,0xC2,0x41,0x83,0xCA,0x04,0x44,0x39,0x96,0xD0,0x07,0x00,0x00,0x74,0x29,0x44,0x8B,0x9C,0x08,0x64,0x02,0x00,0x00,0x41\n"
    ".byte 0xF7,0xD3,0x41,0xF6,0xC3,0x18,0x75,0x0A,0xC7,0x86,0xD4,0x07,0x00,0x00,0x02,0x00,0x00,0x00,0x44,0x89,0x96,0xD0,0x07,0x00\n"
    ".byte 0x00,0xC6,0x86,0x30,0x08,0x00,0x00,0x01,0xC7,0x86,0xDC,0x07,0x00,0x00,0x00,0x00,0x00,0x00,0x45,0x31,0xD2,0x83,0x3A,0x04\n"
    ".byte 0x72,0x35,0x41,0x80,0x39,0x00,0x0F,0x95,0xC2,0x41,0x08,0xD0,0x41,0xBA,0x02,0x00,0x00,0x00,0x75,0x23,0x8B,0x94,0x08,0x34\n"
    ".byte 0x03,0x00,0x00,0x41,0xBA,0x01,0x00,0x00,0x00,0xF6,0xC2,0x02,0x74,0x11,0x83,0xE2,0x24,0x45,0x31,0xD2,0x83,0xFA,0x04,0x41\n"
    ".byte 0x0F,0x94,0xC2,0x41,0x83,0xCA,0x04,0x44,0x39,0x96,0xD8,0x07,0x00,0x00,0x75,0x0B,0x80,0xBE,0x30,0x08,0x00,0x00,0x00,0x75\n"
    ".byte 0x27,0xEB,0x5A,0x8B,0x84,0x08,0x34,0x03,0x00,0x00,0xF7,0xD0,0xA8,0x18,0x75,0x0A,0xC7,0x86,0xDC,0x07,0x00,0x00,0x02,0x00\n"
    ".byte 0x00,0x00,0x44,0x89,0x96,0xD8,0x07,0x00,0x00,0xC6,0x86,0x30,0x08,0x00,0x00,0x01,0x48,0x8D,0xBE,0x08,0x05,0x00,0x00,0x48\n"
    ".byte 0x8B,0x9E,0xD0,0x01,0x00,0x00,0x48,0x8B,0x86,0x08,0x05,0x00,0x00,0x48,0x89,0xF9,0xFF,0x50,0x08,0x48,0x98,0x48,0x39,0x7C\n"
    ".byte 0xC3,0x20,0x74,0x20,0xC7,0x86,0xFC,0x07,0x00,0x00,0x00,0x00,0x00,0x00,0xC6,0x86,0x30,0x08,0x00,0x00,0x00,0x48,0x83,0xC4\n"
    ".byte 0x20,0x5B,0x5D,0x5F,0x5E,0x41,0x5C,0x41,0x5E,0x41,0x5F,0xC3,0x48,0x8B,0x44,0xC3,0x28,0x48,0x8B,0x78,0x08,0x48,0x8B,0x58\n"
    ".byte 0x10,0x48,0x39,0xDF,0x74,0xCE,0x66,0x66,0x2E,0x0F,0x1F,0x84,0x00,0x00,0x00,0x00,0x00,0x48,0x8B,0x4F,0x08,0xBA,0x02,0x00\n"
    ".byte 0x00,0x00,0xFF,0x17,0x48,0x83,0xC7,0x10,0x48,0x39,0xDF,0x75,0xEC,0xEB,0xAD\n"
    ".seh_endproc\n"
    ".section .drectve\n"
    ".ascii \" -export:fake_acclim_refresh\"\n"
    ".text\n");
void fake_acclim_refresh(void *self);

static int acclim_gates, acclim_notified;

__attribute__((used, noinline)) void fake_acclim_gates(void *self)
{
    (void)self;
    acclim_gates++;
}

static int acclim_slot(void *self)
{
    (void)self;
    return 0;
}

static void acclim_listener(void *ctx, int code)
{
    (void)ctx;
    acclim_notified += code == 2;
}

// Game_PracticeTrackAcclimatisation_FlagCurrentTarget, copied byte for byte from the Steam build
// (0x1429a0620..0x1429a0647, a leaf without unwind data). Core's track acclimatisation penalty guard resolves its
// entry and ret sites here, and the test flags the current target of a programme with targets and of one without.
__asm__(
    ".section .xtls,\"xr\"\n"
    ".globl fake_acclim_flag\nfake_acclim_flag:\n"
    ".byte 0x8B,0x91,0x28,0x03,0x00,0x00,0x48,0x8B,0x81,0x50,0x03,0x00,0x00,0x48,0x69,0xCA,0x70,0x03,0x00,0x00,0x83,0xBC,0x08,0xC4\n"
    ".byte 0x00,0x00,0x00,0x00,0x74,0x08,0xC6,0x84,0x08,0x62,0x03,0x00,0x00,0x01,0xC3,0xCC,0xCC,0xCC,0xCC,0xCC,0xCC,0xCC,0xCC,0xCC\n"
    ".section .drectve\n"
    ".ascii \" -export:fake_acclim_flag\"\n"
    ".text\n");
void fake_acclim_flag(void *self);

static void test_track_acclim(void)
{
    static uint8_t empty[0x840], before[0x840], held[0x840], targets[2][0x370], table[0x30], list[0x18];
    static uint8_t flagged[0x840], marks[3][0x370];
    static void *vt[2] = {NULL, (void *)acclim_slot};
    static void *listeners[2] = {(void *)acclim_listener, NULL};
    memcpy(before, empty, sizeof empty);
    fake_acclim_refresh(empty);
    expect(!memcmp(empty, before, sizeof empty) && acclim_gates == 0,
           "acclim: refreshing a Track Acclimatisation programme without targets returns and changes nothing");
    *(uint32_t *)(targets[0] + 4) = 2;
    *(void **)(held + 0x350) = targets;
    *(void **)(held + 0x358) = targets + 2;
    *(int32_t *)(held + 0x7e8) = 5;
    *(void **)(held + 0x508) = vt;
    *(void **)(held + 0x1d0) = table;
    *(void **)(table + 0x20) = held + 0x508;
    *(void **)(table + 0x28) = list;
    *(void **)(list + 8) = listeners;
    *(void **)(list + 0x10) = listeners + 2;
    fake_acclim_refresh(held);
    uint32_t g0 = *(uint32_t *)(held + 0x7c0), g1 = *(uint32_t *)(held + 0x7c8), g2 = *(uint32_t *)(held + 0x7d0);
    int32_t lap = *(int32_t *)(held + 0x7e8);
    printf("      refresh with 2 targets: gate states %u %u %u, lap mark %d, dirty %u, gates %d, notified %d\n", g0, g1, g2,
           lap, held[0x830], acclim_gates, acclim_notified);
    expect(g0 == 1 && g1 == 1 && g2 == 0 && lap == 0 && held[0x830] == 0 && acclim_gates == 1 && acclim_notified == 1,
           "acclim: a programme with two targets runs the game's refresh: two gate states set, listeners notified once");
    memcpy(before, empty, sizeof empty);
    fake_acclim_flag(empty);
    expect(!memcmp(empty, before, sizeof empty),
           "acclim: flagging a penalty on a Track Acclimatisation programme without targets returns and changes nothing");
    *(uint32_t *)(marks[1] + 0xc4) = 2;
    *(void **)(flagged + 0x350) = marks;
    *(void **)(flagged + 0x358) = marks + 3;
    *(uint32_t *)(flagged + 0x328) = 1;
    fake_acclim_flag(flagged);
    *(uint32_t *)(flagged + 0x328) = 2;
    fake_acclim_flag(flagged);
    printf("      penalty flag with 3 targets: flags %u %u %u\n", marks[0][0x362], marks[1][0x362], marks[2][0x362]);
    expect(marks[0][0x362] == 0 && marks[1][0x362] == 1 && marks[2][0x362] == 0,
           "acclim: a programme with targets runs the game's penalty flag: only the current target with gate flags is "
           "flagged");
}

static void crash_track_acclim(int penalty)
{
    static uint8_t programme[0x840];
    SetErrorMode(SEM_NOGPFAULTERRORBOX);
    printf("      acclim: %s a Track Acclimatisation programme without targets\n",
           penalty ? "flagging a penalty on" : "refreshing");
    fflush(stdout);
    if (penalty)
        fake_acclim_flag(programme);
    else
        fake_acclim_refresh(programme);
    printf("      acclim: the %s returned\n", penalty ? "penalty flag" : "refresh");
    fflush(stdout);
}

// The ERS mode load of Car_Powertrain_Step, copied byte for byte from the Steam build (0x1446e2858..0x1446e2878),
// in a small frame that takes the powertrain in rcx and the ERS system in rdx and returns the gear modifier it loads.
// Core's ERS mode guard resolves site_Car_ErsGearModifierLoad here, at +0x20.
__asm__(
    ".section .xtls,\"xr\"\n"
    ".globl fake_powertrain_ers\n"
    ".def fake_powertrain_ers; .scl 2; .type 32; .endef\n"
    ".seh_proc fake_powertrain_ers\n"
    "fake_powertrain_ers:\n"
    ".byte 0x56\n.seh_pushreg %rsi\n"
    ".byte 0x41,0x56\n.seh_pushreg %r14\n"
    ".seh_endprologue\n"
    ".byte 0x48,0x89,0xCE,0x49,0x89,0xD6\n"
    ".byte 0x8B,0x8E,0xF0,0x04,0x00,0x00,0x49,0x63,0x86,0x68,0x1A,0x00,0x00,0x48,0x89,0xC2,0x48,0xC1,0xE2,0x06\n"
    ".byte 0x4C,0x01,0xF2,0xC5,0xFA,0x10,0x8C,0x8A,0x90,0x18,0x00,0x00\n"
    ".byte 0xC5,0xF8,0x28,0xC1,0x41,0x5E,0x5E,0xC3\n"
    ".seh_endproc\n"
    ".section .drectve\n"
    ".ascii \" -export:fake_powertrain_ers\"\n"
    ".text\n");
float fake_powertrain_ers(void *powertrain, void *ers);

static int ers_resets;
static uint32_t ers_reset_flags;

static void fake_ers_reset(uint8_t *ers, uint32_t flags)
{
    ers_resets++;
    ers_reset_flags = flags;
    if (~flags & 0x210)
        return;
    memcpy(ers + 0x19f0, ers + 0x60, 4);
    *(uint64_t *)(ers + 0x1a68) = 0x500000001ull;
}

static void *fake_ers_vt[8] = {NULL, NULL, NULL, (void *)fake_ers_reset};
static uint8_t ers_cars[3][0x100], ers_powertrain[0x500];
static __attribute__((aligned(16))) uint8_t ers_systems[3][0x1a78];

static uint8_t *ers_system(int i, int32_t mode)
{
    uint8_t *ers = ers_systems[i];
    float capacity = 4e6f;
    memset(ers, 0, sizeof ers_systems[i]);
    *(void ***)ers = fake_ers_vt;
    *(void **)(ers + 8) = ers_cars[i];
    ers_cars[i][0xe8] = (uint8_t)(7 + i);
    memcpy(ers + 0x60, &capacity, 4);
    for (int m = 0; m < 4; m++) {
        float modifier = 0.25f * (m + 1);
        memcpy(ers + 0x1890 + m * 0x40 + 4, &modifier, 4);
    }
    memcpy(ers + 0x1a68, &mode, 4);
    *(uint32_t *)(ers_powertrain + 0x4f0) = 1;
    return ers;
}

static void test_ers_mode(void)
{
    float got = fake_powertrain_ers(ers_powertrain, ers_system(0, 2));
    expect(got == 0.75f && ers_resets == 0,
           "ers mode: a car in Overtake loads the Overtake gear modifier and its ERS is not reset");
    uint8_t *ers = ers_system(1, (int32_t)0x8054c554);
    got = fake_powertrain_ers(ers_powertrain, ers);
    float energy = 0.0f;
    int32_t mode = -1;
    memcpy(&energy, ers + 0x19f0, 4);
    memcpy(&mode, ers + 0x1a68, 4);
    printf("      ERS mode 0x8054c554: loaded %.2f, mode %d, energy %.0f, %d reset(s) with flags 0x%x\n", got, mode,
           energy, ers_resets, ers_reset_flags);
    expect(got == 0.5f && mode == 1 && energy == 4e6f && ers_resets == 1 && ers_reset_flags == 0x210,
           "ers mode: a car whose ERS mode was never set gets the game's ERS reset (0x10|0x200) and loads the Medium "
           "gear modifier");
    got = fake_powertrain_ers(ers_powertrain, ers);
    expect(got == 0.5f && ers_resets == 1, "ers mode: that car's next step runs the game's code unchanged");
    got = fake_powertrain_ers(ers_powertrain, ers_system(2, 4));
    expect(got == 0.5f && ers_resets == 2, "ers mode: a mode one past Hotlap is reset to Medium too");
}

static void crash_ers_mode(void)
{
    SetErrorMode(SEM_NOGPFAULTERRORBOX);
    printf("      ers mode: stepping a car whose ERS mode was never set\n");
    fflush(stdout);
    float got = fake_powertrain_ers(ers_powertrain, ers_system(0, (int32_t)0x8054c554));
    printf("      ers mode: the step returned %.2f\n", got);
    fflush(stdout);
}

// Career_Standings_BuildWeekendNode's track read, copied byte for byte from the Steam build (0x14181e195..0x14181e1af
// and 0x14181e4dc..0x14181e4f0, the je's rel32 aside), in a small frame that takes the standings view in rcx, the
// round's TrackDataRecord in rdx, the round index in r8 and the round's Weekend in r9 (kept at [rsp+0xa0] as the game
// does). It returns the record's +0x2c0 on the unchecked path, its +0x128 on the checked one and 0 for a round without
// a record. Core's standings guard resolves site_Career_StandingsWeekendTrackLoad here at +0x28 and
// site_Career_StandingsWeekendTrackDone at +0x38.
__asm__(
    ".section .xtls,\"xr\"\n"
    ".globl fake_standings_weekend\n"
    ".def fake_standings_weekend; .scl 2; .type 32; .endef\n"
    ".seh_proc fake_standings_weekend\n"
    "fake_standings_weekend:\n"
    ".byte 0x55\n.seh_pushreg %rbp\n"
    ".byte 0x53\n.seh_pushreg %rbx\n"
    ".byte 0x41,0x54\n.seh_pushreg %r12\n"
    ".byte 0x48,0x81,0xEC,0xB0,0x00,0x00,0x00\n.seh_stackalloc 0xb0\n"
    ".seh_endprologue\n"
    ".byte 0x49,0x89,0xCC,0x48,0x89,0xD3,0x4C,0x89,0xC5,0x4C,0x89,0x8C,0x24,0xA0,0x00,0x00,0x00\n"
    ".byte 0x41,0x80,0x7C,0x24,0x4A,0x00,0x0F,0x84,0x10,0x00,0x00,0x00\n"
    ".byte 0x48,0x8B,0x83,0xC0,0x02,0x00,0x00,0x48,0x8D,0x48,0x08,0x48,0x85,0xC0\n"
    ".byte 0xEB,0x64\n"
    ".byte 0x48,0x85,0xDB,0x74,0x5D,0x41,0x80,0x7C,0x24,0x49,0x00,0x74,0x55,0x48,0x8B,0x83,0x28,0x01,0x00,0x00\n"
    ".byte 0xEB,0x4E\n"
    ".fill 76,1,0xCC\n"
    ".byte 0x31,0xC0\n"
    ".byte 0x48,0x81,0xC4,0xB0,0x00,0x00,0x00,0x41,0x5C,0x5B,0x5D,0xC3\n"
    ".seh_endproc\n"
    ".section .drectve\n"
    ".ascii \" -export:fake_standings_weekend\"\n"
    ".text\n");
uint64_t fake_standings_weekend(void *view, void *record, uint64_t round, void *weekend);

static uint8_t standings_view[0x50], standings_record[0x2c8], standings_weekend[2][0x280];

static void standings_setup(void)
{
    standings_view[0x4a] = 1;
    standings_view[0x49] = 1;
    *(uint64_t *)(standings_record + 0x2c0) = 0x2c0c0de;
    *(uint64_t *)(standings_record + 0x128) = 0x128c0de;
    standings_weekend[0][0x278] = 3;
    standings_weekend[1][0x278] = 200;
}

static void test_standings(void)
{
    standings_setup();
    uint64_t r = fake_standings_weekend(standings_view, standings_record, 0, standings_weekend[0]);
    expect(r == 0x2c0c0de, "standings: a round with track data reads its record as the game does");
    r = fake_standings_weekend(standings_view, NULL, 6, standings_weekend[1]);
    uint64_t again = fake_standings_weekend(standings_view, NULL, 6, standings_weekend[1]);
    expect(r == 0 && again == 0, "standings: a round whose track has no track data takes the game's no-record path");
    r = fake_standings_weekend(standings_view, NULL, 9, NULL);
    expect(r == 0, "standings: so does a round whose weekend can't be read");
    standings_view[0x4a] = 0;
    r = fake_standings_weekend(standings_view, standings_record, 1, standings_weekend[0]);
    expect(r == 0x128c0de, "standings: the game's checked path still reads the round's record");
    standings_view[0x4a] = 1;
}

static void crash_standings(void)
{
    SetErrorMode(SEM_NOGPFAULTERRORBOX);
    standings_setup();
    printf("      standings: building a round whose track has no track data\n");
    fflush(stdout);
    uint64_t r = fake_standings_weekend(standings_view, NULL, 4, standings_weekend[1]);
    printf("      standings: the round returned %llu\n", (unsigned long long)r);
    fflush(stdout);
}

// The tail of Game_GarageCrewModule_SpawnGarageCrew's assignment loop, copied byte for byte from the Steam build
// (0x14038930e..0x14038931a and 0x14038931e), in a small frame that takes the vector begin in rcx, its end in rdx and
// the garage id in r8d and returns the entry count the game computes after the site. Core's garage crew guard resolves
// site_Game_GarageCrew_SpawnVector here, at +0x12.
__asm__(
    ".section .xtls,\"xr\"\n"
    ".globl fake_garage_crew\n"
    ".def fake_garage_crew; .scl 2; .type 32; .endef\n"
    ".seh_proc fake_garage_crew\n"
    "fake_garage_crew:\n"
    ".byte 0x41,0x55\n.seh_pushreg %r13\n"
    ".byte 0x55\n.seh_pushreg %rbp\n"
    ".seh_endprologue\n"
    ".byte 0x49,0x89,0xD5,0x44,0x89,0xC5\n"
    ".byte 0xEB,0x04,0x31,0xF6,0x31,0xC9\n"
    ".byte 0x4D,0x89,0xE8,0x49,0x29,0xC8,0x49,0xC1,0xF8,0x04,0x4C,0x89,0xEA\n"
    ".byte 0x4C,0x89,0xC0,0x5D,0x41,0x5D,0xC3\n"
    ".seh_endproc\n"
    ".section .drectve\n"
    ".ascii \" -export:fake_garage_crew\"\n"
    ".text\n");
uint64_t fake_garage_crew(void *begin, void *end, uint32_t garage);

static void crew_entries(uint8_t (*records)[0x98], uint8_t (*entries)[16], const uint32_t *slots, int n, int first)
{
    for (int i = 0; i < n; i++) {
        uint8_t *rec = records[i];
        memset(rec, 0, 0x98);
        *(uint16_t *)(rec + 0x52) = (uint16_t)(300 + first + i);
        *(uint16_t *)(rec + 0x5a) = (uint16_t)(11 + first);
        rec[0x83] = (uint8_t)(40 + first + i);
        memset(entries[i], 0, 16);
        *(uint8_t **)entries[i] = rec;
        *(uint32_t *)(entries[i] + 8) = slots[i];
    }
}

static void test_garage_crew(int guard)
{
    static uint8_t records[5][0x98], entries[5][16];
    uint32_t two[] = {0, 1}, three[] = {0, 1, 2}, shuffled[] = {2, 0, 1}, four[] = {0, 1, 2, 3};
    crew_entries(records, entries, two, 2, 0);
    uint64_t n = fake_garage_crew(entries, entries + 2, 4);
    expect(n == 2 && *(uint8_t **)entries[0] == records[0] && *(uint8_t **)entries[1] == records[1],
           "garage crew: a garage with two cars keeps both crews and its vector is untouched");
    crew_entries(records, entries, three, 3, 1);
    n = fake_garage_crew(entries, entries + 3, 5);
    printf("      garage crew: garage 5 with slots 0,1,2 keeps %llu entries\n", (unsigned long long)n);
    if (!guard) {
        expect(n == 3, "garage crew: with the guard off the third car stays in the vector, as in the game");
        return;
    }
    expect(n == 2 && *(uint8_t **)entries[0] == records[0] && *(uint8_t **)entries[1] == records[1],
           "garage crew: a third car in one garage is dropped from the crew vector, the first two keep their crews");
    crew_entries(records, entries, shuffled, 3, 2);
    n = fake_garage_crew(entries, entries + 3, 6);
    expect(n == 2 && *(uint8_t **)entries[0] == records[1] && *(uint32_t *)(entries[0] + 8) == 0 &&
               *(uint8_t **)entries[1] == records[2] && *(uint32_t *)(entries[1] + 8) == 1,
           "garage crew: a slot 2 record ahead of the others is dropped and the rest close up in order");
    crew_entries(records, entries, four, 4, 3);
    n = fake_garage_crew(entries, entries + 4, 7);
    expect(n == 2, "garage crew: a garage with four records keeps two");
    n = fake_garage_crew(NULL, NULL, 8);
    expect(n == 0, "garage crew: a garage with no records runs the game's empty path");
}

// The per-garage tail of the pit stops module's setup (fn 0x1403c69e0), with the byte runs core's pit crew guard
// resolves its sites from copied from the Steam build: the template read at 0x1403c6ee1 (mov rbx,[rsp+0x70] ...), and
// the test r12,r12 / je ahead of the free of the garage's entry vector (0x1403c787b, the site the skip lands on). It takes the crew
// template in rcx (the game's rdi), the entry vector begin in rdx (stored at [rsp+0xa8]), the garage id in r8 (the
// game's r14) and a flag in r9 that the stand-in for Game_PitStops_SetupCrew sets, then reads [template + 0x560]
// there. It returns 2 after the vector free and 1 when the vector is null, plus 0x100 when the loop registers the next
// garage relies on hold what the game leaves there (r13 = rsp+0xc0, sil = the byte at [rsp+0x4b]).
__asm__(
    ".section .xtls,\"xr\"\n"
    ".globl fake_pit_crew\n"
    ".def fake_pit_crew; .scl 2; .type 32; .endef\n"
    ".seh_proc fake_pit_crew\n"
    "fake_pit_crew:\n"
    "push %rbx\n.seh_pushreg %rbx\n"
    "push %rsi\n.seh_pushreg %rsi\n"
    "push %rdi\n.seh_pushreg %rdi\n"
    "push %r12\n.seh_pushreg %r12\n"
    "push %r13\n.seh_pushreg %r13\n"
    "push %r14\n.seh_pushreg %r14\n"
    "sub $0xb8,%rsp\n.seh_stackalloc 0xb8\n"
    ".seh_endprologue\n"
    "mov %rcx,%rdi\n"
    "mov %rdx,0xa8(%rsp)\n"
    "mov %r8,%r14\n"
    "mov %r9,0xb0(%rsp)\n"
    "lea 0x90(%rsp),%rax\n"
    "mov %rax,0x70(%rsp)\n"
    "movq $0,0x90(%rsp)\n"
    "movq $0,0x98(%rsp)\n"
    "movq $0,0xa0(%rsp)\n"
    "movb $0x5a,0x4b(%rsp)\n"
    ".byte 0x48,0x8B,0x5C,0x24,0x70,0x4C,0x8B,0x2B,0x48,0x8B,0x73,0x08\n"
    "mov 0xb0(%rsp),%rax\n"
    "movl $1,(%rax)\n"
    "mov 0x560(%rdi),%rax\n"
    "lea 0xc0(%rsp),%r13\n"
    "movzbl 0x4b(%rsp),%esi\n"
    "mov 0xa8(%rsp),%r12\n"
    ".byte 0x4D,0x85,0xE4,0x0F,0x84\n.long 1f-(.+4)\n"
    ".byte 0x48,0x8B,0x8C,0x24,0xA0,0x00,0x00,0x00\n"
    "mov $2,%eax\n"
    "jmp 2f\n"
    "1: mov $1,%eax\n"
    "2: lea 0xc0(%rsp),%rdx\n"
    "cmp %rdx,%r13\n"
    "jne 4f\n"
    "cmp $0x5a,%sil\n"
    "jne 4f\n"
    "or $0x100,%eax\n"
    "4: add $0xb8,%rsp\n"
    "pop %r14\npop %r13\npop %r12\npop %rdi\npop %rsi\npop %rbx\n"
    "ret\n"
    ".seh_endproc\n"
    ".section .drectve\n"
    ".ascii \" -export:fake_pit_crew\"\n"
    ".text\n");
uint64_t fake_pit_crew(void *tmpl, void *vec, uint64_t garage, uint32_t *ran);

// The garage crew scene lookup of Game_GarageCrewModule_SpawnGarageCrew, copied byte for byte from the Steam build
// (0x14038ba20..0x14038ba2a: the hoisted movzx esi,[rdi+0xe0] ahead of test rdi,rdi / je 0x14038baff). It takes the
// scene in rcx (the game's rdi) and the module in rdx (the game's rbp) and returns 0x100 | the byte at scene + 0xe0,
// or 0x50 on the game's null path, which sits 0xcf bytes after the je like the original.
__asm__(
    ".section .xtls,\"xr\"\n"
    ".globl fake_garage_scene\n"
    ".def fake_garage_scene; .scl 2; .type 32; .endef\n"
    ".seh_proc fake_garage_scene\n"
    "fake_garage_scene:\n"
    "push %rbp\n.seh_pushreg %rbp\n"
    "push %rdi\n.seh_pushreg %rdi\n"
    "push %rsi\n.seh_pushreg %rsi\n"
    "sub $0x20,%rsp\n.seh_stackalloc 0x20\n"
    ".seh_endprologue\n"
    "mov %rdx,%rbp\n"
    "mov %rcx,%rdi\n"
    ".byte 0x0F,0xB6,0xB7,0xE0,0x00,0x00,0x00\n"
    ".byte 0x48,0x85,0xFF,0x0F,0x84,0xCF,0x00,0x00,0x00\n"
    "3: mov %esi,%eax\n"
    "or $0x100,%eax\n"
    "add $0x20,%rsp\n"
    "pop %rsi\npop %rdi\npop %rbp\n"
    "ret\n"
    ".org 3b+0xcf,0xCC\n"
    "mov $0x50,%eax\n"
    "add $0x20,%rsp\n"
    "pop %rsi\npop %rdi\npop %rbp\n"
    "ret\n"
    ".seh_endproc\n"
    ".section .drectve\n"
    ".ascii \" -export:fake_garage_scene\"\n"
    ".text\n");
uint64_t fake_garage_scene(void *scene, void *module);

static void test_pit_crew(int guard)
{
    static uint8_t tmpl[0x600], record[0x98];
    static uint64_t vec[2];
    uint32_t ran = 0;
    record[0x5a] = 14;
    record[0x83] = 44;
    vec[0] = (uint64_t)(uintptr_t)record;
    uint64_t r = fake_pit_crew(tmpl, vec, 9, &ran);
    expect(ran == 1 && r == 0x102, "pit crew: a garage with a crew template runs its setup and frees its vector");
    if (!guard) {
        printf("      pit crew: guard off, the null template cases are not run\n");
        return;
    }
    ran = 0;
    r = fake_pit_crew(NULL, vec, 10, &ran);
    expect(ran == 0 && r == 0x102, "pit crew: a garage whose cars have no crew template skips its setup, frees its vector and leaves the loop registers as the game does");
    ran = 0;
    r = fake_pit_crew(NULL, vec, 10, &ran);
    expect(ran == 0 && r == 0x102, "pit crew: the same garage skipped again takes the same path");
    ran = 0;
    r = fake_pit_crew(NULL, NULL, 11, &ran);
    expect(ran == 0 && r == 0x101, "pit crew: a garage with no entry vector skips its setup and takes the game's own null test instead of freeing");
}

static void test_garage_scene(int guard)
{
    static uint8_t scene[0x100], module[0x100];
    scene[0xe0] = 7;
    *(uint64_t *)(module + 0xd0) = 0x1111;
    *(uint64_t *)(module + 0xe0) = 0x2222;
    expect(fake_garage_scene(scene, module) == 0x107, "garage scene: a scene that exists takes the game's own path");
    if (!guard) {
        printf("      garage scene: guard off, the null scene case is not run\n");
        return;
    }
    expect(fake_garage_scene(NULL, module) == 0x50, "garage scene: a missing scene takes the game's null path instead of faulting");
}

// Pit strategy evaluators, rebuilt around the byte runs core's pit slot guard resolves its sites from (copied from the
// Steam build: 0x143691fbb, 0x143692014, 0x143692129..0x14369214c for Sess_PitStrategy_EvaluateAgainstPitList and
// 0x143691d44, 0x143691d80..0x143691db8, 0x143691dc9 for Sess_PitStrategy_EvaluateByRank). fake_pit_eval takes the
// session in rcx, the pit record in rdx and the list bounds in r8 and r9. fake_pit_rank takes the session in rcx,
// the pit record in rdx and the free-slot count in r8d, and reads its list from fake_pit_list.
uint8_t fake_pit_blank[0x100] = {[0xa8] = 0x00, [0xa9] = 0x00, [0xaa] = 0xc8, [0xab] = 0x42};
uint64_t fake_pit_list;
__asm__(
    ".section .xtls,\"xr\"\n"
    ".globl fake_pit_stub\n"
    ".def fake_pit_stub; .scl 2; .type 32; .endef\n"
    ".seh_proc fake_pit_stub\n"
    "fake_pit_stub:\n"
    ".seh_endprologue\n"
    "ret\n"
    ".seh_endproc\n"
    ".globl fake_pit_eval\n"
    ".def fake_pit_eval; .scl 2; .type 32; .endef\n"
    ".seh_proc fake_pit_eval\n"
    "fake_pit_eval:\n"
    "push %r15\n.seh_pushreg %r15\n"
    "push %r12\n.seh_pushreg %r12\n"
    "push %rbp\n.seh_pushreg %rbp\n"
    "push %rbx\n.seh_pushreg %rbx\n"
    "push %rsi\n.seh_pushreg %rsi\n"
    "push %rdi\n.seh_pushreg %rdi\n"
    "sub $0x48, %rsp\n.seh_stackalloc 0x48\n"
    ".seh_endprologue\n"
    "mov %rcx, 0x20(%rsp)\n"
    "mov %rcx, 0x28(%rsp)\n"
    "mov %rdx, %rdi\n"
    "mov %r8, %r15\n"
    "mov %r9, %r12\n"
    "movzbl 0xc4(%rdi), %ecx\n"
    "lea fake_pit_blank(%rip), %rax\n"
    "cmp %r12, %r15\n"
    "je 1f\n"
    "2:\n"
    "mov 0x20(%rsp), %r8\n"
    "mov 0x148(%r8), %r8\n"
    "mov 0xa8(%rax), %r9d\n"
    ".byte 0x49,0x8B,0x0C,0xC8\n"
    ".byte 0x44,0x33,0x88,0xB0,0x00,0x00,0x00\n"
    ".byte 0x8B,0x69,0x48\n"
    "xor 0x50(%rcx), %ebp\n"
    "3:\n"
    "movzbl (%r15), %edx\n"
    "mov 0x20(%rsp), %rax\n"
    "mov 0x148(%rax), %rax\n"
    "mov (%rax,%rdx,8), %rax\n"
    ".byte 0x8B,0x58,0x48,0x8B,0x70,0x50,0x48,0x8B,0x4C,0x24,0x28\n"
    "call fake_pit_stub\n"
    "xor %esi, %ebx\n"
    "cmp %ebp, %ebx\n"
    "setl %bl\n"
    ".byte 0xF6,0xC3,0x01,0x75,0x1E,0x49,0x83,0xC7,0x08,0x4D,0x39,0xE7,0x0F,0x85\n"
    ".long 3b - . - 4\n"
    ".byte 0xEB,0x0F,0xC5,0xC8,0x57,0xF6,0x4D,0x39,0xE7,0x0F,0x85\n"
    ".long 2b - . - 4\n"
    "1:\n"
    ".byte 0x31,0xDB\n"
    "mov fake_pit_blank(%rip), %rcx\n"
    "and $1, %bl\n"
    "movzbl %bl, %eax\n"
    "add $0x48, %rsp\n"
    "pop %rdi\npop %rsi\npop %rbx\npop %rbp\npop %r12\npop %r15\n"
    "ret\n"
    ".seh_endproc\n"
    ".globl fake_pit_rank\n"
    ".def fake_pit_rank; .scl 2; .type 32; .endef\n"
    ".seh_proc fake_pit_rank\n"
    "fake_pit_rank:\n"
    "push %rsi\n.seh_pushreg %rsi\n"
    "push %rdi\n.seh_pushreg %rdi\n"
    "push %rbp\n.seh_pushreg %rbp\n"
    "push %rbx\n.seh_pushreg %rbx\n"
    "sub $0x688, %rsp\n.seh_stackalloc 0x688\n"
    ".seh_savexmm %xmm6, 0x670\n"
    ".seh_endprologue\n"
    "vxorps %xmm6, %xmm6, %xmm6\n"
    "vmovaps %xmm6, 0x670(%rsp)\n"
    "mov %rcx, %rbx\n"
    "mov %rdx, %rsi\n"
    "mov %r8d, %edi\n"
    "lea fake_pit_blank(%rip), %rax\n"
    "mov 0xa8(%rax), %ebp\n"
    "xor 0xb0(%rax), %ebp\n"
    "movzbl 0xc4(%rsi), %eax\n"
    "mov 0x148(%rbx), %rcx\n"
    ".byte 0x48,0x8B,0x04,0xC1,0x8B,0x70,0x48,0x33,0x70,0x50\n"
    "mov fake_pit_list(%rip), %rcx\n"
    "mov 8(%rcx), %rax\n"
    "mov 0x10(%rcx), %rcx\n"
    "cmp %rcx, %rax\n"
    "je 6f\n"
    "vmovd %ebp, %xmm0\n"
    "xor %ebp, %ebp\n"
    "jmp 5f\n"
    ".byte 0x48,0x83,0xC0,0x08,0x48,0x39,0xC8,0x74,0x2F\n"
    "5:\n"
    ".byte 0xC5,0xFA,0x10,0x48,0x04\n"
    ".byte 0xC5,0xF2,0x5C,0xCE,0xC5,0xF8,0x2E,0xC1,0x72,0xE8,0x0F,0xB6,0x10,0x4C,0x8B,0x83,0x48,0x01,0x00,0x00\n"
    ".byte 0x49,0x8B,0x14,0xD0,0x44,0x8B,0x42,0x48,0x44,0x33,0x42,0x50,0x44,0x39,0xC6,0x83,0xD5,0x00,0xEB,0xCA\n"
    "6:\n"
    ".byte 0x31,0xED\n"
    "mov fake_pit_list(%rip), %rcx\n"
    "sub %esi, %edi\n"
    "cmp %edi, %ebp\n"
    ".byte 0x0F,0x9D,0xC0\n"
    ".byte 0xC5,0xF8,0x28,0xB4,0x24,0x70,0x06,0x00,0x00\n"
    "add $0x688, %rsp\n"
    "pop %rbx\npop %rbp\npop %rdi\npop %rsi\n"
    "ret\n"
    ".seh_endproc\n"
    ".section .drectve\n"
    ".ascii \" -export:fake_pit_eval -export:fake_pit_rank -export:fake_pit_stub\"\n"
    ".text\n");
uint64_t fake_pit_eval(void *session, void *record, void *begin, void *end);
uint64_t fake_pit_rank(void *session, void *record, uint32_t free_slots);


static uint8_t pit_module[0x200], pit_record[0xd0], pit_slot[4][0x60];
static void *pit_table[4];
static uint64_t pit_rank_list[3];
static uint8_t pit_entries[3][8];

static void pit_setup(uint8_t own, int own_slot_missing)
{
    memset(pit_module, 0, sizeof pit_module);
    memset(pit_record, 0, sizeof pit_record);
    memset(pit_slot, 0, sizeof pit_slot);
    memset(pit_entries, 0, sizeof pit_entries);
    uint32_t positions[4] = {5, 3, 0, 9};
    for (int i = 0; i < 4; i++) {
        *(uint32_t *)(pit_slot[i] + 0x48) = positions[i];
        pit_table[i] = pit_slot[i];
    }
    pit_table[2] = NULL;
    (void)own_slot_missing;
    *(void ***)(pit_module + 0x148) = pit_table;
    pit_record[0xc4] = own;
}

static void pit_listed(const uint8_t *vehicles, int n)
{
    float one = 1.0f;
    for (int i = 0; i < n; i++) {
        pit_entries[i][0] = vehicles[i];
        memcpy(pit_entries[i] + 4, &one, sizeof one);
    }
    pit_rank_list[0] = 0;
    pit_rank_list[1] = (uint64_t)(uintptr_t)pit_entries;
    pit_rank_list[2] = (uint64_t)(uintptr_t)(pit_entries + n);
    fake_pit_list = (uint64_t)(uintptr_t)pit_rank_list;
}

static uint64_t pit_eval(uint8_t own, const uint8_t *vehicles, int n)
{
    pit_setup(own, 0);
    pit_listed(vehicles, n);
    return fake_pit_eval(pit_module, pit_record, pit_entries, pit_entries + n) & 1;
}

static uint64_t pit_rank(uint8_t own, const uint8_t *vehicles, int n)
{
    pit_setup(own, 0);
    pit_listed(vehicles, n);
    return fake_pit_rank(pit_module, pit_record, 6) & 0xff;
}

static void test_pit_slots(int guard)
{
    uint8_t ahead[] = {1}, behind[] = {3}, hole[] = {2}, hole_first[] = {2, 1}, hole_behind[] = {2, 3};
    expect(pit_eval(0, ahead, 1) == 1, "pit slots: an evaluation with every slot present sees a car ahead");
    expect(pit_eval(0, behind, 1) == 0, "pit slots: an evaluation with every slot present sees a car behind");
    expect(pit_eval(0, ahead, 0) == 0, "pit slots: an empty pit list answers no");
    expect(pit_rank(0, behind, 1) == 1, "pit slots: the rank evaluator with every slot present counts a car behind it");
    expect(pit_rank(0, ahead, 1) == 0, "pit slots: the rank evaluator counts no car for one ahead of it");
    if (!guard) {
        printf("      pit slots: guard off, the null slot cases are not run\n");
        return;
    }
    expect(pit_eval(0, hole, 1) == 0, "pit slots: a listed car without a slot is skipped and the answer is no");
    expect(pit_eval(0, hole_first, 2) == 1, "pit slots: a listed car without a slot is skipped and the next one is judged");
    expect(pit_eval(0, hole_behind, 2) == 0, "pit slots: a skipped car does not stop the rest of the list from being judged");
    expect(pit_eval(2, ahead, 1) == 0, "pit slots: a car without a slot of its own answers no");
    expect(pit_rank(0, hole, 1) == 0, "pit slots: the rank evaluator skips a listed car without a slot");
    expect(pit_rank(0, hole_behind, 2) == 1, "pit slots: the rank evaluator judges the cars after a skipped one");
    expect(pit_rank(2, behind, 1) == 0, "pit slots: the rank evaluator answers no for a car without a slot of its own");
}

// Career_Weekends_BuildResultNodes's record read, copied byte for byte from the Steam build (0x14181b854..0x14181b866),
// in a small frame that takes the TrackDataSystem items in rcx and the track id in rdx. The load is items[track*0x10+8]
// then [record+0x2c0]; it returns that value, 0 for a record that is not there once core's guard has skipped the unchecked
// read. Core's weekends guard resolves site_Career_WeekendsTrackRecordLoad here at +0x13.
__asm__(
    ".section .xtls,\"xr\"\n"
    ".globl fake_weekends_record\n"
    ".def fake_weekends_record; .scl 2; .type 32; .endef\n"
    ".seh_proc fake_weekends_record\n"
    "fake_weekends_record:\n"
    ".byte 0x57\n.seh_pushreg %rdi\n"
    ".seh_endprologue\n"
    ".byte 0x48,0x89,0xD7,0x48,0x89,0xC8,0x48,0x89,0xF9,0x48,0xC1,0xE1,0x04\n"
    ".byte 0x48,0x8B,0x44,0x08,0x08,0x48,0x8B,0x80,0xC0,0x02,0x00,0x00,0x48,0x8D,0x48,0x08,0x48,0x85,0xC0\n"
    ".byte 0x5F,0xC3\n"
    ".seh_endproc\n"
    ".section .drectve\n"
    ".ascii \" -export:fake_weekends_record\"\n"
    ".text\n");
uint64_t fake_weekends_record(void *items, uint64_t track);

static uint8_t weekends_record[0x2c8];
static uint8_t weekends_node[0x10];
static uint64_t weekends_items[8 * 2];

static void weekends_setup(void)
{
    *(uint64_t *)(weekends_record + 0x2c0) = (uint64_t)weekends_node;
    weekends_items[3 * 2 + 1] = (uint64_t)weekends_record;
}

static void test_weekends(void)
{
    weekends_setup();
    uint64_t r = fake_weekends_record(weekends_items, 3);
    expect(r == (uint64_t)weekends_node, "weekends: a round with track data reads its record as the game does");
    r = fake_weekends_record(weekends_items, 1);
    uint64_t again = fake_weekends_record(weekends_items, 1);
    expect(r == 0 && again == 0, "weekends: a round whose track has no record reads as empty");
}

static void crash_weekends(void)
{
    SetErrorMode(SEM_NOGPFAULTERRORBOX);
    weekends_setup();
    printf("      weekends: reading a round whose track has no record\n");
    fflush(stdout);
    uint64_t r = fake_weekends_record(weekends_items, 1);
    printf("      weekends: the round returned %llu\n", (unsigned long long)r);
    fflush(stdout);
}

// The three unchecked loads of Career_TeamHqAdvance_BuildTrackNode, with the bytes around them copied from the Steam build
// (0x1419b44e5..0x1419b44f6, 0x1419b4788..0x1419b4799, 0x1419b47a6..0x1419b47b6), in a small frame taking the track record
// (rbx) in rcx, the TrackDataTrack (r14) in rdx, the calendar event (rdi, track byte at +0x18) in r8 and a three-slot
// result in r9. The empty node handle (r13) is 0xe3e7; the result holds rdx after each record read and rcx after the track
// read, the game's own fallback for an empty node. Core's Team HQ track guard finds the three loads by their byte
// patterns: the record handle read at +0x1d, the second record read at +0x37 and the track read at +0x52.
__asm__(
    ".section .xtls,\"xr\"\n"
    ".globl fake_teamhq_node\n"
    ".def fake_teamhq_node; .scl 2; .type 32; .endef\n"
    ".seh_proc fake_teamhq_node\n"
    "fake_teamhq_node:\n"
    ".byte 0x53\n.seh_pushreg %rbx\n"
    ".byte 0x57\n.seh_pushreg %rdi\n"
    ".byte 0x41,0x55\n.seh_pushreg %r13\n"
    ".byte 0x41,0x56\n.seh_pushreg %r14\n"
    ".seh_endprologue\n"
    ".byte 0x48,0x89,0xCB,0x49,0x89,0xD6,0x4C,0x89,0xC7,0x41,0xBD,0xE7,0xE3,0x00,0x00\n"
    ".byte 0x48,0x85,0xDB,0x75,0x03\n"
    ".byte 0x45,0x31,0xF6\n"
    ".byte 0x48,0x8B,0x83,0xC0,0x02,0x00,0x00,0x48,0x8D,0x50,0x08,0x48,0x85,0xC0,0x49,0x0F,0x44,0xD5\n"
    ".byte 0x49,0x89,0x11\n"
    "call fake_teamhq_stub\n"
    ".byte 0x48,0x8B,0x83,0xC0,0x02,0x00,0x00,0x48,0x8D,0x50,0x08,0x48,0x85,0xC0,0x49,0x0F,0x44,0xD5\n"
    ".byte 0x49,0x89,0x51,0x08\n"
    "call fake_teamhq_stub\n"
    ".byte 0x49,0x8B,0x46,0x30,0x48,0x8D,0x48,0x08,0x48,0x85,0xC0,0x49,0x0F,0x44,0xCD\n"
    ".byte 0x49,0x89,0x49,0x10\n"
    ".byte 0x41,0x5E,0x41,0x5D,0x5F,0x5B,0xC3\n"
    ".seh_endproc\n"
    "fake_teamhq_stub:\n"
    ".byte 0xC3\n"
    ".section .drectve\n"
    ".ascii \" -export:fake_teamhq_node\"\n"
    ".text\n");
void fake_teamhq_node(void *record, void *track, void *event, uint64_t *out);

// The unchecked record load of Career_TeamHqAdvanceVM_Initialise, with the bytes around it copied from the Steam build
// (0x1422bf171..0x1422bf1a2), in a small frame taking the career entity (rdi, TrackDataSystem at +0xf58) in rcx, the
// calendar event (r15, track byte at +0xb0) in rdx and a one-slot result in r8. The result holds rdx after the read, the
// game's own fallback for an empty node (fake_teamhq_empty). Core's Team HQ track guard finds the load by its byte
// pattern, the record read at +0x2b.
__asm__(
    ".section .xtls,\"xr\"\n"
    ".globl fake_teamhq_vm\n"
    ".def fake_teamhq_vm; .scl 2; .type 32; .endef\n"
    ".seh_proc fake_teamhq_vm\n"
    "fake_teamhq_vm:\n"
    ".byte 0x57\n.seh_pushreg %rdi\n"
    ".byte 0x41,0x57\n.seh_pushreg %r15\n"
    ".seh_endprologue\n"
    ".byte 0x48,0x89,0xCF,0x49,0x89,0xD7\n"
    ".byte 0x41,0x0F,0xB6,0x87,0xB0,0x00,0x00,0x00\n"
    ".byte 0x48,0x8B,0x8F,0x58,0x0F,0x00,0x00\n"
    ".byte 0x48,0x8B,0x49,0x28\n"
    ".byte 0x48,0xC1,0xE0,0x04\n"
    ".byte 0x48,0x8B,0x44,0x01,0x08\n"
    ".byte 0x48,0x8B,0x80,0xC0,0x02,0x00,0x00\n"
    ".byte 0x48,0x8D,0x48,0x08,0x48,0x85,0xC0\n"
    "leaq fake_teamhq_empty(%rip), %rdx\n"
    ".byte 0x48,0x0F,0x45,0xD1\n"
    ".byte 0x49,0x89,0x10\n"
    ".byte 0x41,0x5F,0x5F,0xC3\n"
    ".seh_endproc\n"
    ".globl fake_teamhq_empty\n"
    "fake_teamhq_empty:\n"
    ".quad 0\n"
    ".section .drectve\n"
    ".ascii \" -export:fake_teamhq_vm -export:fake_teamhq_empty\"\n"
    ".text\n");
void fake_teamhq_vm(void *career, void *event, uint64_t *out);
extern uint8_t fake_teamhq_empty[];

// The unchecked record load of Career_TeamHqMyTeamVM_Initialise, with the bytes around it copied from the Steam build
// (0x1423296ad..0x1423296e4), in a small frame taking the career entity (rbx, TrackDataSystem at +0xf58) in rcx, the
// calendar event (rdi, track byte at +0xb0) in rdx and a one-slot result in r8. The result holds rdx after the read, the
// game's own fallback for an empty node (fake_teamhq_empty). Core's Team HQ track guard finds the load by its byte
// pattern, the record read at +0x2b.
__asm__(
    ".section .xtls,\"xr\"\n"
    ".globl fake_teamhq_myteam\n"
    ".def fake_teamhq_myteam; .scl 2; .type 32; .endef\n"
    ".seh_proc fake_teamhq_myteam\n"
    "fake_teamhq_myteam:\n"
    ".byte 0x53\n.seh_pushreg %rbx\n"
    ".byte 0x57\n.seh_pushreg %rdi\n"
    ".seh_endprologue\n"
    ".byte 0x48,0x89,0xCB,0x48,0x89,0xD7\n"
    ".byte 0x0F,0xB6,0x87,0xB0,0x00,0x00,0x00\n"
    ".byte 0x48,0x8B,0x8B,0x58,0x0F,0x00,0x00\n"
    ".byte 0x48,0x8B,0x49,0x28\n"
    ".byte 0x48,0xC1,0xE0,0x04\n"
    ".byte 0x48,0x8B,0x44,0x01,0x08\n"
    ".byte 0x48,0x8B,0x80,0xC0,0x02,0x00,0x00\n"
    ".byte 0x48,0x8D,0x48,0x08,0x48,0x85,0xC0\n"
    "leaq fake_teamhq_empty(%rip), %rdx\n"
    ".byte 0x48,0x0F,0x45,0xD1\n"
    ".byte 0x49,0x89,0x10\n"
    ".byte 0x5F,0x5B,0xC3\n"
    ".seh_endproc\n"
    ".section .drectve\n"
    ".ascii \" -export:fake_teamhq_myteam\"\n"
    ".text\n");
void fake_teamhq_myteam(void *career, void *event, uint64_t *out);

static uint8_t vm_career[0x1000], vm_system[0x40], vm_event[0xc0], vm_record[0x2c8];
static uint8_t *vm_items[4];

static void teamhq_vm_setup(uint8_t index, int with_record)
{
    memset(vm_record, 0, sizeof vm_record);
    *(uint64_t *)(vm_record + 0x2c0) = 0x2c0c0de;
    memset(vm_items, 0, sizeof vm_items);
    vm_items[index * 2 + 1] = with_record ? vm_record : NULL;
    *(void **)(vm_system + 0x28) = vm_items;
    *(void **)(vm_career + 0xf58) = vm_system;
    vm_event[0xb0] = index;
}

static uint8_t teamhq_record[0x2c8], teamhq_track[0x40], teamhq_event[0x20];

static void teamhq_setup(uint8_t track_id)
{
    memset(teamhq_record, 0, sizeof teamhq_record);
    memset(teamhq_track, 0, sizeof teamhq_track);
    teamhq_record[1] = track_id;
    *(uint64_t *)(teamhq_record + 0x2c0) = 0x2c0c0de;
    *(uint64_t *)(teamhq_track + 0x30) = 0x7ac0de;
    teamhq_event[0x18] = track_id;
}

static void test_teamhq(void)
{
    uint64_t out[3] = {0, 0, 0};
    teamhq_setup(9);
    fake_teamhq_node(teamhq_record, teamhq_track, teamhq_event, out);
    expect(out[0] == 0x2c0c0de + 8 && out[1] == 0x2c0c0de + 8 && out[2] == 0x7ac0de + 8,
           "teamhq: an event with a record and a track reads both as the game does");
    teamhq_setup(6);
    fake_teamhq_node(teamhq_record, NULL, teamhq_event, out);
    expect(out[0] == 0x2c0c0de + 8 && out[2] == 0xe3e7, "teamhq: an event whose track has no track data takes the empty node handle");
    fake_teamhq_node(teamhq_record, NULL, teamhq_event, out);
    teamhq_setup(1);
    fake_teamhq_node(NULL, NULL, teamhq_event, out);
    expect(out[0] == 0xe3e7 && out[1] == 0xe3e7 && out[2] == 0xe3e7, "teamhq: an event whose track has no record takes the empty node handle");

    uint64_t vm = 0;
    teamhq_vm_setup(1, 1);
    fake_teamhq_vm(vm_career, vm_event, &vm);
    expect(vm == 0x2c0c0de + 8, "teamhq: a view model event with a record reads it as the game does");
    teamhq_vm_setup(1, 0);
    fake_teamhq_vm(vm_career, vm_event, &vm);
    expect(vm == (uint64_t)(uintptr_t)fake_teamhq_empty, "teamhq: a view model event whose track has no record takes the empty node");
    teamhq_vm_setup(1, 1);
    fake_teamhq_myteam(vm_career, vm_event, &vm);
    expect(vm == 0x2c0c0de + 8, "teamhq: a my team view model event with a record reads it as the game does");
    teamhq_vm_setup(1, 0);
    fake_teamhq_myteam(vm_career, vm_event, &vm);
    expect(vm == (uint64_t)(uintptr_t)fake_teamhq_empty, "teamhq: a my team view model event whose track has no record takes the empty node");
    fake_teamhq_vm(vm_career, vm_event, &vm);
}

static void crash_teamhq(void)
{
    SetErrorMode(SEM_NOGPFAULTERRORBOX);
    uint64_t out[3] = {0, 0, 0};
    teamhq_setup(6);
    printf("      teamhq: reading an event whose track has no track data\n");
    fflush(stdout);
    fake_teamhq_node(teamhq_record, NULL, teamhq_event, out);
    printf("      teamhq: the event returned %llx\n", (unsigned long long)out[2]);
    fflush(stdout);
    uint64_t vm = 0;
    teamhq_vm_setup(1, 0);
    printf("      teamhq: reading a view model event whose track has no record\n");
    fflush(stdout);
    fake_teamhq_vm(vm_career, vm_event, &vm);
    printf("      teamhq: the view model event took the empty node %d\n", vm == (uint64_t)(uintptr_t)fake_teamhq_empty);
    fflush(stdout);
    vm = 0;
    teamhq_vm_setup(1, 0);
    printf("      teamhq: reading a my team view model event whose track has no record\n");
    fflush(stdout);
    fake_teamhq_myteam(vm_career, vm_event, &vm);
    printf("      teamhq: the my team view model event took the empty node %d\n", vm == (uint64_t)(uintptr_t)fake_teamhq_empty);
    fflush(stdout);
}

// The race length text body of the career activity report (Steam .data1 0x151948840: virtual +0x80 and +0x10 of the ui
// variable table at championship +0x1030, then three u32 reads through 0x140bc9ef0 at +0x64, +0x76 and +0x7d), with the
// instructions around the reads copied from the Steam build, in a frame taking the championship in rcx. Core's activity
// resume guard finds the two reads and the epilogue (lea rsi ; mov rax,rsi ; add rsp,0x20) by byte patterns in .data1.
__asm__(
    ".section .data1,\"dwx\"\n"
    ".globl fake_activity_length\n"
    ".def fake_activity_length; .scl 2; .type 32; .endef\n"
    ".seh_proc fake_activity_length\n"
    "fake_activity_length:\n"
    ".byte 0x41,0x56\n.seh_pushreg %r14\n"
    ".byte 0x56\n.seh_pushreg %rsi\n"
    ".byte 0x57\n.seh_pushreg %rdi\n"
    ".byte 0x55\n.seh_pushreg %rbp\n"
    ".byte 0x53\n.seh_pushreg %rbx\n"
    ".byte 0x48,0x83,0xEC,0x20\n.seh_stackalloc 0x20\n"
    ".seh_endprologue\n"
    ".byte 0x48,0x89,0xCF\n"
    ".byte 0x48,0x8B,0x81,0x30,0x10,0x00,0x00\n"
    ".byte 0x48,0x81,0xC7,0x30,0x10,0x00,0x00\n"
    ".byte 0x48,0x89,0xF9\n"
    ".byte 0xFF,0x90,0x80,0x00,0x00,0x00\n"
    ".byte 0x48,0x8D,0x35\n.long fake_activity_full-(.+4)\n"
    ".byte 0x48,0x85,0xC0\n"
    "je 2f\n"
    ".byte 0x48,0x8B,0x98,0x90,0x02,0x00,0x00\n"
    ".byte 0x48,0x2B,0x98,0x88,0x02,0x00,0x00\n"
    ".byte 0x48,0xC1,0xEB,0x03\n"
    ".byte 0x48,0x8B,0x07\n"
    ".byte 0x48,0x8D,0x15\n.long fake_activity_full-(.+4)\n"
    ".byte 0x48,0x89,0xF9\n"
    ".byte 0x41,0xB8,0x01,0x00,0x00,0x00\n"
    ".byte 0xFF,0x50,0x10\n"
    ".byte 0x48,0x89,0xC7\n"
    ".byte 0x48,0x89,0xC1\n"
    ".byte 0xB2,0x02\n"
    ".byte 0xE8\n.long fake_activity_get-(.+4)\n"
    ".byte 0x44,0x8B,0x30\n"
    ".byte 0x48,0x89,0xF9\n"
    ".byte 0xB2,0x01\n"
    ".byte 0xE8\n.long fake_activity_get-(.+4)\n"
    ".byte 0x8B,0x28\n"
    ".byte 0x48,0x89,0xF9\n"
    ".byte 0x31,0xD2\n"
    ".byte 0xE8\n.long fake_activity_get-(.+4)\n"
    ".byte 0x41,0x39,0xDE\n"
    "jae 3f\n"
    ".byte 0x39,0xDD\n"
    "jae 4f\n"
    "jmp 2f\n"
    "3:\n"
    ".byte 0x48,0x8D,0x35\n.long fake_activity_short-(.+4)\n"
    "jmp 2f\n"
    "4:\n"
    ".byte 0x48,0x8D,0x35\n.long fake_activity_medium-(.+4)\n"
    "2:\n"
    ".byte 0x48,0x89,0xF0\n"
    ".byte 0x48,0x83,0xC4,0x20\n"
    ".byte 0x5B,0x5D,0x5F,0x5E,0x41,0x5E,0xC3\n"
    ".seh_endproc\n"
    ".text\n");
const char *fake_activity_length(void *championship);

const char fake_activity_full[] = "full", fake_activity_short[] = "short", fake_activity_medium[] = "medium";

static uint8_t activity_tracklist[0x298];
static uint64_t activity_vt[0x20];
static uint32_t activity_fields[3];
static int activity_has_group;

__attribute__((ms_abi)) void *fake_activity_get(void *group, uint8_t index)
{
    (void)group;
    return activity_has_group ? (void *)&activity_fields[index] : NULL;
}

__attribute__((ms_abi)) static void *activity_tracklist_of(void *table)
{
    (void)table;
    return activity_tracklist;
}

__attribute__((ms_abi)) static void *activity_group_of(void *table, const char *name, int mode)
{
    (void)name;
    (void)mode;
    return table;
}

static uint8_t activity_championship[0x1040];

static const char *activity_run(int group, uint32_t length_field, uint32_t kind_field, uint32_t count)
{
    activity_has_group = group;
    activity_fields[2] = length_field;
    activity_fields[1] = kind_field;
    *(uint64_t *)(activity_tracklist + 0x288) = 0x1000;
    *(uint64_t *)(activity_tracklist + 0x290) = 0x1000 + 8ull * count;
    activity_vt[0x10 / 8] = (uint64_t)(uintptr_t)activity_group_of;
    activity_vt[0x80 / 8] = (uint64_t)(uintptr_t)activity_tracklist_of;
    *(uint64_t *)(activity_championship + 0x1030) = (uint64_t)(uintptr_t)activity_vt;
    return fake_activity_length(activity_championship);
}

static void test_activity(void)
{
    expect(!strcmp(activity_run(1, 1, 1, 4), "full"), "activity: a championship with its fields reads them as the game does");
    expect(!strcmp(activity_run(1, 9, 1, 4), "short") && !strcmp(activity_run(1, 1, 9, 4), "medium"),
           "activity: the fields past the track list give the short and medium texts");
    expect(!strcmp(activity_run(0, 0, 0, 4), "full"), "activity: a championship whose group is missing keeps the default text");
    activity_run(0, 0, 0, 4);
}

static void crash_activity(void)
{
    SetErrorMode(SEM_NOGPFAULTERRORBOX);
    printf("      activity: reading a championship whose track is not in the track list\n");
    fflush(stdout);
    const char *text = activity_run(0, 0, 0, 4);
    printf("      activity: the report returned %s\n", text);
    fflush(stdout);
}

// The tail of the car setup screen handler Game_CarSetupVm_Initialise (0x140c1b904..0x140c1b945), copied byte for byte
// from the Steam build in a small frame taking the vm in rcx: the setup data test (mov rax,[vm+0x268] ; je), the car load
// (mov rax,[rax+0x28]) and the handling version compare with its jne at +0x18, then the two differential range updates,
// here fake_setup_diff_a and fake_setup_diff_b counting their calls, and the game's following mov dword [rsp+0x30],4. Core's
// setup screen car guard finds the compare by its byte pattern.
__asm__(
    ".section .xtls,\"xr\"\n"
    ".globl fake_setup_vm\n"
    ".def fake_setup_vm; .scl 2; .type 32; .endef\n"
    ".seh_proc fake_setup_vm\n"
    "fake_setup_vm:\n"
    ".byte 0x56\n.seh_pushreg %rsi\n"
    ".byte 0x48,0x83,0xEC,0x40\n.seh_stackalloc 0x40\n"
    ".seh_endprologue\n"
    ".byte 0x48,0x89,0xCE\n"
    ".byte 0x48,0x8B,0x86,0x68,0x02,0x00,0x00\n"
    ".byte 0x48,0x85,0xC0\n"
    ".byte 0x74,0x1D\n"
    ".byte 0x48,0x8B,0x40,0x28\n"
    ".byte 0x83,0xB8,0x40,0x9F,0x00,0x00,0x64\n"
    ".byte 0x75,0x10\n"
    ".byte 0x48,0x89,0xF1\n"
    "call fake_setup_diff_a\n"
    ".byte 0x48,0x89,0xF1\n"
    "call fake_setup_diff_b\n"
    ".byte 0xC7,0x44,0x24,0x30,0x04,0x00,0x00,0x00\n"
    ".byte 0x48,0x83,0xC4,0x40\n"
    ".byte 0x5E,0xC3\n"
    ".seh_endproc\n"
    ".section .drectve\n"
    ".ascii \" -export:fake_setup_vm\"\n"
    ".text\n");
void fake_setup_vm(void *vm);

static int setup_diff_calls;
void fake_setup_diff_a(void) { setup_diff_calls++; }
void fake_setup_diff_b(void) { setup_diff_calls += 10; }

static uint8_t setup_vm_mem[0x300], setup_data[0x40], setup_car[0x9f60];

static void setup_vm_prepare(int has_data, int has_car, uint32_t version)
{
    memset(setup_vm_mem, 0, sizeof setup_vm_mem);
    memset(setup_data, 0, sizeof setup_data);
    memset(setup_car, 0, sizeof setup_car);
    *(uint32_t *)(setup_car + 0x9f40) = version;
    *(void **)(setup_data + 0x28) = has_car ? (void *)setup_car : NULL;
    *(void **)(setup_vm_mem + 0x268) = has_data ? (void *)setup_data : NULL;
    setup_diff_calls = 0;
}

static void test_setup_vm(void)
{
    setup_vm_prepare(1, 1, 100);
    fake_setup_vm(setup_vm_mem);
    expect(setup_diff_calls == 11, "setups: a setup screen with a version 100 car updates both differential ranges");
    setup_vm_prepare(1, 1, 90);
    fake_setup_vm(setup_vm_mem);
    expect(setup_diff_calls == 0, "setups: a setup screen with an older car updates neither range");
    setup_vm_prepare(0, 0, 0);
    fake_setup_vm(setup_vm_mem);
    expect(setup_diff_calls == 0, "setups: a setup screen with no setup data updates neither range");
}

static void crash_setup_vm(void)
{
    SetErrorMode(SEM_NOGPFAULTERRORBOX);
    setup_vm_prepare(1, 0, 0);
    printf("      setups: refreshing a setup screen whose setup data has no car\n");
    fflush(stdout);
    fake_setup_vm(setup_vm_mem);
    printf("      setups: the refresh returned with %d range updates\n", setup_diff_calls);
    fflush(stdout);
}

// The pairing step of Sess_Strategy_UpdateTeammatePairs, with the bytes of the size test and the equal-flag test copied
// from the Steam build (0x1436abc9e..0x1436abcf9) in a small frame taking the vector of a team's car records in
// rcx (begin) and rdx (end), a pointer to the team entry in r8 and the session in r9. The sort 0x1436b36f0 is
// fake_pairs_sort: it orders the vector by the byte at +0x80 and faults when a record's byte at +0x81 is 0, where the
// game throws through Ne_Std_ThrowException. Core's teammate pairs guard finds the size test by its byte pattern (+0x2e).
__asm__(
    ".section .xtls,\"xr\"\n"
    ".globl fake_teammate_pairs\n"
    ".def fake_teammate_pairs; .scl 2; .type 32; .endef\n"
    ".seh_proc fake_teammate_pairs\n"
    "fake_teammate_pairs:\n"
    ".byte 0x53\n.seh_pushreg %rbx\n"
    ".byte 0x56\n.seh_pushreg %rsi\n"
    ".byte 0x41,0x54\n.seh_pushreg %r12\n"
    ".byte 0x41,0x55\n.seh_pushreg %r13\n"
    ".byte 0x55\n.seh_pushreg %rbp\n"
    ".byte 0x41,0x56\n.seh_pushreg %r14\n"
    ".byte 0x48,0x83,0xEC,0x28\n.seh_stackalloc 0x28\n"
    ".seh_endprologue\n"
    ".byte 0x49,0x89,0xCC,0x48,0x89,0xD5,0x4D,0x89,0xC6,0x4C,0x89,0xCE\n"
    ".byte 0x49,0x89,0xED,0x4D,0x29,0xE5\n"
    ".byte 0x49,0x83,0xFD,0x10\n"
    ".byte 0x0F,0x82\n.long fake_tp_done - . - 4\n"
    ".byte 0x49,0x8B,0x04,0x24,0x49,0x8B,0x54,0x24,0x08,0x0F,0xB6,0x88,0x81,0x00,0x00,0x00\n"
    ".byte 0x84,0xC9,0x41,0x0F,0x95,0xC0,0x0F,0xB6,0x82,0x81,0x00,0x00,0x00,0x84,0xC0,0x0F,0x95,0xC2,0x44,0x30,0xC2\n"
    ".byte 0x0F,0x85\n.long fake_tp_done - . - 4\n"
    ".byte 0x49,0xC1,0xFD,0x03\n"
    ".byte 0x84,0xC9\n.byte 0x74\n.byte fake_tp_done - . - 1\n"
    ".byte 0x84,0xC0\n.byte 0x74\n.byte fake_tp_done - . - 1\n"
    ".byte 0x4C,0x89,0xE1,0x48,0x89,0xEA,0x4D,0x89,0xE8\n"
    "call fake_pairs_sort\n"
    "fake_tp_done:\n"
    ".byte 0x48,0x83,0xC4,0x28\n"
    ".byte 0x41,0x5E,0x5D,0x41,0x5D,0x41,0x5C,0x5E,0x5B,0xC3\n"
    ".seh_endproc\n"
    ".section .drectve\n"
    ".ascii \" -export:fake_teammate_pairs -export:fake_pairs_sort\"\n"
    ".text\n");
void fake_teammate_pairs(void *begin, void *end, void *team_entry, void *session);

static volatile uintptr_t pairs_null;

__attribute__((noinline)) void fake_pairs_sort(uint8_t **begin, uint8_t **end, uint64_t count)
{
    for (uint8_t **p = begin; p < end; p++)
        if (!(*p)[0x81])
            *(volatile int *)pairs_null = (int)count;
    for (uint8_t **a = begin; a < end; a++)
        for (uint8_t **b = a + 1; b < end; b++)
            if ((*b)[0x80] < (*a)[0x80]) {
                uint8_t *t = *a;
                *a = *b;
                *b = t;
            }
}

static uint8_t pairs_cars[3][0x90], pairs_team[0x30], *pairs_team_ptr = pairs_team, pairs_session[8];

static void pairs_setup(uint8_t third_valid)
{
    memset(pairs_cars, 0, sizeof pairs_cars);
    memset(pairs_team, 0, sizeof pairs_team);
    *(uint32_t *)(pairs_team + 0x28) = 6;
    pairs_cars[0][0x80] = 5;
    pairs_cars[0][0x81] = 1;
    pairs_cars[1][0x80] = 2;
    pairs_cars[1][0x81] = 1;
    pairs_cars[2][0x80] = 9;
    pairs_cars[2][0x81] = third_valid;
}

static void test_teammate_pairs(void)
{
    uint8_t *v[3] = {pairs_cars[0], pairs_cars[1], pairs_cars[2]};
    pairs_setup(0);
    fake_teammate_pairs(v, v + 2, &pairs_team_ptr, pairs_session);
    expect(v[0] == pairs_cars[1] && v[1] == pairs_cars[0],
           "pairs: a team with two cars is sorted as the game does");
    uint8_t *w[3] = {pairs_cars[0], pairs_cars[1], pairs_cars[2]};
    fake_teammate_pairs(w, w + 3, &pairs_team_ptr, pairs_session);
    expect(w[0] == pairs_cars[1] && w[1] == pairs_cars[0] && w[2] == pairs_cars[2],
           "pairs: a team with a third car pairs its first two and leaves the third unpaired");
    uint8_t *x[3] = {pairs_cars[0], pairs_cars[1], pairs_cars[2]};
    pairs_setup(1);
    fake_teammate_pairs(x, x + 3, &pairs_team_ptr, pairs_session);
    expect(x[0] == pairs_cars[1] && x[1] == pairs_cars[0] && x[2] == pairs_cars[2],
           "pairs: a third car with a valid record is left unpaired too");
}

static void crash_pairs(void)
{
    SetErrorMode(SEM_NOGPFAULTERRORBOX);
    uint8_t *v[3] = {pairs_cars[0], pairs_cars[1], pairs_cars[2]};
    pairs_setup(0);
    printf("      pairs: sorting a team with three cars\n");
    fflush(stdout);
    fake_teammate_pairs(v, v + 3, &pairs_team_ptr, pairs_session);
    printf("      pairs: the team was paired\n");
    fflush(stdout);
}

// The two entrant-list lookups behind the front-end tile populate, copied byte for byte from the Steam build in small
// frames, the calls and the RIP-relative operands pointed at stand-ins. fake_list_first_id is
// FeTile_Get_ListFirstEntrantId (0x141738eb0..0x141738f6c) taking the item in rcx and the result in rdx: it scans the
// slot array of the entrant lists for the label and has no end check. fake_list_usable is FeTile_Check_EntrantListUsable
// (0x142428790..0x1424288e9) taking the tile context in rcx and the label in rdx: a label with no database row makes
// the field lookup return the miss pointer, whose first qword is not a canonical address. Core's tile list guard resolves
// site_FeTile_ListScanStart at fake_list_first_id+0x6f, site_FeTile_ListScanNone at +0xb0, site_FeTile_ListRowEntry at
// fake_list_usable+0x11, site_FeTile_ListRowFieldLoad at +0xa2 and site_FeTile_ListRowNone at +0x147.
__asm__(
    ".section .xtls,\"xr\"\n"
    ".globl fake_list_first_id\n"
    ".def fake_list_first_id; .scl 2; .type 32; .endef\n"
    ".seh_proc fake_list_first_id\n"
    "fake_list_first_id:\n"
    ".byte 0x41,0x56\n"
    ".seh_pushreg %r14\n"
    ".byte 0x56\n"
    ".seh_pushreg %rsi\n"
    ".byte 0x57\n"
    ".seh_pushreg %rdi\n"
    ".byte 0x53\n"
    ".seh_pushreg %rbx\n"
    ".byte 0x48,0x83,0xec,0x28\n"
    ".seh_stackalloc 0x28\n.seh_endprologue\n"
    ".byte 0x48,0x89,0xd6,0xc6,0x42,0x04,0x00,0x80,0xb9,0xb8,0x01,0x00,0x00,0x00,0x0f,0x84,0x93,0x00,0x00,0x00,0x48,0x8b,0x91,0x98,0x01,0x00,0x00,0x48,0x8b,0x81,0xc8,0x01,0x00,0x00,0x48,0x8d,0x4a,0x09,0x48,0x85,0xd2,0x48,0x8d,0x3d\n"
    ".long fake_ls_empty - . - 4\n"
    ".byte 0x48,0x0f,0x45,0xf9,0x48,0x8b,0x98,0x60,0x05,0x00,0x00,0x48,0x85,0xdb,0x74,0x26,0x48,0x3b,0x98,0x68,0x05,0x00,0x00,0x74,0x1d,0x48,0x83,0x7b,0x08,0x00,0x75,0x16,0x48,0x83,0xc3,0x10,0x0f,0x1f,0x00,0x48,0x83,0x7b,0x08,0x00,0x48,0x8d,0x5b,0x10,0x74,0xf5,0x48,0x83,0xc3,0xf0,0x4c,0x8b,0x73,0x08,0x66,0x66,0x66,0x66,0x2e,0x0f,0x1f,0x84,0x00,0x00,0x00,0x00,0x00,0x49,0x8b,0x46,0x28,0x48,0x8b,0x48,0x10,0x48,0x89,0xfa,0xe8\n"
    ".long fake_ls_strcmp - . - 4\n"
    ".byte 0x85,0xc0,0x74,0x0f,0x48,0x83,0xc3,0x10,0x4c,0x8b,0x73,0x08,0x4d,0x85,0xf6,0x75,0xdf,0xeb,0xf1,0x49,0x8b,0x46,0x10,0x8b,0x40,0x5c,0x89,0x06,0xc6,0x46,0x04,0x01,0x48,0x89,0xf0,0x48,0x83,0xc4,0x28,0x5b,0x5f,0x5e,0x41,0x5e,0xc3\n"
    ".seh_endproc\n"
    ".section .drectve\n"
    ".ascii \" -export:fake_list_first_id\"\n"
    ".text\n");

__asm__(
    ".section .xtls,\"xr\"\n"
    ".globl fake_list_usable\n"
    ".def fake_list_usable; .scl 2; .type 32; .endef\n"
    ".seh_proc fake_list_usable\n"
    "fake_list_usable:\n"
    ".byte 0x41,0x57\n"
    ".seh_pushreg %r15\n"
    ".byte 0x41,0x56\n"
    ".seh_pushreg %r14\n"
    ".byte 0x56\n"
    ".seh_pushreg %rsi\n"
    ".byte 0x57\n"
    ".seh_pushreg %rdi\n"
    ".byte 0x53\n"
    ".seh_pushreg %rbx\n"
    ".byte 0x48,0x81,0xec,0x30,0x04,0x00,0x00\n"
    ".seh_stackalloc 0x430\n.seh_endprologue\n"
    ".byte 0x49,0x89,0xd0,0x48,0x89,0xce,0x48,0x8b,0x99,0x28,0x02,0x00,0x00,0xc7,0x84,0x24,0x20,0x02,0x00,0x00,0xff,0xff,0xff,0xff,0xc6,0x84,0x24,0x24,0x02,0x00,0x00,0x01,0xc5,0xf8,0x28,0x05\n"
    ".long fake_ls_const - . - 4\n"
    ".byte 0xc5,0xf8,0x11,0x44,0x24,0x20,0xc7,0x44,0x24,0x30,0x5f,0x6c,0x69,0x73,0x66,0xc7,0x44,0x24,0x34,0x74,0x73,0xc6,0x44,0x24,0x36,0x00,0x48,0xb8,0x65,0x6e,0x74,0x72,0x61,0x6e,0x74,0x5f,0x48,0x89,0x84,0x24,0x20,0x01,0x00,0x00,0xc7,0x84,0x24,0x28,0x01,0x00,0x00,0x6c,0x69,0x73,0x74,0xc6,0x84,0x24,0x2c,0x01,0x00,0x00,0x00,0x45,0x31,0xf6,0x48,0x8d,0x4c,0x24,0x20,0x48,0x8d,0xbc,0x24,0x28,0x02,0x00,0x00,0x48,0x89,0xfa,0x45,0x31,0xc9,0xe8\n"
    ".long fake_ls_find - . - 4\n"
    ".byte 0x48,0x8d,0x15\n"
    ".long fake_ls_statid - . - 4\n"
    ".byte 0x48,0x89,0xf9,0x45,0x31,0xc0,0xe8\n"
    ".long fake_ls_get - . - 4\n"
    ".byte 0x48,0x8b,0x00,0x48,0x85,0xc0,0x74,0x05,0x44,0x0f,0xb6,0x70,0x08,0x48,0x8b,0x83,0x60,0x05,0x00,0x00,0x48,0x8b,0x8b,0x68,0x05,0x00,0x00,0x48,0x29,0xc1,0x48,0xc1,0xf9,0x04,0x4c,0x39,0xf1,0x76,0x7e,0x49,0xc1,0xe6,0x04,0x4a,0x8b,0x44,0x30,0x08,0x48,0x85,0xc0,0x74,0x70,0x4c,0x8b,0x70,0x10,0x4c,0x8b,0x78,0x18,0x40,0xb7,0x01,0xeb,0x1b,0x84,0xc9,0x74,0x5f,0x48,0x8b,0x03,0x48,0x89,0xd9,0xff,0x50,0x40,0xa9,0xfd,0xff,0xff,0xff,0x74,0x4f,0x49,0x81,0xc6,0x88,0x00,0x00,0x00,0x4d,0x39,0xfe,0x74,0x45,0x45,0x8b,0x46,0x5c,0x48,0x8b,0x8e,0x28,0x02,0x00,0x00,0x48,0x8b,0x96,0x38,0x02,0x00,0x00,0xe8\n"
    ".long fake_ls_entrant - . - 4\n"
    ".byte 0x48,0x85,0xc0,0x74,0xd8,0x48,0x89,0xc3,0x48,0x8b,0x88,0xc8,0x00,0x00,0x00,0x48,0x85,0xc9,0x74,0xb9,0x48,0x8b,0x01,0xff,0x50,0x40,0x0f,0xb6,0x8e,0x2a,0x03,0x00,0x00,0x83,0xf8,0x01,0x75,0xa3,0x84,0xc9,0x75,0xb3,0xeb,0x9d,0x31,0xff,0x89,0xf8,0x48,0x81,0xc4,0x30,0x04,0x00,0x00,0x5b,0x5f,0x5e,0x41,0x5e,0x41,0x5f,0xc3\n"
    ".seh_endproc\n"
    ".section .drectve\n"
    ".ascii \" -export:fake_list_usable\"\n"
    ".text\n");

uint64_t *fake_list_first_id(void *item, void *out);
uint8_t fake_list_usable(void *ctx, const char *label);

const char fake_ls_empty[] = "";
const char fake_ls_statid[] = "stat_id";
__attribute__((aligned(16))) const uint8_t fake_ls_const[16] = {'e', 'n', 't', 'r', 'a', 'n', 't', 's', '.', 'e', 'n', 't', 'r', 'a', 'n', 't'};

int fake_ls_strcmp(const char *a, const char *b)
{
    return strcmp(a, b);
}

void *fake_ls_find(void *table, void *row, const char *label, void *found)
{
    (void)table;
    (void)found;
    *(const char **)row = label;
    return row;
}

static uint64_t ls_miss = 0x08000f000800000eull;
static uint8_t ls_row_record[16] = {0, 0, 0, 0, 0, 0, 0, 0, 2};
static uint64_t ls_row_slot;

uint64_t *fake_ls_get(void *row, const char *name, int def)
{
    (void)name;
    (void)def;
    const char *label = *(const char **)row;
    if (!strcmp(label, "f2_2026"))
        return &ls_miss;
    ls_row_slot = (uint64_t)ls_row_record;
    return &ls_row_slot;
}

void *fake_ls_entrant(void *a, void *b, uint32_t id)
{
    (void)a;
    (void)b;
    (void)id;
    return NULL;
}

static uint8_t ls_entrants[0x88];
static uint8_t ls_records[3][0x40];
static uint8_t ls_boxes[3][0x20];
static char ls_names[3][16] = {"f1_2026", "", "f3_2026"};
static uint64_t ls_slots[5][2];
static uint8_t ls_container[0x580];
static uint8_t ls_item[0x1d0];
static uint8_t ls_label_str[0x20];
static uint8_t ls_ctx[0x240];
static uint64_t ls_out[2];

static void ls_setup(void)
{
    static const int rec_for_slot[3] = {0, -1, 2};
    for (int s = 0; s < 3; s++) {
        if (rec_for_slot[s] < 0)
            continue;
        int r = rec_for_slot[s];
        ls_slots[s][1] = (uint64_t)ls_records[r];
        *(uint64_t *)(ls_records[r] + 0x28) = (uint64_t)ls_boxes[r];
        *(uint64_t *)(ls_boxes[r] + 0x10) = (uint64_t)ls_names[r];
        *(uint64_t *)(ls_records[r] + 0x10) = (uint64_t)ls_entrants;
        *(uint64_t *)(ls_records[r] + 0x18) = (uint64_t)ls_entrants;
    }
    *(uint32_t *)(ls_entrants + 0x5c) = 303;
    ls_slots[3][1] = 0x00001234deadb000ull;
    *(uint64_t *)(ls_container + 0x560) = (uint64_t)&ls_slots[0];
    *(uint64_t *)(ls_container + 0x568) = (uint64_t)&ls_slots[3];
    *(uint64_t *)(ls_ctx + 0x228) = (uint64_t)ls_container;
    *(uint64_t *)(ls_ctx + 0x238) = 0;
    *(uint64_t *)(ls_container + 0x58) = 0;
    ls_item[0x1b8] = 1;
    *(uint64_t *)(ls_item + 0x1c8) = (uint64_t)ls_container;
    *(uint64_t *)(ls_item + 0x198) = (uint64_t)ls_label_str;
    ls_slots[1][1] = 0;
    memset(ls_slots[4], 0, sizeof ls_slots[4]);
}

static void ls_label(const char *name)
{
    strcpy((char *)ls_label_str + 9, name);
}

static int ls_first_id(const char *name, uint32_t *id)
{
    ls_setup();
    ls_label(name);
    ls_out[0] = ls_out[1] = 0;
    fake_list_first_id(ls_item, ls_out);
    *id = (uint32_t)ls_out[0];
    return ((uint8_t *)ls_out)[4];
}

static void test_fe_tiles(void)
{
    uint32_t id = 0;
    int found = ls_first_id("f3_2026", &id);
    expect(found == 1 && id == 303, "fe tiles: a list in the slot array is found and runs the game's code");
    found = ls_first_id("f2_2026", &id);
    expect(found == 0, "fe tiles: a list the import removed takes the game's no-list path");
    found = ls_first_id("f2_2026", &id);
    expect(found == 0, "fe tiles: the removed list is skipped again");
    ls_setup();
    expect(fake_list_usable(ls_ctx, "f1_2026") == 1, "fe tiles: a list with a database row is usable as the game says");
    expect(fake_list_usable(ls_ctx, "f2_2026") == 0, "fe tiles: a list with no database row is not usable");
}

static void crash_fe_scan(void)
{
    SetErrorMode(SEM_NOGPFAULTERRORBOX);
    uint32_t id = 0;
    printf("      fe tiles: looking up a list that is not in the slot array\n");
    fflush(stdout);
    int found = ls_first_id("f2_2026", &id);
    printf("      fe tiles: the scan returned %d\n", found);
    fflush(stdout);
}

static void crash_fe_row(void)
{
    SetErrorMode(SEM_NOGPFAULTERRORBOX);
    ls_setup();
    printf("      fe tiles: reading a list that has no database row\n");
    fflush(stdout);
    uint64_t r = fake_list_usable(ls_ctx, "f2_2026");
    printf("      fe tiles: the list returned %llu\n", (unsigned long long)r);
    fflush(stdout);
}

// Ai_Job_UpdateCarsParallel's corner line read, copied byte for byte from the Steam build (0x14064069e..0x1406406ac and
// 0x1406409be..0x1406409ce), in a small frame that takes a gap evaluation result in rcx, the car index in rdx and the car's
// AI state in r8. The load is [[result+8]+car*8+0xb8], with no check on result+8; it returns that entry for a line that is
// there, 0x9be for the game's own no-corner-line path (the cmp byte [state+0x185] and its je, state+0x185 clear), 0x222 when
// that byte is set. Core's AI corner guard resolves site_Ai_UpdateCars_CornerLineLoad at +0x10 and
// site_Ai_UpdateCars_NoCornerLine here.
__asm__(
    ".section .xtls,\"xr\"\n"
    ".globl fake_ai_corner\n"
    ".def fake_ai_corner; .scl 2; .type 32; .endef\n"
    ".seh_proc fake_ai_corner\n"
    "fake_ai_corner:\n"
    ".byte 0x56\n.seh_pushreg %rsi\n"
    ".byte 0x57\n.seh_pushreg %rdi\n"
    ".byte 0x53\n.seh_pushreg %rbx\n"
    ".seh_endprologue\n"
    ".byte 0x48,0x89,0xCE,0x48,0x89,0xD7,0x4C,0x89,0xC3,0x48,0x8B,0x46,0x08\n"
    ".byte 0x48,0x8B,0x84,0xF8,0xB8,0x00,0x00,0x00\n"
    ".byte 0x48,0x85,0xC0\n"
    ".byte 0x0F,0x84\n.long .Laicnone-.Laicje\n.Laicje:\n"
    ".byte 0xEB\n.byte .Laicdone-.Laicj1\n.Laicj1:\n"
    ".Laicnone:\n"
    ".byte 0x80,0xBB,0x85,0x01,0x00,0x00,0x00,0x74,0x4C\n"
    ".byte 0xC5,0xFA,0x11,0xBB,0x20,0x01,0x00,0x00\n"
    ".byte 0xB8,0x22,0x02,0x00,0x00\n"
    ".byte 0xEB\n.byte .Laicdone-.Laicj2\n.Laicj2:\n"
    ".fill 0x3D,1,0x90\n"
    ".byte 0xB8,0xBE,0x09,0x00,0x00\n"
    ".Laicdone:\n"
    ".byte 0x5B,0x5F,0x5E,0xC3\n"
    ".seh_endproc\n"
    ".section .drectve\n"
    ".ascii \" -export:fake_ai_corner\"\n"
    ".text\n");
uint64_t fake_ai_corner(void *result, uint64_t car, void *state);

static uint64_t ai_corner_result[0x50 / 8];
static uint8_t ai_corner_line[0x100];
static uint8_t ai_corner_state[0x200];

static void ai_corner_setup(void)
{
    memset(ai_corner_result, 0, sizeof ai_corner_result);
    memset(ai_corner_state, 0, sizeof ai_corner_state);
    *(uint64_t *)(ai_corner_line + 0xb8 + 3 * 8) = 0xc0de;
}

static void test_ai_corner(void)
{
    ai_corner_setup();
    ai_corner_result[1] = (uint64_t)ai_corner_line;
    uint64_t r = fake_ai_corner(ai_corner_result, 3, ai_corner_state);
    expect(r == 0xc0de, "ai corner: a car with a brake line reads its entry as the game does");
    ai_corner_result[1] = 0;
    r = fake_ai_corner(ai_corner_result, 3, ai_corner_state);
    uint64_t again = fake_ai_corner(ai_corner_result, 5, ai_corner_state);
    expect(r == 0x9be && again == 0x9be, "ai corner: a car with no brake line takes the game's no-corner-line path");
}

static void crash_ai_corner(void)
{
    SetErrorMode(SEM_NOGPFAULTERRORBOX);
    ai_corner_setup();
    printf("      ai corner: reading a car with no brake line\n");
    fflush(stdout);
    uint64_t r = fake_ai_corner(ai_corner_result, 3, ai_corner_state);
    printf("      ai corner: the car returned 0x%llx\n", (unsigned long long)r);
    fflush(stdout);
}

// Ne_Replay_WriteFrames' stream and holder state around site_Ne_ReplayAfterRecord (the call [rax+0x20], mov r12,rsi,
// mov rax,[rsi], mov rcx,rsi bytes of the Steam build at 0x1445d170f), in a small frame that takes the scratch buffer in rcx,
// the serialized state's size in rdx, the holder's capacity in r8 and the recordable's name in r9. It lays them out where the
// game keeps them (stream position [rsp+0x98], stream base [rsp+0xa0], holder capacity [rsp+0x108], name in r13) and returns
// the size the recorder would copy into the holder: position minus base after the site. Core's replay recorder grid guard
// cuts a state larger than the capacity to nothing there.
__asm__(
    ".section .xtls,\"xr\"\n"
    ".globl fake_replay_write\n"
    ".def fake_replay_write; .scl 2; .type 32; .endef\n"
    ".seh_proc fake_replay_write\n"
    "fake_replay_write:\n"
    "push %rsi\n.seh_pushreg %rsi\n"
    "push %r12\n.seh_pushreg %r12\n"
    "push %r13\n.seh_pushreg %r13\n"
    "sub $0x120,%rsp\n.seh_stackalloc 0x120\n"
    ".seh_endprologue\n"
    "mov %r9,%r13\n"
    "mov 0x160(%rsp),%rax\n"
    "mov %rax,0x198(%rsp)\n"
    "movq $0,0x190(%rsp)\n"
    "movq $0,0x118(%rsp)\n"
    "mov %rcx,0xa0(%rsp)\n"
    "lea (%rcx,%rdx),%rax\n"
    "mov %rax,0x98(%rsp)\n"
    "mov %r8,0x108(%rsp)\n"
    "lea .Lrwstub(%rip),%rax\n"
    "mov %rax,0x60(%rsp)\n"
    "lea 0x40(%rsp),%rax\n"
    "lea 0x10(%rsp),%rsi\n"
    ".byte 0xFF,0x50,0x20,0x49,0x89,0xF4,0x48,0x8B,0x06,0x48,0x89,0xF1\n"
    "mov 0x98(%rsp),%rax\n"
    "sub 0xa0(%rsp),%rax\n"
    "add $0x120,%rsp\n"
    "pop %r13\npop %r12\npop %rsi\n"
    "ret\n"
    ".Lrwstub:\n"
    "ret\n"
    ".seh_endproc\n"
    ".section .drectve\n"
    ".ascii \" -export:fake_replay_write\"\n"
    ".text\n");
uint64_t fake_replay_write(void *scratch, uint64_t size, uint64_t capacity, const char *name, void *entry);


// Ne_ReplayRecordable_ReadFrom's two apply sites (the bytes of the Steam build at 0x1445cdcc0 and 0x1445ceece): the full state
// read, which calls the recordable's vtable slot 0x38 with the stream at rsp+0x90, and the delta read, which calls slot 0x28
// (mode 1) or 0x40 (mode 2) and then the stream's cleanup slot 0xc0. The stored state size is the stream's field at
// rsp+0xa8. The frame takes the registry entry in rcx, the stored size in edx and the read mode in r8d, and applies the
// entry's recordable (entry+0x10) through the game's own bytes. Core's replay read guard skips the apply for a recordable the
// recorder wrote as nothing and a stored size of 0.
__asm__(
    ".section .xtls,\"xr\"\n"
    ".globl fake_replay_read\n"
    ".def fake_replay_read; .scl 2; .type 32; .endef\n"
    ".seh_proc fake_replay_read\n"
    "fake_replay_read:\n"
    "push %rsi\n.seh_pushreg %rsi\n"
    "push %rdi\n.seh_pushreg %rdi\n"
    "push %r12\n.seh_pushreg %r12\n"
    "push %r13\n.seh_pushreg %r13\n"
    "sub $0x120,%rsp\n.seh_stackalloc 0x120\n"
    ".seh_endprologue\n"
    "mov %edx,0xa8(%rsp)\n"
    "lea 0x30(%rsp),%r13\n"
    "movl %r8d,4(%r13)\n"
    "movl $0,0x10(%r13)\n"
    "lea fake_stream_obj(%rip),%rsi\n"
    "mov %rcx,%r12\n"
    "mov %rcx,%rdi\n"
    "test %r8d,%r8d\n"
    "jnz .Lrrdelta\n"
    "lea 0x90(%rsp),%rdi\n"
    ".byte 0x48,0x89,0xF9,0x48,0x89,0xF2\n"
    "call .Lrrnop\n"
    ".byte 0x49,0x8B,0x4C,0x24,0x10,0x48,0x8B,0x01,0x48,0x89,0xFA,0xFF,0x50,0x38,0xE9\n"
    ".long .Lrrend - . - 4\n"
    ".Lrrdelta:\n"
    ".byte 0x41,0x8B,0x45,0x04,0x83,0xF8,0x02,0x74,0x19,0x83,0xF8,0x01,0x75,0x30,0x48,0x8B,0x4F,0x10,0x48,0x8B,0x01,0x48,0x8D,0x94,0x24,0x90,0x00,0x00,0x00,0xFF,0x50,0x28,0xEB,0x1C,0xC4,0xC1,0x7A,0x10,0x45,0x10,0xC5,0xFA,0x5A,0xD0,0x48,0x8B,0x4F,0x10,0x48,0x8B,0x01,0x48,0x8D,0x94,0x24,0x90,0x00,0x00,0x00,0xFF,0x50,0x40,0x48,0x8B,0x06,0x48,0x89,0xF1,0xFF,0x90,0xC0,0x00,0x00,0x00\n"
    ".Lrrend:\n"
    "mov fake_applied(%rip),%rax\n"
    "add $0x120,%rsp\n"
    "pop %r13\npop %r12\npop %rdi\npop %rsi\n"
    "ret\n"
    ".Lrrnop:\n"
    "ret\n"
    ".seh_endproc\n"
    ".section .drectve\n"
    ".ascii \" -export:fake_replay_read\"\n"
    ".text\n");
uint64_t fake_replay_read(void *entry, uint32_t stored_size, uint32_t mode);

uint64_t fake_applied;
uintptr_t fake_stream_obj;
static uintptr_t fake_stream_vt[0x20], fake_rec_vt[0x10];
static uintptr_t fake_rec_obj[2][1];
static uint8_t fake_entries[2][0x688];

static void fake_apply(void *self)
{
    (void)self;
    fake_applied++;
}

static void fake_stream_done(void *self)
{
    (void)self;
}

static uint8_t replay_scratch[0x400];

static void replay_entries_init(void)
{
    fake_stream_vt[0xc0 / 8] = (uintptr_t)&fake_stream_done;
    fake_stream_obj = (uintptr_t)fake_stream_vt;
    for (int i = 0; i < 0x10; i++)
        fake_rec_vt[i] = (uintptr_t)&fake_apply;
    for (int i = 0; i < 2; i++) {
        fake_rec_obj[i][0] = (uintptr_t)fake_rec_vt;
        *(uintptr_t *)(fake_entries[i] + 0x10) = (uintptr_t)&fake_rec_obj[i];
    }
}

static uint64_t replay_read(int entry, uint32_t size, uint32_t mode)
{
    uint64_t before = fake_applied;
    fake_replay_read(fake_entries[entry], size, mode);
    return fake_applied - before;
}

static void test_replay_grid(void)
{
    replay_entries_init();
    expect(fake_replay_write(replay_scratch, 0x100, 0x200, "VehicleSystem", fake_entries[0]) == 0x100,
           "replay grid: a state within its holder's capacity is recorded whole");
    expect(fake_replay_write(replay_scratch, 0x200, 0x200, "VehicleSystem", fake_entries[0]) == 0x200,
           "replay grid: a state exactly the holder's capacity is recorded whole");
    expect(replay_read(0, 12, 1) == 1 && replay_read(0, 12, 2) == 1 && replay_read(0, 12, 0) == 1,
           "replay grid: a recorded state is applied by the delta reads and the full read");
    expect(replay_read(1, 0, 1) == 1 && replay_read(1, 0, 2) == 1 && replay_read(1, 0, 0) == 1,
           "replay grid: an empty stored state of a recordable the recorder never cut is applied as the game does");
    expect(fake_replay_write(replay_scratch, 0x300, 0x200, "CannedEventsSystem", fake_entries[0]) == 0,
           "replay grid: a state larger than its holder's capacity is recorded as nothing");
    expect(fake_replay_write(replay_scratch, 0x300, 0x300, "TyreSystem", fake_entries[1]) == 0x300,
           "replay grid: the next recordable with a fitting state is recorded whole");
    expect(replay_read(0, 0, 1) == 0 && replay_read(0, 0, 2) == 0,
           "replay grid: reading the empty state of a cut recordable skips its delta apply");
    expect(replay_read(0, 0, 0) == 0, "replay grid: reading the empty state of a cut recordable skips its full apply");
    expect(replay_read(0, 12, 1) == 1 && replay_read(0, 12, 0) == 1,
           "replay grid: a stored state with bytes is applied even for a cut recordable");
    expect(replay_read(1, 0, 1) == 1 && replay_read(1, 0, 0) == 1,
           "replay grid: another recordable's empty state is still applied");
}

static void run_replay_grid(void)
{
    SetErrorMode(SEM_NOGPFAULTERRORBOX);
    replay_entries_init();
    uint64_t r = fake_replay_write(replay_scratch, 0x3000, 0x1c00, "VehicleSystem", fake_entries[0]);
    printf("      replay grid: the recorder would copy 0x%llx bytes\n", (unsigned long long)r);
    fflush(stdout);
}

static void test_settings(int on, uint64_t chain)
{
    HMODULE t = GetModuleHandleW(L"t_settings.dll");
    void (*read)(void) = t ? (void (*)(void))(void *)GetProcAddress(t, "t_settings_read") : NULL;
    expect(read != NULL, "settings: t_settings.dll loaded and exports t_settings_read");
    if (!read)
        return;
    expect(reads_are(read, !on, on ? 1 : 7, on ? -250 : 500, on ? "tier01" : "plain"),
           on ? "settings: reads come from settings.json (bool, int, double, string_view); the type mismatch, the "
                "undeclared setting and another mod's option read their code defaults"
              : "settings: no settings.json, every setting reads its code default");
    wchar_t req[MAX_PATH] = {0}, snap[MAX_PATH + 16];
    GetEnvironmentVariableW(L"SWOFTY_TEST_REQUESTS", req, MAX_PATH);
    int n = sizeof setting_commands / sizeof *setting_commands, same = 1;
    for (int i = 0; i < n; i++) {
        same &= send_command(setting_commands[i]) == chain;
        if (*req) {
            swprintf(snap, MAX_PATH + 16, L"%ls.%d", req, i + 1);
            CopyFileW(req, snap, FALSE);
        }
        if (i == 3) {
            expect(reads_are(read, 1, on ? 9 : 7, on ? 750 : 500, on ? "replica" : "plain"),
                   on ? "settings: after four swofty_set commands the reads follow the new states"
                      : "settings: commands change nothing without settings.json");
            expect(on ? probe[8] == 1 && probe[9] == 9 && probe[10] == 750 && probe_str(11) &&
                            !strcmp(probe_str(11), "replica") && probe[16] == 1 && probe[17] == 1 && probe[18] == 1 &&
                            probe[19] == 1
                      : probe[16] + probe[17] + probe[18] + probe[19] == 0,
                   on ? "settings: SWOFTY_ON_CHANGE watchers got verbose=true, greeting_count=9, offset=0.75, "
                        "flavour='replica', once each"
                      : "settings: no watcher ran");
            if (on)
                expect(probe[11] == probe[27], "settings: the choice string is the same core-owned pointer in the watcher "
                                               "and in later reads");
        }
        if (i == 4)
            expect(probe[20] == 0 && reads_are(read, 1, on ? 9 : 7, on ? 750 : 500, on ? "replica" : "plain"),
                   "settings: a command for the mistyped option runs no watcher and its read keeps the code default");
        if (i == 6)
            expect(on ? probe[8] == 0 && probe[16] == 2 && reads_are(read, 0, 9, 750, "replica") : probe[16] == 0,
                   on ? "settings: verbose back to false, its watcher ran a second time" : "settings: still no watcher");
    }
    expect(same, "settings: FeHome_Handle_Command returns the chain's value for every command (core's wrap keeps rax)");
    for (DWORD t0 = GetTickCount(); GetTickCount() - t0 < 1000 && !closes;) {
        MSG m;
        while (PeekMessageW(&m, NULL, 0, 0, PM_REMOVE))
            DispatchMessageW(&m);
        Sleep(10);
    }
    expect(closes == on, on ? "settings: swofty_restart posted WM_CLOSE to the main window"
                            : "settings: swofty_restart without settings.json closes nothing");
}

extern ATOM(WINAPI *__imp_RegisterClassExW)(const WNDCLASSEXW *);
extern HWND(WINAPI *__imp_CreateWindowExW)(DWORD, LPCWSTR, LPCWSTR, DWORD, int, int, int, int, HWND, HMENU, HINSTANCE,
                                           LPVOID);

__declspec(dllexport) __attribute__((noinline)) void fake_crash(volatile int *p)
{
    *p = 1;
    printf("not reached\n");
}

static uintptr_t slot8(void) { return ((uintptr_t (*)(void))OTHER_VT[8])(); }

// A stand-in NeMemoryHeap for the crash report's heap decoding: Ne_ReportMemoryError with the game's
// prologue (the core hooks it by that pattern), Ne_FatalAbort's write to address 1, and the start
// of the heap error method that the heap's vtable carries at +0x118, by which the core recognises a
// heap object. The heap object and its blocks have the field offsets of Ne_Heap_DeallocateNonSmall.
int fake_report_memory_error(int code, const char *msg, char fatal, char brk, uint64_t a5, void *heap);
void fake_fatal_abort(void);
void fake_heap_error_method(void);
__asm__(
    ".section .xtls,\"xr\"\n"
    ".globl fake_report_memory_error\n.seh_proc fake_report_memory_error\nfake_report_memory_error:\n"
    ".byte 0x41,0x57\n.seh_pushreg %r15\n.byte 0x41,0x56\n.seh_pushreg %r14\n"
    ".byte 0x41,0x55\n.seh_pushreg %r13\n.byte 0x41,0x54\n.seh_pushreg %r12\n"
    ".byte 0x56\n.seh_pushreg %rsi\n.byte 0x57\n.seh_pushreg %rdi\n"
    ".byte 0x55\n.seh_pushreg %rbp\n.byte 0x53\n.seh_pushreg %rbx\n"
    ".byte 0x48,0x81,0xEC,0xB8,0x02,0x00,0x00\n.seh_stackalloc 0x2b8\n.seh_endprologue\n"
    ".byte 0xC5,0x78,0x29,0x8C,0x24,0xA0,0x02,0x00,0x00\n"
    ".byte 0xC5,0x79,0x29,0x84,0x24,0x90,0x02,0x00,0x00\n"
    ".byte 0xC5,0xF9,0x29,0xBC,0x24,0x80,0x02,0x00,0x00\n"
    "mov 0x320(%rsp), %rax\nmov %rax, 0x20(%rsp)\nmov 0x328(%rsp), %rax\nmov %rax, 0x28(%rsp)\n"
    "call report_memory_error_impl\nadd $0x2b8, %rsp\n"
    "pop %rbx\npop %rbp\npop %rdi\npop %rsi\npop %r12\npop %r13\npop %r14\npop %r15\nret\n.seh_endproc\n"
    ".globl fake_fatal_abort\n.seh_proc fake_fatal_abort\nfake_fatal_abort:\n"
    ".byte 0x48,0x83,0xEC,0x28\n.seh_stackalloc 0x28\n.seh_endprologue\n"
    ".byte 0xC7,0x04,0x25,0x01,0x00,0x00,0x00,0xAD,0xDE,0xAD,0xDE\nint3\n.seh_endproc\n"
    ".globl fake_heap_error_method\nfake_heap_error_method:\n"
    ".byte 0x55,0x41,0x57,0x41,0x56,0x41,0x55,0x41,0x54,0x56,0x57,0x53,0x48,0x83,0xEC,0x18\n"
    ".byte 0x48,0x8D,0x6C,0x24,0x10,0x89,0xD6,0x48,0x89,0xCF,0x48,0x8B,0x5D,0x78\nint3\n"
    ".text\n");

#define HEAP_GUARD 32
static uint64_t fake_heap_obj[0x120 / 8];
static void *fake_heap_vt[0x130 / 8];
static const char fake_heap_name[] = "FakeHeap";
static uint64_t heap_capacity(void *h) { (void)h; return 0x10000; }
static uint64_t heap_free_bytes(void *h) { (void)h; return 0xfe00; }
static uint64_t heap_used(void *h) { (void)h; return 0x200; }
static uint64_t heap_largest(void *h, int a) { (void)h, (void)a; return 0xfc00; }
static const char *heap_name(void *h) { (void)h; return fake_heap_name; }

int report_memory_error_impl(int code, const char *msg, char fatal, char brk, uint64_t a5, void *heap)
{
    printf("      fake Ne_ReportMemoryError(0x%x, \"%s\", fatal %d, heap %p)\n", code, msg, fatal, heap);
    (void)brk, (void)a5;
    if (fatal)
        fake_fatal_abort();
    return code;
}

static uint32_t heap_block_size(uint32_t user) { return ((user + 15) & ~15u) + 2 * HEAP_GUARD + 0x10; }

static uint8_t *heap_block(uint8_t *at, uint32_t user, uint8_t fill)
{
    *(int32_t *)at = 0;
    *(int32_t *)(at + 4) = 0;
    *(uint32_t *)(at + 8) = heap_block_size(user);
    *(uint32_t *)(at + 12) = user;
    memcpy(at + 0x10, "[BLOCKSTART] Bound checked mem.", HEAP_GUARD);
    memset(at + 0x10 + HEAP_GUARD, fill, user);
    memcpy(at + 0x10 + HEAP_GUARD + user, "[BLOCKEND] Bound checked mem.  ", HEAP_GUARD);
    return at + 0x10 + HEAP_GUARD;
}

__declspec(dllexport) __attribute__((noinline)) void fake_heap_free(uint8_t *user)
{
    uint8_t *header = user - HEAP_GUARD - 0x10;
    if (*(uint32_t *)(header + 8) != heap_block_size(*(uint32_t *)(header + 12))) {
        char msg[96];
        snprintf(msg, sizeof msg, "Failed to find address %p in the allocated list", (void *)user);
        fake_report_memory_error(0x16, msg, 1, 0, 0, fake_heap_obj);
    }
}

static void heap_error(void)
{
    uint8_t *base = VirtualAlloc(NULL, 0x10000, MEM_RESERVE | MEM_COMMIT, PAGE_READWRITE);
    fake_heap_vt[0x60 / 8] = (void *)heap_capacity;
    fake_heap_vt[0x68 / 8] = (void *)heap_free_bytes;
    fake_heap_vt[0x70 / 8] = (void *)heap_used;
    fake_heap_vt[0x80 / 8] = (void *)heap_largest;
    fake_heap_vt[0xa0 / 8] = (void *)heap_name;
    fake_heap_vt[0x118 / 8] = (void *)fake_heap_error_method;
    fake_heap_obj[0] = (uint64_t)(uintptr_t)fake_heap_vt;
    fake_heap_obj[0x58 / 8] = (uint64_t)(uintptr_t)fake_heap_name;
    fake_heap_obj[0x98 / 8] = (uint64_t)(uintptr_t)base;
    fake_heap_obj[0xa0 / 8] = 0x10000;
    fake_heap_obj[0x100 / 8] = (uint64_t)(uintptr_t)base;
    fake_heap_obj[0x108 / 8] = HEAP_GUARD;
    fake_heap_obj[0x110 / 8] = 0x200;
    uint8_t *a = heap_block(base, 40, 0xaa);
    uint8_t *b = heap_block(base + heap_block_size(40), 64, 0xbb);
    heap_block(base + heap_block_size(40) + heap_block_size(64), 16, 0xcc);
    fake_report_memory_error(0x50, "Start bounding check buffer corrupt", 0, 0, 0, fake_heap_obj);
    static const char record[0x80] = "serialized replay record that is longer than its holder";
    memcpy(a, record, sizeof record - 0x18);
    printf("      copied 0x%x bytes into a 40-byte block; freeing the block after it\n", (unsigned)(sizeof record - 0x18));
    fake_heap_free(b);
}


static int gate_has(uint32_t h)
{
    for (size_t i = 0, start = h % 373; i < 373; i++) {
        uint32_t cur = fake_log_gate.hashes[(start + (13 + h % 13) * i) % 373];
        if (cur == h)
            return 1;
        if (!cur)
            return 0;
    }
    return 0;
}

static DWORD WINAPI gamelog_thread(LPVOID arg)
{
    for (int i = 0; i < 40; i++)
        fake_log_site_c();
    (void)arg;
    return 0;
}

static void test_gamelog(int on)
{
    expect(on ? fake_log_gate.enabled == 1 && gate_has(0x7c7c88cb) && gate_has(0x22222222) && !gate_has(0x11111111)
              : fake_log_gate.enabled == 0 && !gate_has(0x7c7c88cb),
           on ? "gamelog: gate enabled, TEST_WARN and UI_FLOW opened by the default channels, TEST_VERBOSE closed"
              : "gamelog: SWOFTY_GAME_LOG=off leaves the gate closed");
    fake_log_site_b();
    fake_log_site_c();
    for (int i = 0; i < 150; i++)
        fake_log_site_a();
    HANDLE t[4];
    for (int i = 0; i < 4; i++)
        t[i] = CreateThread(NULL, 0, gamelog_thread, NULL, 0, NULL);
    WaitForMultipleObjects(4, t, TRUE, 10000);
    for (int i = 0; i < 4; i++)
        CloseHandle(t[i]);
    NeStr *s = NULL;
    int64_t n = fake_format_string(&s, "Screen %hs failed to open because of %d", "swofty_settings", 3);
    expect(n == (int64_t)strlen("Screen swofty_settings failed to open because of 3") &&
               !strcmp(chars(s), "Screen swofty_settings failed to open because of 3"),
           "gamelog: Ne_Format_String still builds its string with the entry hook installed");
    fake_format_string(&s, "Failed to find screen %hs. Solution: Make sure the the screen is loaded into the runtime",
                       "swofty_settings");
    fake_format_string(&s, "button_%d", 7);
    Sleep(1100);
    fake_format_string(&s, "No screen called %hs exists", "late_screen");
    expect(!strcmp(chars(s), "No screen called late_screen exists"), "gamelog: a later route error is formatted normally");
    nestr_release(&s);
    if (on) {
        char *reserved = VirtualAlloc(NULL, 0x10000, MEM_RESERVE, PAGE_NOACCESS);
        fake_log_thunk(fake_chan_warn, fake_fmt_bad, reserved + 0x685);
        VirtualFree(reserved, 0, MEM_RELEASE);
        expect(1, "gamelog: a %s argument into reserved memory is survived");
    }
    expect(gamelog_body_calls == 0, "gamelog: the overwritten logger never runs the game's own body");
}

static void test_deadlock_warn(void)
{
    wchar_t pattern[MAX_PATH];
    GetModuleFileNameW(NULL, pattern, MAX_PATH);
    wchar_t *slash = wcsrchr(pattern, L'\\');
    if (slash)
        wcscpy(slash + 1, L"swofty\\stall-*.txt");
    fake_log_thunk(fake_chan_warn, fake_fmt_deadlock);
    int found = 0;
    for (int i = 0; i < 150 && !found; i++) {
        Sleep(100);
        WIN32_FIND_DATAW fd;
        HANDLE h = FindFirstFileW(pattern, &fd);
        if (h != INVALID_HANDLE_VALUE) {
            found = i + 1;
            FindClose(h);
        }
    }
    Sleep(1500);
    expect(found > 0, "stall: the game's deadlock WARN asks for a load-stall report without waiting for a threshold");
}

static void stage_early(void)
{
    expect(fake_stage_site() == 7 && fake_stage_wrap() == 9 && fake_stage_plain() == 5 && probe[32] == 0,
           "stages: before mods load, the stage targets are vanilla");
    HANDLE mx = CreateMutexW(NULL, FALSE, L"Codemasters F1 25");
    expect(mx && GetModuleHandleW(L"t_stage_early.dll") && !GetModuleHandleW(L"t_stage_late.dll") &&
               !GetModuleHandleW(L"t_stage_refused.dll"),
           "stages: CreateMutexW('Codemasters F1 25') loaded the early plugin, the window-stage plugins wait");
    int site = fake_stage_site(), wrap = fake_stage_wrap();
    printf("      after the early stage: site target %d (site handler ran %lld time(s)), wrap target %d\n", site,
           (long long)probe[32], wrap);
    expect(site == 7 && probe[32] == 1 && wrap == 91,
           "stages: the early plugin's site and wrap are installed at the entries of both targets");
    if (mx)
        CloseHandle(mx);
}

static void register_early(void)
{
    HANDLE mx = CreateMutexW(NULL, FALSE, L"Codemasters F1 25");
    expect(mx && GetModuleHandleW(L"t_register.dll"), "register: CreateMutexW loaded the addon that exports SwoftyAddonRegister");
    Sleep(600);
    wchar_t path[MAX_PATH];
    GetModuleFileNameW(NULL, path, MAX_PATH);
    wchar_t *slash = wcsrchr(path, L'\\');
    if (slash)
        wcscpy(slash + 1, L"swofty\\loader.log");
    HANDLE log = CreateFileW(path, FILE_APPEND_DATA, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, NULL,
                             OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, NULL);
    const char line[] = "fake game: the game database loads now\r\n";
    DWORD put = 0;
    expect(log != INVALID_HANDLE_VALUE && WriteFile(log, line, sizeof line - 1, &put, NULL),
           "register: the database moment is marked in loader.log");
    if (log != INVALID_HANDLE_VALUE)
        CloseHandle(log);
    Sleep(300);
    if (mx)
        CloseHandle(mx);
}

static int stage_window(HWND w)
{
    printf("      window stage: fn<Fake_Stage_Site> = %p (function at %p) returned %lld; fn<Fake_Stage_Wrap> = %p "
           "(function at %p) returned %lld; refused plugin marker %lld\n",
           (void *)(intptr_t)probe[33], (void *)fake_stage_site, (long long)probe[34], (void *)(intptr_t)probe[35],
           (void *)fake_stage_wrap, (long long)probe[36], (long long)probe[37]);
    expect(GetModuleHandleW(L"t_stage_late.dll") && GetModuleHandleW(L"t_stage_refused.dll"),
           "stages: the window-stage plugins load with the window");
    expect(probe[33] == (intptr_t)fake_stage_site,
           "stages: fn<Fake_Stage_Site> bound at the window stage resolves to the function under the early site hook");
    expect(probe[34] == 7 && probe[32] == 2,
           "stages: the call through fn<Fake_Stage_Site> runs the early site handler and returns the game's 7");
    expect(probe[35] == (intptr_t)fake_stage_wrap,
           "stages: fn<Fake_Stage_Wrap> bound at the window stage resolves to the function under the early wrap");
    expect(probe[36] == 91, "stages: the call through fn<Fake_Stage_Wrap> returns the early wrap's 91");
    expect(probe[37] == 0, "stages: the plugin whose patterns fail without a hook to explain it is refused and runs nothing");
    expect(probe[40] == 5, "stages: fn<Fake_Stage_Plain>, whose KB address holds other bytes, is found by a scan and runs");
    expect(fake_stage_site() == 7 && probe[32] == 3 && fake_stage_wrap() == 91 && fake_stage_plain() == 5,
           "stages: the game's own calls still go through the early hooks, the unhooked target is vanilla");
    printf("%s (%d failure(s))\n", fails ? "FAILED" : "ALL PASSED", fails);
    DestroyWindow(w);
    return fails != 0;
}

static DWORD WINAPI launch_error_box(LPVOID unused)
{
    (void)unused;
    MessageBoxW(NULL, L"Game launch error. Error code: 0x00000004", L"Error", MB_OK);
    return 0;
}

/* The EA App build's launch errors come up as a dialog before any game window: the loader must log it and keep
   waiting instead of loading addons on it. */
static void show_launch_error(void)
{
    HANDLE t = CreateThread(NULL, 0, launch_error_box, NULL, 0, NULL);
    HWND box = NULL;
    for (DWORD t0 = GetTickCount(); !box && GetTickCount() - t0 < 5000; Sleep(20))
        box = FindWindowW(L"#32770", L"Error");
    Sleep(600);
    if (box)
        PostMessageW(box, WM_COMMAND, IDOK, 0);
    WaitForSingleObject(t, 5000);
    CloseHandle(t);
    expect(box != NULL, "fallback mode: a launch error dialog came up before the window");
}

/* Tuning and overlay tools put a visible window of their own into the game a moment after it starts, before the
   game's: the loader must not take it for the main window and load addons past the early stage. */
static void show_foreign_window(void)
{
    HMODULE u32 = GetModuleHandleW(L"user32.dll");
    ATOM(WINAPI * rc)(const WNDCLASSEXW *) = (void *)GetProcAddress(u32, "RegisterClassExW");
    HWND(WINAPI * cw)(DWORD, LPCWSTR, LPCWSTR, DWORD, int, int, int, int, HWND, HMENU, HINSTANCE, LPVOID) =
        (void *)GetProcAddress(u32, "CreateWindowExW");
    HMODULE tool = GetModuleHandleW(L"kernel32.dll");
    WNDCLASSEXW fc = {.cbSize = sizeof fc, .lpfnWndProc = DefWindowProcW, .hInstance = tool,
                      .lpszClassName = L"F125TuneStatus"};
    ATOM a = rc(&fc);
    HWND f = cw(0, L"F125TuneStatus", L"", WS_POPUP, 0, 0, 40, 20, NULL, NULL, tool, NULL);
    if (!f)
        printf("      foreign window: RegisterClassExW %u, CreateWindowExW error %lu\n", a, GetLastError());
    ShowWindow(f, SW_SHOWNOACTIVATE);
    for (DWORD t0 = GetTickCount(); GetTickCount() - t0 < 600;) {
        MSG m;
        while (PeekMessageW(&m, NULL, 0, 0, PM_REMOVE))
            DispatchMessageW(&m);
        Sleep(20);
    }
    expect(f != NULL && IsWindowVisible(f), "fallback mode: another module's window came up before the game's");
    DestroyWindow(f);
}

#ifdef PACKED_TEST
// Two full pages of filler so every page the core samples is covered by encrypted bytes.
__asm__(".section .xtls,\"xr\"\n.fill 0x2000, 1, 0xCC\n.text\n");
#endif

// The packed build: run_wine_test.sh encrypts .xtls in the file with this xorshift stream, like the EA App
// build whose code reads encrypted at load; the exe decrypts it in bulk before its window, as that build does.
#define PACK_SEED 0x5EEDC0DEu

static IMAGE_SECTION_HEADER *exe_section(const char *name)
{
    uint8_t *b = (uint8_t *)GetModuleHandleW(NULL);
    IMAGE_NT_HEADERS *nt = (IMAGE_NT_HEADERS *)(b + ((IMAGE_DOS_HEADER *)b)->e_lfanew);
    IMAGE_SECTION_HEADER *s = IMAGE_FIRST_SECTION(nt);
    for (int i = 0; i < nt->FileHeader.NumberOfSections; i++, s++)
        if (!strncmp((const char *)s->Name, name, 8))
            return s;
    return NULL;
}

static double page_entropy(const uint8_t *p)
{
    unsigned cnt[256] = {0};
    for (int i = 0; i < 0x1000; i++)
        cnt[p[i]]++;
    double h = 0;
    for (int i = 0; i < 256; i++)
        if (cnt[i]) {
            double pr = cnt[i] / 4096.0;
            h -= pr * log2(pr);
        }
    return h;
}

static void decrypt_code(uint8_t *p, size_t n)
{
    DWORD old;
    VirtualProtect(p, n, PAGE_EXECUTE_READWRITE, &old);
    uint32_t x = PACK_SEED;
    for (size_t i = 0; i < n; i++) {
        x ^= x << 13, x ^= x >> 17, x ^= x << 5;
        p[i] ^= (uint8_t)(x >> 8);
    }
    VirtualProtect(p, n, old, &old);
    FlushInstructionCache(GetCurrentProcess(), p, n);
}

/* packed: .xtls reads encrypted at the early stage and is decrypted before the window. packed_never: it never
   decrypts. packed_plain: the same flow on the exe left unencrypted, with a set that is not packed. */
static void expect_in(const char *mode, int ok, const char *what)
{
    printf("%s: %s: %s\n", ok ? "ok  " : "FAIL", mode, what);
    fails += !ok;
}

static int packed_main(const char *mode)
{
    int encrypted = strcmp(mode, "packed_plain") != 0, decrypts = strcmp(mode, "packed_never") != 0;
    IMAGE_SECTION_HEADER *s = exe_section(".xtls");
    uint8_t *code = (uint8_t *)GetModuleHandleW(NULL) + s->VirtualAddress;
    size_t n = s->Misc.VirtualSize;
    uint8_t *before = malloc(n);
    memcpy(before, code, n);
    double e = page_entropy(code);
    printf("      %s: .xtls is %zu bytes, its first page reads entropy %.2f\n", mode, n, e);
    expect_in(mode, encrypted ? e > 7.5 : e < 6.5, encrypted ? "the game code section reads encrypted at load"
                                                    : "the game code section reads as code at load");
    HANDLE mx = CreateMutexW(NULL, FALSE, L"Codemasters F1 25");
    expect_in(mode, mx && GetModuleHandleW(L"swofty_core.dll") && GetModuleHandleW(L"t_packed.dll") &&
               GetModuleHandleW(L"t_packed_scan.dll"),
           "CreateMutexW loaded the core and the early plugins");
    if (encrypted)
        expect_in(mode, !memcmp(before, code, n), "nothing was written into the encrypted code at the early stage");
    if (decrypts && encrypted) {
        decrypt_code(code, n);
        printf("      packed: decrypted .xtls in bulk, first page entropy now %.2f\n", page_entropy(code));
        expect_in(mode, fake_stage_wrap() == 9 && probe[41] == 0, "before the window stage the decrypted code runs unhooked");
    }
    WNDCLASSEXW wc = {.cbSize = sizeof wc, .lpfnWndProc = wndproc, .hInstance = GetModuleHandleW(NULL),
                      .lpszClassName = L"EGO_WindowClass"};
    __imp_RegisterClassExW(&wc);
    HWND w = __imp_CreateWindowExW(0, L"EGO_WindowClass", L"F1 25", WS_OVERLAPPEDWINDOW, 0, 0, 320, 200, NULL, NULL,
                                   wc.hInstance, NULL);
    if (!decrypts) {
        Sleep(10500);
        expect_in(mode, !memcmp(before, code, n), "no hook was ever written into the code that never decrypted");
        expect_in(mode, probe[41] == 0 && probe[44] == 0 && probe[43] == 0,
               "no handler of the early plugins ran and no scan found anything");
    } else {
        run_build(1, 1);
        expect_in(mode, ncalls == 3 && !strcmp(calls[1].name, "career") && calls[1].index == 3 && !movie_built(),
               "the core's site on the home builder fires after the window stage (mod tile added, movie tile skipped)");
        expect_in(mode, fake_stage_wrap() == 89, "t_packed's wrap on Fake_Stage_Wrap runs");
        expect_in(mode, fake_stage_site() == 7 && probe[41] == 1, "t_packed's site on the entry of Fake_Stage_Site runs");
        expect_in(mode, probe[44] == 1, "t_packed's on_load handler ran once");
        expect_in(mode, probe[43] == (intptr_t)fake_stage_plain && probe[42] == (encrypted ? 2 : 1),
               encrypted ? "t_packed_scan's init was refused its scan of encrypted code, then found "
                           "Fake_Stage_Plain when called again"
                         : "t_packed_scan's init found Fake_Stage_Plain on its first call");
    }
    printf("%s (%d failure(s))\n", fails ? "FAILED" : "ALL PASSED", fails);
    DestroyWindow(w);
    return fails != 0;
}

/* nearfull / nopool: take every free 64 KB slot within 2 GB of the image before the core loads, the way the
   game's own allocations have by the window stage on a build whose core loads there. nopool first gives
   back the hook pool the loader reserved at attach. */
static int exhaust_near(int release_pool)
{
    wchar_t path[MAX_PATH];
    GetModuleFileNameW(NULL, path, MAX_PATH);
    wcscpy(wcsrchr(path, L'\\') + 1, L"version.dll");
    HMODULE loader = GetModuleHandleW(path);
    int (*take)(uintptr_t *, size_t *) = loader ? (void *)GetProcAddress(loader, "SwoftyLoaderHookPool") : NULL;
    uintptr_t pool = 0;
    size_t pool_size = 0;
    if (release_pool && take && take(&pool, &pool_size))
        VirtualFree((void *)pool, 0, MEM_RELEASE);
    uintptr_t base = (uintptr_t)GetModuleHandleW(NULL);
    const IMAGE_NT_HEADERS *nt = (const IMAGE_NT_HEADERS *)(base + ((const IMAGE_DOS_HEADER *)base)->e_lfanew);
    uintptr_t lo = base - 0x80010000ULL, hi = base + nt->OptionalHeader.SizeOfImage + 0x80010000ULL;
    MEMORY_BASIC_INFORMATION m;
    int taken = 0;
    for (uintptr_t p = lo; p < hi && VirtualQuery((void *)p, &m, sizeof m); p = (uintptr_t)m.BaseAddress + m.RegionSize) {
        uintptr_t a = (p + 0xFFFF) & ~(uintptr_t)0xFFFF, e = (uintptr_t)m.BaseAddress + m.RegionSize;
        e = (e < hi ? e : hi + 0xFFFF) & ~(uintptr_t)0xFFFF;
        if (m.State == MEM_FREE && a < e && VirtualAlloc((void *)a, e - a, MEM_RESERVE, PAGE_NOACCESS))
            taken++;
    }
    printf("      reserved %d free region(s) within 2 GB of the image%s\n", taken,
           release_pool ? (pool ? ", after releasing the loader's hook pool" : ", the loader had no hook pool") : "");
    return taken;
}


static void udp_participants(uint8_t *b, size_t size, size_t stride, int format, int driver, int team)
{
    memset(b, 0, size);
    b[0] = (uint8_t)format;
    b[1] = (uint8_t)(format >> 8);
    b[6] = 4;
    b[29] = 2;
    b[30 + 1] = (uint8_t)driver;
    b[30 + 3] = (uint8_t)team;
    b[30 + stride + 1] = 5;
    b[30 + stride + 3] = 2;
}

static int udp_get(SOCKET rx, uint8_t *out, int cap)
{
    DWORD ms = 1500;
    setsockopt(rx, SOL_SOCKET, SO_RCVTIMEO, (const char *)&ms, sizeof ms);
    return recvfrom(rx, (char *)out, cap, 0, NULL, NULL);
}

static void test_udp(int off)
{
    WSADATA wsa;
    WSAStartup(MAKEWORD(2, 2), &wsa);
    SOCKET tx = socket(AF_INET, SOCK_DGRAM, 0), rx = socket(AF_INET, SOCK_DGRAM, 0), lan = socket(AF_INET, SOCK_DGRAM, 0);
    struct sockaddr_in a = {.sin_family = AF_INET, .sin_addr.s_addr = htonl(INADDR_LOOPBACK)}, to, lan_to;
    int n = sizeof a;
    bind(rx, (struct sockaddr *)&a, sizeof a);
    getsockname(rx, (struct sockaddr *)&a, &n);
    to = a;
    struct sockaddr_in l = {.sin_family = AF_INET, .sin_addr.s_addr = htonl(INADDR_LOOPBACK), .sin_port = htons(21414)};
    int lan_bound = bind(lan, (struct sockaddr *)&l, sizeof l) == 0;
    lan_to = l;
    uint8_t sent[1500], got[1500];
    int want_driver = off ? 197 : 255, want_team = off ? 251 : 4;

    udp_participants(sent, 1284, 57, 2025, 197, 251);
    uint8_t keep[1500];
    memcpy(keep, sent, 1284);
    int r = sendto(tx, (char *)sent, 1284, 0, (struct sockaddr *)&to, sizeof to);
    expect(r == 1284, "udp: sendto returns the packet length");
    expect(!memcmp(sent, keep, 1284), "udp: the game's own buffer is not changed");
    r = udp_get(rx, got, sizeof got);
    expect(r == 1284 && got[31] == want_driver && got[33] == want_team,
           off ? "udp: SWOFTY_UDP_STOCK_IDS=0 sends the ids as the game wrote them"
               : "udp: a 2025 Participants packet arrives with driver 197 as 255 and team 251 as 4");
    expect(r == 1284 && got[30 + 57 + 1] == 5 && got[30 + 57 + 3] == 2, "udp: stock ids in the next entry arrive as sent");
    if (r == 1284)
        got[31] = 197, got[33] = 251;
    expect(r == 1284 && !memcmp(got, keep, 1284), "udp: no other byte of the packet changes");

    udp_participants(sent, 1283, 57, 2025, 197, 251);
    sendto(tx, (char *)sent, 1283, 0, (struct sockaddr *)&to, sizeof to);
    r = udp_get(rx, got, sizeof got);
    expect(r == 1283 && got[31] == 197 && got[33] == 251, "udp: a buffer one byte short of the packet size is untouched");

    udp_participants(sent, 1284, 57, 2023, 197, 251);
    sendto(tx, (char *)sent, 1284, 0, (struct sockaddr *)&to, sizeof to);
    r = udp_get(rx, got, sizeof got);
    expect(r == 1284 && got[31] == 197 && got[33] == 251, "udp: a buffer with another packet format is untouched");

    memset(sent, 0x41, 64);
    sendto(tx, (char *)sent, 64, 0, (struct sockaddr *)&to, sizeof to);
    r = udp_get(rx, got, sizeof got);
    expect(r == 64 && !memcmp(sent, got, 64), "udp: other traffic arrives byte for byte");

    if (lan_bound) {
        udp_participants(sent, 1284, 57, 2025, 197, 251);
        sendto(tx, (char *)sent, 1284, 0, (struct sockaddr *)&lan_to, sizeof lan_to);
        r = udp_get(lan, got, sizeof got);
        expect(r == 1284 && got[31] == 197 && got[33] == 251, "udp: a packet to the LAN game port is untouched");
    } else {
        printf("      port 21414 was taken; the LAN-port case is skipped\n");
    }
    closesocket(tx);
    closesocket(rx);
    closesocket(lan);
}

// The fake game's hand-written stand-ins leave callee-saved registers as the game's own code does, so the mode is read
// from memory after them, not from the register main would keep it in.
static int main_argc;
static char **main_argv;

int main(int argc, char **argv)
{
    main_argc = argc;
    main_argv = argv;
    int fallback = main_argc > 1 && !strcmp(main_argv[1], "fallback"), vm = main_argc > 1 && !strcmp(main_argv[1], "vm");
    int session = main_argc > 1 && !strcmp(main_argv[1], "session"), settings = main_argc > 1 && !strncmp(main_argv[1], "settings", 8);
    int stages = main_argc > 1 && !strcmp(main_argv[1], "stages");
    int registers = main_argc > 1 && !strcmp(main_argv[1], "register");
    int nopool = main_argc > 1 && !strcmp(main_argv[1], "nopool");
    setvbuf(stdout, NULL, _IONBF, 0);
    fake_log_gate.hashes[0x22222222 % 373] = 0xbeef;
    for (int i = 0; i < 40; i++)
        career_vt_storage[i] = other_vt_storage[i] = (void *)stub_zero;
    CAREER_VT[4] = (void *)career_populate;
    CAREER_VT[5] = (void *)id_action;
    OTHER_VT[4] = (void *)fake_base_populate;
    OTHER_VT[5] = (void *)id_action;
    if (main_argc > 1 && !strncmp(main_argv[1], "packed", 6))
        return packed_main(main_argv[1]);

    DWORD dummy, size = GetFileVersionInfoSizeW(L"C:\\windows\\system32\\kernel32.dll", &dummy);
    void *buf = malloc(size ? size : 1);
    VS_FIXEDFILEINFO *fi = NULL;
    UINT len = 0;
    int vok = size && GetFileVersionInfoW(L"C:\\windows\\system32\\kernel32.dll", 0, size, buf) &&
              VerQueryValueW(buf, L"\\", (void **)&fi, &len) && fi;
    expect(vok, "version.dll forwarding (GetFileVersionInfoSizeW/GetFileVersionInfoW/VerQueryValueW on kernel32)");
    if (vok)
        printf("      kernel32 version %u.%u.%u.%u\n", HIWORD(fi->dwFileVersionMS), LOWORD(fi->dwFileVersionMS),
               HIWORD(fi->dwFileVersionLS), LOWORD(fi->dwFileVersionLS));

    run_build(1, 1);
    expect(ncalls == 3 && calls[1].index == 3 && movie_built(), "before mods load: vanilla order career=2 movie=3 world=4");
    expect(fake_handle_command(0, 0) == 1 && slot8() == 0 && fake_caller() == 1 && fake_patchable() == 1,
           "before mods load: wrap, vtable, redirect and patch targets are vanilla");

    HMODULE u32 = GetModuleHandleW(L"user32.dll");
    void *real_rc = (void *)GetProcAddress(u32, "RegisterClassExW"), *real_cw = (void *)GetProcAddress(u32, "CreateWindowExW");
    expect(fallback || ((void *)__imp_RegisterClassExW != real_rc && (void *)__imp_CreateWindowExW != real_cw),
           "IAT trigger armed on USER32!RegisterClassExW and CreateWindowExW before the window exists");
    if (stages)
        stage_early();
    if (registers)
        register_early();
    if (nopool || (main_argc > 1 && !strcmp(main_argv[1], "nearfull")))
        expect(exhaust_near(nopool) > 0, "the address space within 2 GB of the image is taken before the core loads");
    ATOM(WINAPI * rc)(const WNDCLASSEXW *) = fallback ? (void *)real_rc : (void *)__imp_RegisterClassExW;
    HWND(WINAPI * cw)(DWORD, LPCWSTR, LPCWSTR, DWORD, int, int, int, int, HWND, HMENU, HINSTANCE, LPVOID) =
        fallback ? (void *)real_cw : (void *)__imp_CreateWindowExW;
    WNDCLASSEXW wc = {.cbSize = sizeof wc, .lpfnWndProc = wndproc, .hInstance = GetModuleHandleW(NULL),
                      .lpszClassName = L"EGO_WindowClass"};
    if (vm) {
        WNDCLASSEXW vc = wc;
        vc.lpszClassName = L"VmThunkClass";
        vm_thunk_register(&vc);
        expect((void *)__imp_RegisterClassExW != real_rc, "vm mode: call from a writable+exec section does not fire the trigger");
        run_build(1, 1);
        expect(ncalls == 3 && movie_built(), "vm mode: build after the skipped call is still vanilla");
    }
    rc(&wc);
    HWND w = cw(0, L"EGO_WindowClass", L"F1 25", WS_OVERLAPPEDWINDOW, 0, 0, 320, 200, NULL, NULL, wc.hInstance, NULL);
    expect((void *)__imp_RegisterClassExW == real_rc && (void *)__imp_CreateWindowExW == real_cw || fallback,
           "IAT slots restored after the trigger");
    if (stages)
        return stage_window(w);
    if (registers) {
        Sleep(800);
        printf("%s (%d failure(s))\n", fails ? "FAILED" : "ALL PASSED", fails);
        DestroyWindow(w);
        return fails != 0;
    }
    if (nopool) {
        printf("%s (%d failure(s))\n", fails ? "FAILED" : "ALL PASSED", fails);
        DestroyWindow(w);
        return fails != 0;
    }

    run_build(1, 1);
    if (fallback)
        expect(ncalls == 3 && movie_built(), "fallback mode: first build before the window is visible is vanilla");
    if (fallback) {
        show_launch_error();
        run_build(1, 1);
        expect(ncalls == 3 && movie_built(), "fallback mode: a dialog before the window does not load addons");
        show_foreign_window();
        run_build(1, 1);
        expect(ncalls == 3 && movie_built(), "fallback mode: another module's window before the game's does not load addons");
    }
    else
        expect(ncalls == 3 && calls[1].index == 3 && !movie_built(),
               "first home build right after window creation already has the mod tile and no movie tile (career=2, "
               "mod=3, world=4)");

    ShowWindow(w, SW_SHOW);
    // The fallback watcher loads the addons on its own thread once the window is visible, which takes longer than
    // the pump's 1.5 s on a loaded machine. The v1 plugin's init is the last step of that load, so the pump goes on
    // until it has run.
    DWORD loaded_at = 0;
    for (DWORD t0 = GetTickCount();;) {
        MSG m;
        while (PeekMessageW(&m, NULL, 0, 0, PM_REMOVE))
            DispatchMessageW(&m);
        DWORD now = GetTickCount();
        if (fallback && !loaded_at && probe[2])
            loaded_at = now;
        if (now - t0 >= 1500 && (!fallback || now - t0 >= 60000 || (loaded_at && now - loaded_at >= 200)))
            break;
        Sleep(20);
    }

    run_build(1, 1);
    expect(ncalls == 3 && !movie_built(), "hooked build with the movie flag set: movie tile skipped, 3 adder calls");
    expect(ncalls == 3 && !strcmp(calls[1].name, "career") && !strcmp(calls[2].name, "f1_world") && calls[0].index == 2 &&
               calls[1].index == 3 && calls[2].index == 4,
           "hooked: career=2, mod tile=3, world=4");
    expect(out_r12 == 3 && out_r15 == 4, "hooked: r12/r15 renumbered (3/4)");
    expect(build_leak == 0, "hooked build, populate and getAction leave no game string reference behind "
                            "(replaced Career id released, title/description/action strings released)");
    expect(probe[1] == 3 && probe[6] >= 1, "site chain: t_chain_a (priority -10) runs after core and sees the r12 it advanced");
    uint64_t chain = fake_handle_command(0, 0);
    printf("      FeHome_Handle_Command chain result %llu\n", (unsigned long long)chain);
    expect(chain == (session ? 132 : 1432),
           session ? "session: undeclared t_chain_b refused, chain t_chain_c(10) > swofty_core(0, passes through) > "
                     "t_chain_a(0) > game"
                   : "wrap chain on FeHome_Handle_Command: t_chain_c(10) > swofty_core(0, passes through) > t_chain_a(0) > "
                     "t_chain_b(0, loaded later) > game");
    expect(slot8() == 15, "vtable chain on one slot: t_chain_c(10) * 3 around t_chain_a(0) + 5");
    expect(fake_caller() == 101, "redirect: the call reaches the mod, orig is the old callee");
    expect(fake_patchable() == 2, "patch: expect bytes checked, new bytes written");
    expect(probe[0] == 1, "SWOFTY_ON_LOAD ran after the commit; fields are per object and released");
    expect(probe[2] == 1 && probe[3] == 1, "v1 plugin gets API v1 after the mixin commit (wrap already installed)");
    expect(probe[4] == 0 && probe[5] == 0, "dropped mods (unresolved wrap target, unresolved call) run nothing");
    Vec *v = (Vec *)(list + 0x78);
    uint8_t *mod = *(uint8_t **)(((uint8_t **)v->begin)[3] + 0xd0);
    uint8_t *career = *(uint8_t **)(((uint8_t **)v->begin)[1] + 0xd0);
    VmNode *mn = *(VmNode **)(mod + 0x20), *cn = *(VmNode **)(career + 0x20);
    Action a = {0};
    ((Action * (*)(uint8_t *, Action *))(*(void ***)mod)[5])(mod, &a);
    expect(!strcmp(chars(*(NeStr **)(mod + 0x10)), "mod_menu"), "mod item id = mod_menu");
    expect(mn && !strcmp(mn->title, "ADDON SETTINGS") && mn->title_localised == 0, "mod item title 'ADDON SETTINGS', not localised");
    expect(a.type == 1 && !strcmp(chars(a.str), "mod_menu"), "mod item action {1, mod_menu}");
    expect(cn && !strcmp(cn->title, "lng_mode_career") && cn->title_localised == 1, "career item untouched");
    expect((*(void ***)mod)[0] == CAREER_VT[0] && (*(void ***)mod)[-1] == CAREER_VT[-1], "mod vtable keeps career slots 0 and -1");

    expect((void *)__imp_RegisterClassExW == real_rc && (void *)__imp_CreateWindowExW == real_cw,
           "IAT slots pristine after mods loaded");

    run_build(0, 0);
    expect(ncalls == 3 && calls[0].index == 1 && calls[1].index == 2 && calls[2].index == 3, "hooked without movie: career=1, mod=2, world=3");
    expect(out_r12 == 2 && out_r15 == 3, "hooked without movie: r12/r15 = 2/3");
    expect(build_leak == 0, "second hooked build leaves no game string reference behind");

    test_settings(settings, chain);
    test_items(settings);
    if (main_argc > 1 && (!strcmp(main_argv[1], "udp") || !strcmp(main_argv[1], "udp_off")))
        test_udp(!strcmp(main_argv[1], "udp_off"));
    if (main_argc > 1 && !strcmp(main_argv[1], "trigger")) {
        test_ers_newlap();
        test_ers_mode();
        test_standings();
        test_weekends();
        test_teamhq();
        test_activity();
        test_teammate_pairs();
        test_setup_vm();
        test_fe_tiles();
        test_ai_corner();
        test_replay_grid();
        test_garage_crew(1);
        test_pit_crew(1);
        test_garage_scene(1);
        test_pit_slots(1);
        test_leaderboard_guard();
        test_track_acclim();
    }
    HMODULE ta = GetModuleHandleW(L"t_assets.dll");
    void (*assets_late)(void) = ta ? (void (*)(void))(void *)GetProcAddress(ta, "t_assets_late") : NULL;
    if (assets_late)
        assets_late();

    if (main_argc > 1 && !strcmp(main_argv[1], "trigger")) {
        fake_flow();
        Sleep(40);
        fake_flow();
        fake_flow();
        printf("      drove the flow tick three times: two frames 40 ms apart, the second with a nested tick\n");
        expect((probe[39] & 3) == 3, "events: at ON_LOAD, before the events setup, t_events_early saw available() true and "
                                     "on_frame ok");
        expect(probe[38] == 2, "events: the frame callback t_events_early queued at ON_LOAD fired once per frame after "
                               "the setup armed it, not on the nested tick");
    }

    if (main_argc > 1 && (!strcmp(main_argv[1], "trigger") || !strcmp(main_argv[1], "vm") || !strcmp(main_argv[1], "gamelog_off")))
        test_gamelog(strcmp(main_argv[1], "gamelog_off") != 0);
    if (main_argc > 1 && !strcmp(main_argv[1], "trigger"))
        test_deadlock_warn();
    printf("%s (%d failure(s))\n", fails ? "FAILED" : "ALL PASSED", fails);
    if (main_argc > 1 && !strcmp(main_argv[1], "crash")) {
        SetErrorMode(SEM_NOGPFAULTERRORBOX);
        fake_crash((volatile int *)0x10);
    }
    if (main_argc > 1 && !strcmp(main_argv[1], "exit"))
        ExitProcess(255);
    if (main_argc > 1 && !strcmp(main_argv[1], "acclim"))
        crash_track_acclim(0);
    if (main_argc > 1 && !strcmp(main_argv[1], "acclim_penalty"))
        crash_track_acclim(1);
    if (main_argc > 1 && !strcmp(main_argv[1], "ers_mode"))
        crash_ers_mode();
    if (main_argc > 1 && !strcmp(main_argv[1], "standings"))
        crash_standings();
    if (main_argc > 1 && !strcmp(main_argv[1], "weekends"))
        crash_weekends();
    if (main_argc > 1 && !strcmp(main_argv[1], "teamhq"))
        crash_teamhq();
    if (main_argc > 1 && !strcmp(main_argv[1], "activity"))
        crash_activity();
    if (main_argc > 1 && !strcmp(main_argv[1], "pairs"))
        crash_pairs();
    if (main_argc > 1 && !strcmp(main_argv[1], "setup_vm"))
        crash_setup_vm();
    if (main_argc > 1 && !strcmp(main_argv[1], "fe_scan"))
        crash_fe_scan();
    if (main_argc > 1 && !strcmp(main_argv[1], "fe_row"))
        crash_fe_row();
    if (main_argc > 1 && !strcmp(main_argv[1], "ai_corner"))
        crash_ai_corner();
    if (main_argc > 1 && !strcmp(main_argv[1], "replay_grid"))
        run_replay_grid();
    if (main_argc > 1 && !strcmp(main_argv[1], "garage_crew")) {
        const char *off = getenv("SWOFTY_GARAGE_CREW_GUARD");
        test_garage_crew(!(off && *off));
        test_pit_crew(!(off && *off));
        test_garage_scene(!(off && *off));
    }
    if (main_argc > 1 && !strcmp(main_argv[1], "pit_slots")) {
        const char *off = getenv("SWOFTY_PIT_SLOT_GUARD");
        test_pit_slots(!(off && *off));
    }
    if (main_argc > 1 && !strcmp(main_argv[1], "myteam_keep")) {
        const char *off = getenv("SWOFTY_MYTEAM_VEHICLE_KEEP");
        test_myteam_vehicle_keep(!(off && *off));
    }
    if (main_argc > 1 && !strcmp(main_argv[1], "myteam_vehicle")) {
        const char *off = getenv("SWOFTY_MYTEAM_VEHICLE_GUARD");
        test_myteam_vehicle_guard(!(off && *off));
    }
    if (main_argc > 1 && !strcmp(main_argv[1], "myteam_heal")) {
        const char *off = getenv("SWOFTY_MYTEAM_VEHICLE_HEAL");
        test_myteam_vehicle_heal(!(off && *off));
    }
    if (main_argc > 1 && !strcmp(main_argv[1], "leaderboard"))
        crash_leaderboard_guard();
    if (main_argc > 1 && !strcmp(main_argv[1], "track_list")) {
        const char *off = getenv("SWOFTY_TRACK_LIST_GUARD");
        test_track_list_guard(!(off && *off));
    }
    if (main_argc > 1 && !strcmp(main_argv[1], "heap")) {
        SetErrorMode(SEM_NOGPFAULTERRORBOX);
        for (int i = 0; i < 3; i++)
            fake_log_site_a();
        heap_error();
    }
    if (main_argc > 1 && !strcmp(main_argv[1], "abort")) {
        SetErrorMode(SEM_NOGPFAULTERRORBOX);
        fake_fatal_abort();
    }
    DestroyWindow(w);
    return fails != 0;
}
