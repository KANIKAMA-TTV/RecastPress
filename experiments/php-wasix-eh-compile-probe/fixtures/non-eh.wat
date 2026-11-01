(module
  (import "env" "callee" (func $callee))
  (func (export "_start")
    call $callee))
