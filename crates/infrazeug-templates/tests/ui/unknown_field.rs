use infrazeug_templates::template;

struct V {
    name: String,
}

fn main() {
    let v = V { name: "x".into() };
    // `nope` is not a field of `V` — rustc rejects the generated field access.
    let _ = template!("{{ v.nope }}", v = v);
}
