/* Closed NTS/fresh-WASI Zend Fiber candidate; not a general PHP ABI.
 * Final post-optimizer bodies must be inspected before guest execution.
 * The bootstrap word is ordinary linear memory; __stack_pointer is the real
 * mutable Wasm global. Never call C or store to the linear stack before entry
 * installs its own SP. Never change to the target SP in the switch shim. */
.globaltype __stack_pointer, i32
.functype rp_zend_wasix_import_switch (i64) -> (i32)
.import_module rp_zend_wasix_import_switch, wasix_32v1
.import_name rp_zend_wasix_import_switch, context_switch
.functype rp_zend_wasix_dispatch () -> ()

.globl rp_zend_wasix_switch_raw
rp_zend_wasix_switch_raw:
  .functype rp_zend_wasix_switch_raw (i64) -> (i32)
  .local i32, i32
  global.get __stack_pointer
  local.set 1
  local.get 0
  call rp_zend_wasix_import_switch
  local.set 2
  local.get 1
  global.set __stack_pointer
  local.get 2
  end_function

.globl rp_zend_wasix_entry
rp_zend_wasix_entry:
  .functype rp_zend_wasix_entry () -> ()
  i32.const rp_zend_wasix_bootstrap_sp
  i32.load 0
  global.set __stack_pointer
  call rp_zend_wasix_dispatch
  unreachable
  end_function
