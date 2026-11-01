;; Original WASI functions must see the PHP instance's memory as their caller's
;; export. A host-to-host Func::call has no such caller, so delegate through this
;; fixed module sharing that same memory. No PHP source or guest pointers change.
(module
  (import "php" "memory" (memory 0))
  (import "original" "path_open" (func $open
    (param i32 i32 i32 i32 i32 i64 i64 i32 i32) (result i32)))
  (import "original" "fd_write" (func $write
    (param i32 i32 i32 i32) (result i32)))
  (import "original" "fd_close" (func $close (param i32) (result i32)))
  (export "memory" (memory 0))
  (func (export "path_open")
    (param i32 i32 i32 i32 i32 i64 i64 i32 i32) (result i32)
    local.get 0 local.get 1 local.get 2 local.get 3 local.get 4
    local.get 5 local.get 6 local.get 7 local.get 8 call $open)
  (func (export "fd_write") (param i32 i32 i32 i32) (result i32)
    local.get 0 local.get 1 local.get 2 local.get 3 call $write)
  (func (export "fd_close") (param i32) (result i32)
    local.get 0 call $close))
