use infrazeug_templates::{escape, template};

#[test]
fn plain_text_passthrough() {
    let s = template!("hello world\n");
    assert_eq!(s, "hello world\n");
}

#[test]
fn interpolation_uses_in_scope_binding() {
    let port = 8443u16;
    let s = template!("listen = {{ port }}\n");
    assert_eq!(s, "listen = 8443\n");
}

#[test]
fn named_bindings() {
    let s = template!("a={{ x }} b={{ y }}", x = 1 + 2, y = "ok");
    assert_eq!(s, "a=3 b=ok");
}

#[test]
fn method_calls_act_as_filters() {
    let name = "pi";
    let s = template!("{{ name.to_uppercase() }}");
    assert_eq!(s, "PI");
}

#[test]
fn for_loop_over_slice() {
    let xs = [1, 2, 3];
    let s = template!("@for n in &xs {{{ n }};}", xs = xs);
    assert_eq!(s, "1;2;3;");
}

#[test]
fn if_else_branches() {
    fn render(flag: bool) -> String {
        template!("@if flag {on} @else {off}", flag = flag)
    }
    assert_eq!(render(true), "on");
    assert_eq!(render(false), "off");
}

#[test]
fn else_if_chain() {
    fn render(n: i32) -> String {
        template!("@if n > 1 {big} @else if n == 1 {one} @else {small}", n = n)
    }
    assert_eq!(render(5), "big");
    assert_eq!(render(1), "one");
    assert_eq!(render(0), "small");
}

#[test]
fn escapes() {
    let s = template!("@@ @{ @}");
    assert_eq!(s, "@ { }");
}

#[test]
fn struct_field_access_in_loop() {
    struct Host {
        name: &'static str,
        ip: &'static str,
    }
    let hosts = vec![
        Host {
            name: "a",
            ip: "10.0.0.1",
        },
        Host {
            name: "b",
            ip: "10.0.0.2",
        },
    ];
    let s = template!(
        "@for h in &hosts {{{ h.name }}={{ h.ip }}\n}",
        hosts = hosts
    );
    assert_eq!(s, "a=10.0.0.1\nb=10.0.0.2\n");
}

#[test]
fn escape_helpers() {
    let val = "it's";
    let s = template!(
        "x={{ escape::shell(val) }} y={{ escape::yaml_squote(val) }}",
        val = val
    );
    assert_eq!(s, "x='it'\\''s' y='it''s'");
}

/// Port of the nebula static_host_map shape from ../infra to lock real-world output.
#[test]
fn nebula_static_host_map_fixture() {
    struct Lh {
        nebula_ip: &'static str,
        public: &'static str,
    }
    let lighthouses = vec![
        Lh {
            nebula_ip: "10.10.0.1",
            public: "203.0.113.1:4242",
        },
        Lh {
            nebula_ip: "10.10.0.2",
            public: "203.0.113.2:4242",
        },
    ];
    let am_lighthouse = false;
    let out = template!(
        "lighthouse:\n  am_lighthouse: {{ am_lighthouse }}\nstatic_host_map:\n@for lh in &lighthouses {  \"{{ lh.nebula_ip }}\": [\"{{ lh.public }}\"]\n}",
        lighthouses = lighthouses,
        am_lighthouse = am_lighthouse
    );
    let expected = "lighthouse:\n  am_lighthouse: false\nstatic_host_map:\n  \"10.10.0.1\": [\"203.0.113.1:4242\"]\n  \"10.10.0.2\": [\"203.0.113.2:4242\"]\n";
    assert_eq!(out, expected);
}

#[test]
fn compile_fail_cases() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
}
