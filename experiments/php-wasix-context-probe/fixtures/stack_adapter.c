/* Closed two-context guest adapter. Not PHP/Fiber support or a general ABI.
 * Never include stack_adapter.h here: SDK calls below must remain unwrapped. */
#include <errno.h>
#include <stdint.h>
#include <stdlib.h>
#include <wasix/context.h>

#define STACK_BYTES 65536u
#define SLOT_COUNT 2u

_Static_assert(sizeof(void *) == 4, "adapter pins wasm32 linear-memory pointers");
_Static_assert(sizeof(wasix_context_id_t) == 8, "adapter pins u64 context IDs");

struct slot {
    void *allocation;
    void (*entry)(void);
    wasix_context_id_t id;
};
static struct slot slots[SLOT_COUNT];

/* Ordinary linear-memory objects, read directly by stackless assembly thunks.
 * They are NOT C declarations of the __stack_pointer Wasm mutable global. */
uintptr_t rp_stack_top_a;
uintptr_t rp_stack_top_b;

extern void rp_stack_entry_a(void);
extern void rp_stack_entry_b(void);
extern unsigned rp_stack_switch_raw(wasix_context_id_t target);

_Noreturn void rp_stack_dispatch_a(void) {
    if (!slots[0].entry) _Exit(125);
    slots[0].entry();
    _Exit(125); /* A context entrypoint returning normally is never success. */
}

_Noreturn void rp_stack_dispatch_b(void) {
    if (!slots[1].entry) _Exit(125);
    slots[1].entry();
    _Exit(125);
}

int rp_context_create(wasix_context_id_t *id, void (*entry)(void)) {
    if (!id || !entry) {
        errno = EINVAL;
        return -1;
    }
    unsigned index = 0;
    while (index < SLOT_COUNT && slots[index].allocation) ++index;
    if (index == SLOT_COUNT) {
        errno = ENOMEM;
        return -1;
    }
    void *allocation = malloc(STACK_BYTES + 15u);
    if (!allocation) {
        errno = ENOMEM;
        return -1;
    }
    uintptr_t base = ((uintptr_t)allocation + 15u) & ~(uintptr_t)15u;
    uintptr_t top = base + STACK_BYTES;
    if (top < base || (top & 15u)) _Exit(126);
    slots[index].allocation = allocation;
    slots[index].entry = entry;
    if (index == 0) rp_stack_top_a = top;
    else rp_stack_top_b = top;
    int result = wasix_context_create(&slots[index].id,
        index == 0 ? rp_stack_entry_a : rp_stack_entry_b);
    if (result != 0) return result;
    *id = slots[index].id;
    return 0;
}

int rp_context_switch(wasix_context_id_t target) {
    /* This C frame stays on its own stack. The assembly restores that stack
     * before any C epilogue, spill or errno conversion may execute again. */
    unsigned error = rp_stack_switch_raw(target);
    if (error) {
        errno = (int)error;
        return -1;
    }
    return 0;
}

int rp_context_destroy(wasix_context_id_t target) {
    /* Wasmer cancellation is deferred. Do not free/recycle these two buffers,
     * even after destroy succeeds or create fails. Owned process exit reclaims
     * them; this diagnostic proves neither stack guards nor immediate release. */
    return wasix_context_destroy(target);
}
