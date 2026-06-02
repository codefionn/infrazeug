use infrazeug_templates::template;

fn main() {
    // `@for` header has no `in` keyword — the macro rejects it at parse time.
    let _ = template!("@for x y {z}");
}
