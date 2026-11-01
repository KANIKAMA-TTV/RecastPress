#ifndef RECASTPRESS_FIXED_CONTEXT_STACK_ADAPTER_H
#define RECASTPRESS_FIXED_CONTEXT_STACK_ADAPTER_H

/* Forced into the unchanged diagnostic fixture ONLY. Adapter objects are
 * compiled separately, without this header or these redirection macros. */
#include <wasix/context.h>

int rp_context_create(wasix_context_id_t *, void (*)(void));
int rp_context_switch(wasix_context_id_t);
int rp_context_destroy(wasix_context_id_t);

#define wasix_context_create rp_context_create
#define wasix_context_switch rp_context_switch
#define wasix_context_destroy rp_context_destroy

#endif
