#!/usr/bin/env python3
"""Generate typed Rust bindings for OVHcloud API v2 product branches.

Downloads schemas from https://eu.api.ovh.com/v2/{product}.json and emits one
module per product under src/v2/. Skips products that are hand-maintained
(domain, backupServices).
"""

from __future__ import annotations

import json
import re
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "src" / "v2"
BASE = "https://eu.api.ovh.com/v2"

SKIP = {"domain", "backupServices"}

SKIP_MODELS = {
    "iam.ResourceMetadata",
    "iam.ResourceMetadata.StateEnum",
}

PRODUCTS = [
    "commercialCatalog",
    "iam",
    "location",
    "managedCMS",
    "networkDefense",
    "notification",
    "okms",
    "publicCloud",
    "videocenter",
    "vmwareCloudDirector",
    "vrackServices",
    "webhosting",
    "zimbra",
]

RUST_KEYWORDS = {
    "as", "break", "const", "continue", "crate", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod",
    "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct",
    "super", "trait", "true", "type", "unsafe", "use", "where", "while",
    "async", "await", "dyn", "abstract", "become", "box", "do", "final",
    "macro", "override", "priv", "typeof", "unsized", "virtual", "yield",
}


def fetch(product: str) -> dict:
    url = f"{BASE}/{product}.json"
    with urllib.request.urlopen(url) as resp:
        return json.load(resp)


def snake(s: str) -> str:
    s = str(s)
    s = re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", s)
    s = re.sub(r"[^a-zA-Z0-9]+", "_", s)
    return s.lower().strip("_")


def pascal(s: str) -> str:
    parts = re.split(r"[^a-zA-Z0-9]+", s)
    return "".join(p[:1].upper() + p[1:] for p in parts if p)


def rust_ident(name: str) -> str:
    out = snake(name)
    if out in RUST_KEYWORDS:
        out = f"{out}_"
    if out == "type":
        return "kind"
    return out


def rust_type(ovh_type: str, models: dict, product: str) -> str:
    ovh_type = ovh_type.strip()
    if ovh_type == "void":
        return "()"
    if ovh_type == "string":
        return "String"
    if ovh_type == "uuid":
        return "String"
    if ovh_type == "long":
        return "i64"
    if ovh_type == "double":
        return "f64"
    if ovh_type == "boolean":
        return "bool"
    if ovh_type == "datetime":
        return "String"
    if ovh_type == "ipBlock":
        return "String"
    if ovh_type.startswith("map[") and ovh_type.endswith("]"):
        inner = ovh_type[4:-1]
        if inner == "string]string":
            return "std::collections::HashMap<String, String>"
        return "serde_json::Value"
    if ovh_type.endswith("[]"):
        inner = rust_type(ovh_type[:-2], models, product)
        return f"Vec<{inner}>"
    if ovh_type == "iam.ResourceMetadata":
        return "crate::iam::ResourceMetadata"
    if ovh_type == "iam.ResourceMetadata.StateEnum":
        return "crate::iam::ResourceState"
    if ovh_type in models:
        return model_rust_name(ovh_type, product)
    if "." in ovh_type:
        return "serde_json::Value"
    return "serde_json::Value"


def model_rust_name(full_id: str, product: str) -> str:
    parts = full_id.split(".")
    # Always include enough namespace to avoid collisions:
    # iam.policy.Response -> IamPolicyResponse
    # iam.PermissionsGroup -> IamPermissionsGroup
    # common.ResourceStatusEnum -> CommonResourceStatus
    if parts[-1].endswith("Enum"):
        stem = parts[-1].replace("Enum", "")
        body = "".join(pascal(p) for p in parts[:-1])
        return body + pascal(stem)
    if len(parts) == 1:
        return pascal(parts[0])
    return "".join(pascal(p) for p in parts[1:])


def topo_sort(models: dict) -> list[str]:
    deps: dict[str, set[str]] = {}
    for mid, spec in models.items():
        d: set[str] = set()
        if "enum" in spec:
            deps[mid] = d
            continue
        for prop in (spec.get("properties") or {}).values():
            ft = prop.get("fullType") or prop.get("type", "")
            collect_deps(ft, models, d)
        deps[mid] = d

    ordered: list[str] = []
    seen: set[str] = set()

    def visit(n: str) -> None:
        if n in seen:
            return
        seen.add(n)
        for dep in sorted(deps.get(n, ())):
            if dep in models:
                visit(dep)
        ordered.append(n)

    for mid in sorted(models):
        visit(mid)
    return ordered


def collect_deps(ft: str, models: dict, out: set[str]) -> None:
    if not ft or ft in ("string", "uuid", "long", "double", "boolean", "datetime", "void", "ipBlock"):
        return
    if ft.endswith("[]"):
        collect_deps(ft[:-2], models, out)
        return
    if ft.startswith("map["):
        return
    if ft in models:
        out.add(ft)


def emit_enum(mid: str, spec: dict, product: str) -> str:
    name = model_rust_name(mid, product)
    lines = [
        f"/// `{mid}`",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]",
        '#[serde(rename_all = "SCREAMING_SNAKE_CASE")]',
        f"pub enum {name} {{",
    ]
    for val in spec.get("enum", []):
        variant = pascal(snake(str(val)))
        if not variant or variant[0].isdigit():
            variant = f"V{variant}"
        if variant == "Unknown":
            variant = "UnknownValue"
        lines.append(f"    #[serde(rename = {json.dumps(str(val))})]")
        lines.append(f"    {variant},")
    values = {str(v).upper() for v in spec.get("enum", [])}
    if "OTHER" not in values:
        lines.append("    /// Value not present in the schema this crate was built against.")
        lines.append("    #[serde(other)]")
        lines.append("    Other,")
    lines.append("}")
    return "\n".join(lines)


def emit_struct(mid: str, spec: dict, product: str, models: dict) -> str:
    name = model_rust_name(mid, product)
    lines = [
        f"/// `{mid}`",
        "#[derive(Debug, Clone, Serialize, Deserialize)]",
        '#[serde(rename_all = "camelCase")]',
        f"pub struct {name} {{",
    ]
    props = spec.get("properties") or {}
    if not props:
        lines.append("    #[serde(flatten)]")
        lines.append("    pub extra: serde_json::Value,")
    used_fields: set[str] = set()
    for key, prop in props.items():
        field = rust_ident(key)
        if field in used_fields:
            field = f"{field}_value"
        used_fields.add(field)
        ft = prop.get("fullType") or prop.get("type", "string")
        ty = rust_type(ft, models, product)
        nullable = prop.get("canBeNull", False)
        read_only = prop.get("readOnly", False)
        required = prop.get("required", False)
        if nullable or not required:
            if not ty.startswith("Option<") and ty != "serde_json::Value":
                ty = f"Option<{ty}>"
        if ty.startswith("Vec<") or ty == "serde_json::Value":
            lines.append("    #[serde(default)]")
        if key == "type":
            field = "event_type" if "kind" in props else "kind"
            lines.append('    #[serde(rename = "type")]')
        lines.append(f"    pub {field}: {ty},")
    lines.append("}")
    return "\n".join(lines)


def method_name(product: str, path: str, method: str) -> str:
    # /iam/policy/{policyId} GET -> iam_policy
    rel = path
    prefix = f"/{product}"
    if product == "publicCloud":
        prefix = "/publicCloud"
    if rel.startswith(prefix):
        rel = rel[len(prefix) :]
    rel = rel.strip("/")
    parts = []
    for seg in rel.split("/"):
        if seg.startswith("{") and seg.endswith("}"):
            continue
        parts.append(snake(seg))
    base = "_".join(parts) if parts else snake(product)
    prefix_name = snake(product)
    if method == "GET" and "{" not in path.split("/")[-1]:
        fn = f"{prefix_name}_{base}" if base else prefix_name
        if path.count("{") == 0:
            return fn + "s" if not fn.endswith("s") else fn
        return fn
    if method == "GET":
        return f"{prefix_name}_{base}" if base else f"{prefix_name}_get"
    return f"{prefix_name}_{base}_{method.lower()}"


def emit_method(product: str, path: str, op: dict, models: dict) -> str:
    method = op["httpMethod"]
    fn = method_name(product, path, method)
    params = op.get("parameters", [])
    path_params = [p for p in params if p.get("paramType") == "path"]
    query_params = [p for p in params if p.get("paramType") == "query"]
    body_params = [p for p in params if p.get("paramType") == "body"]
    paginated = any(p.get("name") == "X-Pagination-Cursor" for p in params)

    resp = op.get("responseType", "void")
    ret_ty = "()" if resp == "void" else rust_type(resp.replace("[]", "") if resp.endswith("[]") else resp, models, product)
    is_list = resp.endswith("[]")
    if is_list:
        item_ty = rust_type(resp[:-2], models, product)
        ret_ty = f"Vec<{item_ty}>"

    # signature
    args = ["&self"]
    fmt_args = []
    path_expr = path
    for p in path_params:
        pname = rust_ident(p["name"])
        args.append(f"{pname}: &str")
        fmt_args.append(pname)
        placeholder = "{" + p["name"] + "}"
        path_expr = path_expr.replace(placeholder, "{}")
    if query_params:
        args.append("query: &[(&str, &str)]")
    if body_params:
        btype = body_params[0].get("fullType") or body_params[0].get("dataType", "serde_json::Value")
        bname = rust_type(btype, models, product)
        args.append(f"body: &{bname}")
    if paginated and method == "GET" and is_list:
        args.append("page: &PageParams")

    # build path format string with percent_encode
    path_build: list[str] = []
    format_args: list[str] = []
    for seg in path.strip("/").split("/"):
        if seg.startswith("{") and seg.endswith("}"):
            pname = rust_ident(seg[1:-1])
            path_build.append("{}")
            format_args.append(f"percent_encode({pname})")
        else:
            path_build.append(seg)
    path_literal = "/" + "/".join(path_build)
    if format_args:
        path_fmt = f'format!("{path_literal}", {", ".join(format_args)})'
    else:
        path_fmt = f'"{path}"'

    doc = op.get("description", "").replace("*/", "* /")
    lines = [
        f"    /// `{method} {path}` — {doc}",
        f"    pub async fn {fn}({', '.join(args)}) -> Result<{ret_ty}> {{",
    ]

    call_path = path_fmt
    if query_params and method == "GET":
        qpath = f"Self::append_query(&{call_path}, query)"
        if paginated and is_list and "page: &PageParams" in ", ".join(args):
            lines.append(f"        self.get_page(&{qpath}, &[], page).await.map(|p| p.items)")
        elif is_list:
            lines.append(f"        self.get_all(&{qpath}, &[]).await")
        else:
            lines.append(f"        self.get(&{qpath}).await")
    elif method == "GET":
        if paginated and is_list and "page: &PageParams" in ", ".join(args):
            lines.append(f"        self.get_page(&{call_path}, &[], page).await.map(|p| p.items)")
        elif is_list:
            lines.append(f"        self.get_all(&{call_path}, &[]).await")
        else:
            lines.append(f"        self.get(&{call_path}).await")
    elif method == "PUT" and ret_ty == "()":
        if body_params:
            lines.append(f"        self.put(&{call_path}, body).await")
        else:
            lines.append(f"        todo!(\"PUT without body {path}\")")
    elif method == "PUT":
        lines.append(f"        self.put_json(&{call_path}, body).await")
    elif method == "POST" and ret_ty == "()":
        if body_params:
            lines.append(f"        self.post_void(&{call_path}, body).await")
        else:
            lines.append(
                f"        self.post_v2_no_body_void(&{call_path}, V2RequestOptions::default()).await"
            )
    elif method == "POST" and body_params:
        lines.append(
            f"        self.post_v2(&{call_path}, body, V2RequestOptions::default()).await"
        )
    elif method == "POST":
        lines.append(
            f"        self.post_v2_no_body(&{call_path}, V2RequestOptions::default()).await"
        )
    elif method == "DELETE" and ret_ty != "()":
        lines.append(f"        self.delete_json(&{call_path}).await")
    elif method == "DELETE":
        lines.append(f"        self.delete(&{call_path}).await")
    else:
        lines.append(f"        todo!(\"{method} {path}\")")

    lines.append("    }")
    return "\n".join(lines)


def generate_product(product: str, schema: dict) -> str:
    models = schema.get("models", {})
    ordered = topo_sort(models)

    mod_name = snake(product)
    lines = [
        f"//! OVHcloud API v2 **{product}** bindings (`/v2/{product}`).",
        "//!",
        "//! Generated from the official schema; do not edit by hand.",
        "",
        "#![allow(unused_imports, unused_variables)]",
        "",
        "use crate::client::{percent_encode, OvhClient, PageParams, V2RequestOptions};",
        "use crate::error::Result;",
        "use serde::{{Deserialize, Serialize}};",
        "",
    ]

    if product == "iam":
        lines.extend([
            "pub use crate::iam::{ResourceMetadata, ResourceState};",
            "",
        ])

    for mid in ordered:
        if mid in SKIP_MODELS:
            continue
        spec = models[mid]
        if "enum" in spec:
            lines.append(emit_enum(mid, spec, product))
        else:
            lines.append(emit_struct(mid, spec, product, models))
        lines.append("")

    lines.append("impl OvhClient {")
    seen_fns: set[str] = set()
    for api in schema.get("apis", []):
        path = api["path"]
        for op in api.get("operations", []):
            fn = method_name(product, path, op["httpMethod"])
            if fn in seen_fns:
                fn = f"{fn}_{op['httpMethod'].lower()}"
            seen_fns.add(fn)
            # patch function name into emit_method by temporarily overriding
            body = emit_method(product, path, op, models)
            body = body.replace(
                f"pub async fn {method_name(product, path, op['httpMethod'])}(",
                f"pub async fn {fn}(",
                1,
            )
            lines.append(body)
            lines.append("")
    lines.append("}")

    return "\n".join(lines)


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    mod_lines = [
        "//! OVHcloud API v2 product bindings (generated + hand-maintained elsewhere).",
        "",
    ]
    for product in PRODUCTS:
        if product in SKIP:
            continue
        print(f"generating {product}...", file=sys.stderr)
        schema = fetch(product)
        rust = generate_product(product, schema)
        fname = snake(product) + ".rs"
        (OUT / fname).write_text(rust)
        mod_lines.append(f"pub mod {snake(product)};")
        mod_lines.append("")

    (OUT / "mod.rs").write_text("\n".join(mod_lines))
    print(f"wrote {len(PRODUCTS)} modules to {OUT}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
