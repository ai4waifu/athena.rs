//! 内置函数注册表冒烟测试。

use athena_engine::lookup_function;

#[test]
fn registry_contains_builtin_names() {
    for name in ["Exp", "Sin", "Sinh", "ArcTan", "Gamma", "Erf", "Abs", "Sign"] {
        assert!(lookup_function(name).is_some(), "missing {name}");
    }
}
