(module
  (import "env" "callee" (func $callee))
  (func (export "_start")
    block $caught (result exnref)
      try_table (catch_all_ref $caught)
        call $callee
      end
      return
    end
    drop))
