/* Closed C/WASIX context experiment. No PHP, argv, environment or user code. */
#include <errno.h>
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <wasix/context.h>

#define ROUNDS 8
#define DEPTH 4
#define B_EXTRA_ROUNDS 2
#define WORDS 32
#define RANGE_COUNT (1 + 2 * DEPTH)
#define TRACE_CAPACITY 32

_Static_assert(sizeof(wasix_context_id_t) == 8, "context IDs must be u64");
_Static_assert(sizeof(void *) == 4, "this fixture pins wasm32 pointers");

enum { MAIN_OWNER = 0, A_OWNER = 1, B_OWNER = 2 };
enum { DESTROY_MAIN, DESTROY_SELF_A, DESTROY_MAIN_A, SWITCH_DEAD_A, SWITCH_DEAD_B, ERROR_COUNT };

static wasix_context_id_t main_id, a_id, b_id;
static unsigned a_rounds, b_rounds, a_ready, b_ready, a_done, b_done, a_destroyed;
static unsigned b_only, a_resumed_after_destroy, unwound_a, unwound_b;
static unsigned trace[TRACE_CAPACITY], trace_count;
static const char *phase = "initialization";
static const char *failure;
static int failure_return, failure_errno;

struct range {
    unsigned owner, depth;
    uintptr_t begin, end;
    volatile uint64_t *words;
    unsigned live;
};
static struct range ranges[RANGE_COUNT];
static unsigned range_count;

struct error_result { int result, error; unsigned recorded; };
static struct error_result errors[ERROR_COUNT];
static const char *error_names[ERROR_COUNT] = {
    "destroy_main_from_main", "destroy_self_a", "destroy_main_from_a",
    "switch_destroyed_a", "switch_destroyed_b"
};

static struct {
    unsigned main_self_switch, a_self_switch, destroy_main_rejected;
    unsigned destroy_self_a_rejected, destroy_main_from_a_rejected;
    unsigned ranges_disjoint, canaries_preserved, trace_matches;
    unsigned destroy_a_succeeded, switch_destroyed_a_rejected, destroy_a_twice_succeeded;
    unsigned b_continued_after_a_destroy, b_recursion_unwound;
    unsigned destroy_b_succeeded, switch_destroyed_b_rejected, destroy_b_twice_succeeded;
    unsigned a_recursion_unwound, main_clobber_after_a_unwind, b_preserved_after_a_unwind;
} checks;

static const char *boolean(unsigned value) { return value ? "true" : "false"; }
static const char *owner_name(unsigned owner) {
    return owner == MAIN_OWNER ? "main" : owner == A_OWNER ? "a" : "b";
}

static void emit_json(unsigned ok) {
    printf("{\"schema\":\"recastpress.wasix-context-c.v0.5\",\"ok\":%s,\"phase\":\"%s\",",
           boolean(ok), phase);
    if (failure) printf("\"failure\":\"%s\",", failure);
    else printf("\"failure\":null,");
    printf("\"failure_return\":%d,\"failure_errno\":%d,", failure_return, failure_errno);
    printf("\"abi\":{\"context_id_bytes\":%zu,\"pointer_bytes\":%zu,\"einval\":%d},",
           sizeof(wasix_context_id_t), sizeof(void *), EINVAL);
    printf("\"contexts\":{\"main\":%" PRIu64 ",\"a\":%" PRIu64 ",\"b\":%" PRIu64 "},",
           (uint64_t)main_id, (uint64_t)a_id, (uint64_t)b_id);
    printf("\"rounds\":%d,\"depth\":%d,\"a_rounds\":%u,\"b_rounds\":%u,",
           ROUNDS, DEPTH, a_rounds, b_rounds);
    printf("\"a_ready\":%s,\"b_ready\":%s,\"b_done\":%s,\"a_resumed_after_destroy\":%u,",
           boolean(a_ready), boolean(b_ready), boolean(b_done), a_resumed_after_destroy);
    printf("\"a_done\":%s,\"a_unwound_frames\":%u,\"b_unwound_frames\":%u,",
           boolean(a_done), unwound_a, unwound_b);
    printf("\"range_observation\":\"initial-simultaneous-live-snapshot\",");
    printf("\"range_count\":%u,\"ranges\":[", range_count);
    for (unsigned i = 0; i < range_count && i < RANGE_COUNT; ++i) {
        if (i) putchar(',');
        printf("{\"owner\":\"%s\",\"depth\":%u,\"begin\":%" PRIuPTR ",\"end\":%" PRIuPTR "}",
               owner_name(ranges[i].owner), ranges[i].depth, ranges[i].begin, ranges[i].end);
    }
    printf("],\"trace\":[");
    for (unsigned i = 0; i < trace_count && i < TRACE_CAPACITY; ++i) printf(i ? ",%u" : "%u", trace[i]);
    printf("],\"checks\":{");
#define CHECK_JSON(name, first) printf(first "\"" #name "\":%s", boolean(checks.name))
    CHECK_JSON(main_self_switch, "");
    CHECK_JSON(a_self_switch, ",");
    CHECK_JSON(destroy_main_rejected, ",");
    CHECK_JSON(destroy_self_a_rejected, ",");
    CHECK_JSON(destroy_main_from_a_rejected, ",");
    CHECK_JSON(ranges_disjoint, ",");
    CHECK_JSON(canaries_preserved, ",");
    CHECK_JSON(trace_matches, ",");
    CHECK_JSON(destroy_a_succeeded, ",");
    CHECK_JSON(switch_destroyed_a_rejected, ",");
    CHECK_JSON(destroy_a_twice_succeeded, ",");
    CHECK_JSON(b_continued_after_a_destroy, ",");
    CHECK_JSON(b_recursion_unwound, ",");
    CHECK_JSON(destroy_b_succeeded, ",");
    CHECK_JSON(switch_destroyed_b_rejected, ",");
    CHECK_JSON(destroy_b_twice_succeeded, ",");
    CHECK_JSON(a_recursion_unwound, ",");
    CHECK_JSON(main_clobber_after_a_unwind, ",");
    CHECK_JSON(b_preserved_after_a_unwind, ",");
#undef CHECK_JSON
    printf("},\"errors\":{");
    for (unsigned i = 0; i < ERROR_COUNT; ++i) {
        if (i) putchar(',');
        printf("\"%s\":", error_names[i]);
        if (errors[i].recorded) printf("{\"return\":%d,\"errno\":%d}", errors[i].result, errors[i].error);
        else printf("null");
    }
    printf("}}\n");
    if (fflush(stdout) != 0) _Exit(2);
}

static _Noreturn void fail(const char *reason, int result, int error) {
    failure = reason;
    failure_return = result;
    failure_errno = error;
    emit_json(0);
    _Exit(1);
}

static void require(unsigned condition, const char *reason) {
    if (!condition) fail(reason, 0, 0);
}

static void record_event(unsigned event) {
    require(trace_count < TRACE_CAPACITY, "trace-capacity");
    trace[trace_count++] = event;
}

static uint64_t canary(unsigned owner, unsigned depth, unsigned index) {
    return UINT64_C(0xa71e000000000000) ^ ((uint64_t)owner << 32) ^
           ((uint64_t)depth << 16) ^ (uint64_t)(index * 0x101u + 7u);
}

static void register_range(unsigned owner, unsigned depth, volatile uint64_t *words) {
    require(range_count < RANGE_COUNT, "range-capacity");
    uintptr_t begin = (uintptr_t)words;
    require(begin != 0 && begin <= UINTPTR_MAX - sizeof(uint64_t) * WORDS, "range-overflow");
    struct range *range = &ranges[range_count++];
    *range = (struct range){owner, depth, begin, begin + sizeof(uint64_t) * WORDS, words, 1};
    for (unsigned i = 0; i < WORDS; ++i) words[i] = canary(owner, depth, i);
}

static void check_live_ranges(void) {
    require(range_count <= RANGE_COUNT, "range-count-corrupted");
    for (unsigned i = 0; i < range_count; ++i) {
        const struct range *a = &ranges[i];
        if (!a->live) continue;
        for (unsigned j = i + 1; j < range_count; ++j) {
            const struct range *b = &ranges[j];
            if (b->live && !(a->end <= b->begin || b->end <= a->begin)) {
                checks.ranges_disjoint = 0;
                fail("live-linear-stack-ranges-overlap", 0, 0);
            }
        }
        for (unsigned word = 0; word < WORDS; ++word) {
            if (a->words[word] != canary(a->owner, a->depth, word)) {
                checks.canaries_preserved = 0;
                fail("volatile-stack-canary-changed", 0, 0);
            }
        }
    }
    if (range_count == RANGE_COUNT) {
        checks.ranges_disjoint = 1;
        checks.canaries_preserved = 1;
    }
}

static void switch_to(wasix_context_id_t target) {
    errno = 0;
    int result = wasix_context_switch(target);
    if (result != 0) fail("context-switch-failed", result, errno);
}

static void expect_einval(unsigned slot, int result, int error) {
    errors[slot] = (struct error_result){result, error, 1};
    if (result != -1 || error != EINVAL) fail("expected-EINVAL-not-observed", result, error);
}

static __attribute__((noinline)) void deepest(unsigned owner) {
    if (owner == A_OWNER) {
        a_ready = 1;
        record_event(10);
    } else {
        b_ready = 1;
        record_event(20);
    }
    switch_to(main_id);
    unsigned count = owner == A_OWNER ? ROUNDS : ROUNDS + B_EXTRA_ROUNDS;
    for (unsigned round = 0; round < count; ++round) {
        check_live_ranges();
        if (owner == A_OWNER) {
            if (a_destroyed) {
                ++a_resumed_after_destroy;
                fail("destroyed-A-resumed", 0, 0);
            }
            require(!b_only && a_rounds == round && b_rounds == round, "A-alternation-order");
            ++a_rounds;
            record_event(100 + round);
            switch_to(b_id);
        } else {
            if (round < ROUNDS) require(!b_only && a_rounds == round + 1, "B-alternation-order");
            else {
                require(b_only && a_destroyed && a_done && unwound_a == DEPTH,
                        "B-post-A-unwind-and-destroy-phase");
                require(checks.main_clobber_after_a_unwind, "B-resumed-before-post-A-unwind-clobber");
                checks.b_preserved_after_a_unwind = 1;
            }
            require(b_rounds == round, "B-round-order");
            ++b_rounds;
            record_event(200 + round);
            switch_to(main_id);
        }
    }
}

static __attribute__((noinline)) void walk(unsigned owner, unsigned depth) {
    volatile uint64_t words[WORDS];
    register_range(owner, depth, words);
    if (depth + 1 < DEPTH) walk(owner, depth + 1);
    else deepest(owner);
    for (unsigned i = 0; i < WORDS; ++i) {
        if (words[i] != canary(owner, depth, i)) fail("recursive-local-changed-on-unwind", 0, 0);
    }
    if (owner == B_OWNER) ++unwound_b;
    else ++unwound_a;
    /* No escaped pointer may be dereferenced after this frame returns. */
    for (unsigned i = 0; i < range_count; ++i) {
        if (ranges[i].owner == owner && ranges[i].depth == depth) ranges[i].live = 0;
    }
}

static void entry_a(void) {
    errno = 0;
    int result = wasix_context_switch(a_id);
    if (result != 0) fail("A-self-switch-failed", result, errno);
    checks.a_self_switch = 1;
    errno = 0;
    result = wasix_context_destroy(a_id);
    expect_einval(DESTROY_SELF_A, result, errno);
    checks.destroy_self_a_rejected = 1;
    errno = 0;
    result = wasix_context_destroy(main_id);
    expect_einval(DESTROY_MAIN_A, result, errno);
    checks.destroy_main_from_a_rejected = 1;
    walk(A_OWNER, 0);
    require(unwound_a == DEPTH && b_rounds == ROUNDS && !b_done, "A-unwind-lifetime-order");
    checks.a_recursion_unwound = 1;
    a_done = 1;
    record_event(40);
    switch_to(main_id);
    if (a_destroyed) ++a_resumed_after_destroy;
    fail("A-entrypoint-unexpectedly-resumed", 0, 0);
}

static void entry_b(void) {
    walk(B_OWNER, 0);
    require(unwound_b == DEPTH, "B-recursion-unwind-count");
    checks.b_recursion_unwound = 1;
    b_done = 1;
    record_event(30);
    switch_to(main_id);
    fail("B-entrypoint-unexpectedly-resumed", 0, 0);
}

static __attribute__((noinline)) void clobber(unsigned depth) {
    volatile unsigned char pressure[2048];
    for (unsigned i = 0; i < sizeof(pressure); ++i) pressure[i] = (unsigned char)(i ^ depth ^ 0x5a);
    check_live_ranges();
    if (depth) clobber(depth - 1);
    for (unsigned i = 0; i < sizeof(pressure); ++i) {
        require(pressure[i] == (unsigned char)(i ^ depth ^ 0x5a), "main-recursive-clobber-changed");
    }
}

int main(void) {
    volatile uint64_t main_words[WORDS];
    register_range(MAIN_OWNER, 0, main_words);
    main_id = wasix_context_main;
    errno = 0;
    int result = wasix_context_create(&a_id, entry_a);
    if (result != 0) fail("create-A-failed", result, errno);
    errno = 0;
    result = wasix_context_create(&b_id, entry_b);
    if (result != 0) fail("create-B-failed", result, errno);
    require(a_id != main_id && b_id != main_id && a_id != b_id, "context-ID-collision");
    switch_to(main_id);
    checks.main_self_switch = 1;
    errno = 0;
    result = wasix_context_destroy(main_id);
    expect_einval(DESTROY_MAIN, result, errno);
    checks.destroy_main_rejected = 1;
    phase = "warmup_a";
    switch_to(a_id);
    require(a_ready && !b_ready, "A-warmup-order");
    phase = "warmup_b";
    switch_to(b_id);
    require(a_ready && b_ready && range_count == RANGE_COUNT, "B-warmup-ranges");
    check_live_ranges();
    phase = "alternating";
    for (unsigned round = 0; round < ROUNDS; ++round) {
        clobber(2);
        switch_to(a_id);
        require(a_rounds == round + 1 && b_rounds == round + 1, "main-alternation-order");
        check_live_ranges();
    }
    /* A must release its recursive frames while B's frames remain live. A
     * monotonically lowered shared __stack_pointer is insufficient: after A
     * unwinds, main deliberately allocates/clobbers before B may resume. */
    phase = "a_unwind";
    switch_to(a_id);
    require(a_done && unwound_a == DEPTH && !b_done && b_rounds == ROUNDS,
            "A-did-not-unwind-while-B-suspended");
    phase = "post_a_unwind_clobber";
    clobber(2);
    check_live_ranges();
    checks.main_clobber_after_a_unwind = 1;
    phase = "destroy_a";
    a_destroyed = 1;
    errno = 0;
    result = wasix_context_destroy(a_id);
    if (result != 0) fail("destroy-A-failed", result, errno);
    checks.destroy_a_succeeded = 1;
    for (unsigned i = 0; i < range_count; ++i) if (ranges[i].owner == A_OWNER) ranges[i].live = 0;
    errno = 0;
    result = wasix_context_switch(a_id);
    expect_einval(SWITCH_DEAD_A, result, errno);
    checks.switch_destroyed_a_rejected = 1;
    errno = 0;
    result = wasix_context_destroy(a_id);
    if (result != 0) fail("destroy-A-twice-failed", result, errno);
    checks.destroy_a_twice_succeeded = 1;
    phase = "b_continuation";
    b_only = 1;
    for (unsigned round = 0; round < B_EXTRA_ROUNDS; ++round) {
        clobber(2);
        switch_to(b_id);
        require(b_rounds == ROUNDS + round + 1 && !a_resumed_after_destroy, "B-continuation-count");
        check_live_ranges();
    }
    checks.b_continued_after_a_destroy = 1;
    phase = "b_unwind";
    switch_to(b_id);
    require(b_done && checks.b_recursion_unwound, "B-did-not-unwind");
    check_live_ranges();
    phase = "destroy_b";
    errno = 0;
    result = wasix_context_destroy(b_id);
    if (result != 0) fail("destroy-B-failed", result, errno);
    checks.destroy_b_succeeded = 1;
    errno = 0;
    result = wasix_context_switch(b_id);
    expect_einval(SWITCH_DEAD_B, result, errno);
    checks.switch_destroyed_b_rejected = 1;
    errno = 0;
    result = wasix_context_destroy(b_id);
    if (result != 0) fail("destroy-B-twice-failed", result, errno);
    checks.destroy_b_twice_succeeded = 1;
    unsigned position = 0;
    require(trace_count == 2 + 2 * ROUNDS + 1 + B_EXTRA_ROUNDS + 1, "trace-length");
    require(trace[position++] == 10 && trace[position++] == 20, "trace-warmup");
    for (unsigned round = 0; round < ROUNDS; ++round) {
        require(trace[position++] == 100 + round && trace[position++] == 200 + round, "trace-alternating");
    }
    require(trace[position++] == 40, "trace-A-unwind");
    for (unsigned round = 0; round < B_EXTRA_ROUNDS; ++round) require(trace[position++] == 200 + ROUNDS + round, "trace-continuation");
    require(trace[position++] == 30 && position == trace_count, "trace-finish");
    checks.trace_matches = 1;
    phase = "complete";
    emit_json(1);
    return 0;
}
