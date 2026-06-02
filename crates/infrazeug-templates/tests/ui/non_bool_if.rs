use infrazeug_templates::template;

fn main() {
    let n = 3i32;
    // `@if` lowers to a real Rust `if`, so a non-bool condition is a type error.
    let _ = template!("@if n {x}", n = n);
}
