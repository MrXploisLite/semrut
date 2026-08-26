p = 'src/mir/mod.rs'
s = open(p).read()

helper = """
/// Emit a cast when an integer value's width does not match the expected type.
/// Enum discriminants are i64 while a function may declare i32, so returns and
/// assignments need an explicit narrow/widen instead of a type-mismatched IR.
fn coerce_int_width(
    val: MirValue,
    from: &MirType,
    to: &MirType,
    stmts: &mut Vec<MirStmt>,
    temp_counter: &mut usize,
) -> MirValue {
    let int_width = |t: &MirType| match t {
        MirType::I8 | MirType::U8 => Some(8),
        MirType::I16 | MirType::U16 => Some(16),
        MirType::I32 | MirType::U32 => Some(32),
        MirType::I64 | MirType::U64 | MirType::Enum(_) => Some(64),
        _ => None,
    };
    match (int_width(from), int_width(to)) {
        (Some(a), Some(b)) if a != b => {
            let dest = format!("_coerce{}", temp_counter);
            *temp_counter += 1;
            stmts.push(MirStmt::Cast {
                dest: dest.clone(),
                value: val,
                target_ty: to.clone(),
            });
            MirValue::Var(dest)
        }
        _ => val,
    }
}

// -- Expression Builder --
"""

anchor = "// \u2500\u2500\u2500 Expression Builder \u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\n"
assert anchor in s, "anchor not found"
s = s.replace(anchor, helper, 1)
open(p, 'w').write(s)
print("helper added")
