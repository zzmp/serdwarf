use dwarfdump::{ Function, Typed, Modifier, TypedValue };

pub fn check(function: &Function, allow_char_str: bool) -> bool {
    check_typed(&function.typed, allow_char_str) &&
        function.parameters.iter().all(|ref t| check_typed(&t.typed, allow_char_str))
}

fn check_typed(typed: &Typed, allow_char_str: bool) -> bool {
    if typed.modifiers.iter().any(|ref m| match **m {
        Modifier::Pointer => true,
        _ => false
    }) {
        allow_char_str && 
            typed.modifiers.iter().fold(0, |count, ref m| {
                count + match **m {
                    Modifier::Pointer => 1,
                    _ => 0
                }
            }) == 1 &&
            typed.name == "char" &&
            match typed.value {
                TypedValue::Base => true,
                _ => false
            }
    } else {
        match typed.value {
            TypedValue::Base | TypedValue::Enum => true,
            TypedValue::Typedef(ref nested) | TypedValue::Array(ref nested, _) =>
                check_typed(nested.as_ref(), allow_char_str),
            TypedValue::Struct(ref members) | TypedValue::Union(ref members) =>
                members.iter().all(|ref m| check_typed(&m.typed, allow_char_str)),
            TypedValue::Function(_) | TypedValue::Circular => false
        }
    }
}

