/* Wasm32 assembly only. No C prologue or linear-stack stores are allowed here.
 * The final post-Binaryen bodies are checked by the Rust host before compile. */
.globaltype __stack_pointer, i32
.functype rp_context_import_switch (i64) -> (i32)
.import_module rp_context_import_switch, wasix_32v1
.import_name rp_context_import_switch, context_switch
.functype rp_stack_dispatch_a () -> ()
.functype rp_stack_dispatch_b () -> ()

.globl rp_stack_switch_raw
rp_stack_switch_raw:
  .functype rp_stack_switch_raw (i64) -> (i32)
  .local i32, i32
  global.get __stack_pointer
  local.set 1
  local.get 0
  call rp_context_import_switch
  local.set 2
  local.get 1
  global.set __stack_pointer
  local.get 2
  end_function

.globl rp_stack_entry_a
rp_stack_entry_a:
  .functype rp_stack_entry_a () -> ()
  i32.const rp_stack_top_a
  i32.load 0
  global.set __stack_pointer
  call rp_stack_dispatch_a
  unreachable
  end_function

.globl rp_stack_entry_b
rp_stack_entry_b:
  .functype rp_stack_entry_b () -> ()
  i32.const rp_stack_top_b
  i32.load 0
  global.set __stack_pointer
  call rp_stack_dispatch_b
  unreachable
  end_function
