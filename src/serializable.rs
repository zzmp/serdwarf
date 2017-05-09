use dwarfdump::{ Function, Typed, Modifier, TypedValue };
use Flags;

pub fn check(function: &Function, flags: &Flags) -> bool {
    check_typed(&function.typed, flags) &&
        function.parameters.iter().all(|ref t| check_typed(&t.typed, flags))
}

fn check_typed(typed: &Typed, flags: &Flags) -> bool {
    if typed.modifiers.iter().any(|ref m| match **m {
        Modifier::Pointer => true,
        _ => false
    }) {
        let allow_char_str = flags.allow_char_str && typed.name == "char";
        let allow_void_str = flags.allow_void_str && typed.name == "void";
        (allow_char_str || allow_void_str || flags.allow_basic_str) &&
            typed.modifiers.iter().fold(0, |count, ref m| {
                count + match **m {
                    Modifier::Pointer => 1,
                    _ => 0
                }
            }) == 1 &&
            match typed.value {
                TypedValue::Base => true,
                _ => false
            }
    } else {
        match typed.value {
            TypedValue::Base | TypedValue::Enum => true,
            TypedValue::Typedef(ref nested) | TypedValue::Array(ref nested, _) =>
                check_typed(nested.as_ref(), flags),
            TypedValue::Struct(ref members) | TypedValue::Union(ref members) =>
                members.iter().all(|ref m| check_typed(&m.typed, flags)),
            TypedValue::Function(_) | TypedValue::Circular => false
        }
    }
}

