from __future__ import annotations

import inspect
import json
import os
import re
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from xml.etree import ElementTree

DOCS_ROOT = Path(__file__).resolve().parents[1]
REPO_ROOT = DOCS_ROOT.parent
GENERATED_ROOT = DOCS_ROOT / ".generated" / "api"
REFERENCE_PATH = DOCS_ROOT / "src" / "data" / "api-reference.json"

PUBLIC_KINDS = {
    "struct": "struct",
    "enum": "enum",
    "function": "function",
    "constant": "constant",
    "type_alias": "type alias",
    "typedef": "type alias",
    "class": "class",
}


@dataclass(frozen=True)
class RustdocTarget:
    package: str
    language: str
    cargo_package: str
    crate_json_name: str
    source_path: str
    docs_rs_base: str | None = None
    target: str | None = None


@dataclass(frozen=True)
class PythonTarget:
    package: str
    module: str
    source_path: str
    python_path: Path


@dataclass(frozen=True)
class DoxygenTarget:
    package: str
    language: str
    name: str
    inputs: list[Path]
    source_path: str
    cbindgen_crate: Path | None = None


def main() -> None:
    GENERATED_ROOT.mkdir(parents=True, exist_ok=True)
    entries: list[dict[str, object]] = []

    for target in rustdoc_targets():
        entries.extend(extract_rustdoc(target))
    for target in python_targets():
        entries.extend(extract_python(target))
    for target in doxygen_targets():
        entries.extend(extract_doxygen(target))

    entries = sorted(unique_entries(entries), key=lambda item: (str(item["package"]), str(item["language"]), str(item["name"])))
    if not entries:
        raise RuntimeError("API extraction produced no entries")

    REFERENCE_PATH.parent.mkdir(parents=True, exist_ok=True)
    REFERENCE_PATH.write_text(
        json.dumps({"schemaVersion": 1, "entries": entries}, indent=2) + "\n",
        encoding="utf-8",
    )
    print(f"wrote {REFERENCE_PATH.relative_to(REPO_ROOT)} with {len(entries)} entries")


def rustdoc_targets() -> list[RustdocTarget]:
    return [
        RustdocTarget(
            package="cityjson-lib",
            language="rust",
            cargo_package="cityjson-lib",
            crate_json_name="cityjson_lib",
            source_path="crates/cityjson-lib/src/lib.rs",
            docs_rs_base="https://docs.rs/cityjson-lib/latest/cityjson_lib/",
        ),
        RustdocTarget(
            package="cityjson-index",
            language="rust",
            cargo_package="cityjson-index",
            crate_json_name="cityjson_index",
            source_path="crates/cityjson-index/src/lib.rs",
            docs_rs_base="https://docs.rs/cityjson-index/latest/cityjson_index/",
        ),
        RustdocTarget(
            package="cityjson-lib",
            language="wasm",
            cargo_package="cityjson-lib-wasm",
            crate_json_name="cityjson_lib_wasm",
            source_path="crates/cityjson-lib/ffi/wasm/src/lib.rs",
            target="wasm32-unknown-unknown",
        ),
    ]


def python_targets() -> list[PythonTarget]:
    return [
        PythonTarget(
            package="cityjson-lib",
            module="cityjson_lib",
            source_path="crates/cityjson-lib/ffi/python/src/cityjson_lib/__init__.py",
            python_path=REPO_ROOT / "crates/cityjson-lib/ffi/python/src",
        ),
        PythonTarget(
            package="cityjson-index",
            module="cityjson_index",
            source_path="crates/cityjson-index/ffi/python/src/cityjson_index/__init__.py",
            python_path=REPO_ROOT / "crates/cityjson-index/ffi/python/src",
        ),
    ]


def doxygen_targets() -> list[DoxygenTarget]:
    return [
        DoxygenTarget(
            package="cityjson-lib",
            language="cpp",
            name="cityjson-lib-cpp",
            inputs=[REPO_ROOT / "crates/cityjson-lib/ffi/cpp/include/cityjson_lib/cityjson_lib.hpp"],
            source_path="crates/cityjson-lib/ffi/cpp/include/cityjson_lib/cityjson_lib.hpp",
        ),
        DoxygenTarget(
            package="cityjson-lib",
            language="c",
            name="cityjson-lib-c",
            inputs=[REPO_ROOT / "crates/cityjson-lib/ffi/core/include/cityjson_lib/cityjson_lib.h"],
            source_path="crates/cityjson-lib/ffi/core/include/cityjson_lib/cityjson_lib.h",
        ),
        DoxygenTarget(
            package="cityjson-index",
            language="c",
            name="cityjson-index-c",
            inputs=[],
            source_path="crates/cityjson-index/ffi/core/include/cityjson_index/cityjson_index.h",
            cbindgen_crate=REPO_ROOT / "crates/cityjson-index/ffi/core",
        ),
    ]


def extract_rustdoc(target: RustdocTarget) -> list[dict[str, object]]:
    if shutil.which("cargo") is None:
        raise RuntimeError("cargo is required for rustdoc JSON extraction")

    args = [
        "cargo",
        "+nightly",
        "rustdoc",
        "-p",
        target.cargo_package,
        "--lib",
        "--all-features",
    ]
    if target.target is not None:
        args.extend(["--target", target.target])
    args.extend(["--", "-Z", "unstable-options", "--output-format", "json"])
    run(args, cwd=REPO_ROOT)

    json_path = REPO_ROOT / "target" / "doc" / f"{target.crate_json_name}.json"
    if target.target is not None:
        json_path = REPO_ROOT / "target" / target.target / "doc" / f"{target.crate_json_name}.json"
    if not json_path.exists():
        raise RuntimeError(f"rustdoc JSON was not produced at {json_path}")

    data = json.loads(json_path.read_text(encoding="utf-8"))
    index: dict[str, dict[str, object]] = data.get("index", {})
    paths: dict[str, dict[str, object]] = data.get("paths", {})
    entries: list[dict[str, object]] = []

    for item_id, item in index.items():
        if not is_public_rust_item(item):
            continue
        inner = item.get("inner", {})
        kind = rust_kind(inner)
        if kind is None:
            continue
        path = rust_path(item_id, item, paths, target.crate_json_name)
        member_name = path[-1]
        owner_from_span = rust_owner_from_span(item) if kind == "function" else None
        if owner_from_span is not None:
            path = [target.crate_json_name, owner_from_span, member_name]
            kind = "method"
        if should_skip_rust_path(path):
            continue
        name = "::".join(path)
        owner_label = rust_owner_label(path, kind)
        signature = rust_source_signature(item) or rust_signature(member_name, kind, inner)
        entries.append(
            entry(
                package=target.package,
                language=target.language,
                name=name,
                member_name=member_name,
                kind=kind,
                signature=signature,
                docs=rust_docs_markdown(str(item.get("docs") or "")),
                source_path=target.source_path,
                owner_label=owner_label,
                docs_rs_url=rust_docs_url(target, path, kind),
            )
        )
    if target.package == "cityjson-lib" and target.language == "rust":
        entries.extend(cityjson_lib_facade_entries(target))
    return entries


def cityjson_lib_facade_entries(target: RustdocTarget) -> list[dict[str, object]]:
    return [
        entry(
            package=target.package,
            language=target.language,
            name="cityjson_lib::Model",
            member_name="Model",
            kind="type alias",
            signature="pub use cityjson_types::v2_0::OwnedCityModel as Model",
            docs="High-level CityJSON model facade re-exported by cityjson-lib.",
            source_path=target.source_path,
            owner_label="Model",
            docs_rs_url=f"{target.docs_rs_base}type.Model.html" if target.docs_rs_base else None,
        ),
        entry(
            package=target.package,
            language=target.language,
            name="cityjson_lib::Model::parse_document",
            member_name="parse_document",
            kind="method",
            signature="pub fn parse_document(data: &[u8]) -> Result<Model>",
            docs="Parses a CityJSON document into a model.",
            source_path=target.source_path,
            owner_label="Model",
            docs_rs_url=target.docs_rs_base,
        ),
        entry(
            package=target.package,
            language=target.language,
            name="cityjson_lib::query::summary",
            member_name="summary",
            kind="function",
            signature="pub fn summary(model: &Model) -> ModelSummary",
            docs="Returns a summary of model contents.",
            source_path=target.source_path,
            owner_label="cityjson_lib::query",
            docs_rs_url=f"{target.docs_rs_base}query/fn.summary.html" if target.docs_rs_base else None,
        ),
    ]


def is_public_rust_item(item: dict[str, object]) -> bool:
    visibility = item.get("visibility")
    if visibility == "public":
        return True
    if isinstance(visibility, dict) and "public" in visibility:
        return True
    return False


def rust_kind(inner: object) -> str | None:
    if not isinstance(inner, dict) or len(inner) != 1:
        return None
    raw_kind = next(iter(inner.keys()))
    return PUBLIC_KINDS.get(raw_kind)


def rust_path(item_id: str, item: dict[str, object], paths: dict[str, dict[str, object]], crate_name: str) -> list[str]:
    path_data = paths.get(item_id, {})
    path = path_data.get("path")
    if isinstance(path, list) and path:
        parts = [str(part) for part in path]
    else:
        name = str(item.get("name") or "")
        parts = [crate_name, name] if name else [crate_name]
    if parts[0] != crate_name:
        parts.insert(0, crate_name)
    return parts


def should_skip_rust_path(path: list[str]) -> bool:
    return any(part.startswith("_") for part in path) or "benchmark" in path or "profile" in path


def rust_owner_label(path: list[str], kind: str) -> str:
    if kind == "method" and len(path) > 1:
        return path[-2]
    if len(path) > 2:
        return "::".join(path[:-1])
    if kind in {"struct", "enum", "class"}:
        return path[-1]
    return "Functions"


def rust_owner_from_span(item: dict[str, object]) -> str | None:
    span = item.get("span")
    if not isinstance(span, dict):
        return None
    begin = span.get("begin")
    filename = span.get("filename")
    if not isinstance(begin, list) or len(begin) < 2 or not isinstance(filename, str):
        return None
    column = int(begin[1])
    if column <= 1:
        return None
    source_path = REPO_ROOT / filename
    if not source_path.exists():
        return None
    lines = source_path.read_text(encoding="utf-8").splitlines()
    line_index = int(begin[0]) - 1
    for index in range(line_index, -1, -1):
        match = re.match(r"(?:pub\s+)?impl(?:<[^>]+>)?\s+([A-Za-z][A-Za-z0-9_]*)", lines[index].strip())
        if match:
            return match.group(1)
    return None


def rust_source_signature(item: dict[str, object]) -> str:
    span = item.get("span")
    if not isinstance(span, dict):
        return ""
    begin = span.get("begin")
    filename = span.get("filename")
    if not isinstance(begin, list) or not isinstance(filename, str):
        return ""
    source_path = REPO_ROOT / filename
    if not source_path.exists():
        return ""
    lines = source_path.read_text(encoding="utf-8").splitlines()
    start = int(begin[0]) - 1
    collected: list[str] = []
    paren_depth = 0
    for line in lines[start:]:
        stripped = line.strip()
        if stripped.startswith("#") or stripped.startswith("///"):
            continue
        collected.append(stripped)
        paren_depth += stripped.count("(") - stripped.count(")")
        if paren_depth <= 0 and (stripped.endswith("{") or stripped.endswith(";") or " where " not in stripped):
            break
        if len(collected) > 16:
            break
    signature = " ".join(collected).removesuffix("{").strip()
    return compact_text(signature)


def rust_signature(member_name: str, kind: str, inner: object) -> str:
    if kind in {"function", "method"}:
        return f"pub fn {member_name}(...)"
    if kind == "struct":
        return f"pub struct {member_name}"
    if kind == "enum":
        return f"pub enum {member_name}"
    if kind == "constant":
        return f"pub const {member_name}"
    if kind == "type alias":
        return f"pub type {member_name}"
    return member_name


def rust_docs_url(target: RustdocTarget, path: list[str], kind: str) -> str | None:
    if target.docs_rs_base is None:
        return None
    member_name = path[-1]
    if kind == "struct":
        return f"{target.docs_rs_base}struct.{member_name}.html"
    if kind == "enum":
        return f"{target.docs_rs_base}enum.{member_name}.html"
    if kind == "type alias":
        return f"{target.docs_rs_base}type.{member_name}.html"
    if kind == "constant":
        return f"{target.docs_rs_base}constant.{member_name}.html"
    if kind == "function":
        return f"{target.docs_rs_base}fn.{member_name}.html"
    return target.docs_rs_base


def extract_python(target: PythonTarget) -> list[dict[str, object]]:
    xml_dir = run_sphinx(target)
    xml_entries = parse_sphinx_xml(target, xml_dir)
    if not xml_entries:
        raise RuntimeError(f"Sphinx produced no Python API entries for {target.module}")
    return xml_entries


def run_sphinx(target: PythonTarget) -> Path:
    src_dir = GENERATED_ROOT / "sphinx" / target.module / "src"
    out_dir = GENERATED_ROOT / "sphinx" / target.module / "xml"
    if src_dir.exists():
        shutil.rmtree(src_dir)
    if out_dir.exists():
        shutil.rmtree(out_dir)
    src_dir.mkdir(parents=True)
    out_dir.mkdir(parents=True)
    (src_dir / "conf.py").write_text(
        "extensions = ['sphinx.ext.autodoc', 'sphinx_autodoc_typehints']\n"
        "autodoc_member_order = 'bysource'\n"
        "autodoc_typehints = 'signature'\n"
        "add_module_names = False\n"
        "nitpicky = False\n",
        encoding="utf-8",
    )
    (src_dir / "index.rst").write_text(
        f"{target.module}\n{'=' * len(target.module)}\n\n.. automodule:: {target.module}\n   :members:\n   :undoc-members:\n",
        encoding="utf-8",
    )

    env = os.environ.copy()
    env["CITYJSON_DOCS_IMPORT"] = "1"
    python_paths = [str(target.python_path), str(REPO_ROOT / "crates/cityjson-lib/ffi/python/src")]
    if env.get("PYTHONPATH"):
        python_paths.append(env["PYTHONPATH"])
    env["PYTHONPATH"] = os.pathsep.join(python_paths)
    run(["uv", "run", "sphinx-build", "-b", "xml", str(src_dir), str(out_dir)], cwd=DOCS_ROOT, env=env)
    return out_dir


def parse_sphinx_xml(target: PythonTarget, xml_dir: Path) -> list[dict[str, object]]:
    xml_path = xml_dir / "index.xml"
    if not xml_path.exists():
        raise RuntimeError(f"Sphinx XML is missing {xml_path}")
    root = ElementTree.parse(xml_path).getroot()
    entries: list[dict[str, object]] = []

    for desc in root.iter():
        if local_name(desc.tag) != "desc" or desc.attrib.get("domain") != "py":
            continue
        objtype = desc.attrib.get("objtype")
        if objtype not in {"class", "function", "method"}:
            continue
        signature_node = first_child(desc, "desc_signature")
        if signature_node is None:
            continue
        fullname = signature_node.attrib.get("fullname") or signature_node.attrib.get("module") or ""
        ids = signature_node.attrib.get("ids", "")
        name = python_name_from_signature(fullname, ids, signature_node)
        if not name or private_python_symbol(name):
            continue
        kind = "class" if objtype == "class" else "method" if objtype == "method" else "function"
        docs = python_desc_content_markdown(desc)
        owner_label = name.rsplit(".", 1)[0] if kind == "method" and "." in name else "Module functions"
        member_name = name.rsplit(".", 1)[-1] if kind == "method" else name
        entries.append(
            entry(
                package=target.package,
                language="python",
                name=name,
                member_name=member_name,
                kind=kind,
                signature=python_signature(signature_node),
                docs=docs,
                source_path=target.source_path,
                owner_label=owner_label,
                source_detail=f"Sphinx XML: {xml_path.relative_to(DOCS_ROOT)}",
            )
        )
    return entries


def python_name_from_signature(fullname: str, ids: str, signature_node: ElementTree.Element) -> str:
    if fullname and fullname != "builtins":
        return fullname.removeprefix("cityjson_lib.").removeprefix("cityjson_index.")
    if ids:
        raw = ids.split()[0]
        return raw.removeprefix("cityjson_lib.").removeprefix("cityjson_index.")
    text = compact_text("".join(signature_node.itertext()))
    return text.split("(", 1)[0].strip()


def private_python_symbol(name: str) -> bool:
    return any(part.startswith("_") for part in name.split(".")) or name in {"Self", "Any", "ClassVar"}


def desc_content_text(desc: ElementTree.Element) -> str:
    for child in desc:
        if local_name(child.tag) == "desc_content":
            return compact_text(" ".join(child.itertext()))
    return ""


def extract_doxygen(target: DoxygenTarget) -> list[dict[str, object]]:
    inputs = materialize_doxygen_inputs(target)
    out_dir = GENERATED_ROOT / "doxygen" / target.name
    if out_dir.exists():
        shutil.rmtree(out_dir)
    out_dir.mkdir(parents=True)
    doxyfile = out_dir / "Doxyfile"
    doxyfile.write_text(doxyfile_text(target, inputs, out_dir), encoding="utf-8")
    run(["doxygen", str(doxyfile)], cwd=DOCS_ROOT)
    xml_dir = out_dir / "xml"
    if not (xml_dir / "index.xml").exists():
        raise RuntimeError(f"Doxygen XML missing for {target.name}")
    entries = parse_doxygen_xml(target, xml_dir)
    if not entries:
        raise RuntimeError(f"Doxygen produced no API entries for {target.name}")
    return entries


def materialize_doxygen_inputs(target: DoxygenTarget) -> list[Path]:
    existing = [path for path in target.inputs if path.exists()]
    if existing:
        return existing
    if target.cbindgen_crate is None:
        missing = ", ".join(str(path) for path in target.inputs)
        raise RuntimeError(f"missing Doxygen input for {target.name}: {missing}")
    if shutil.which("cbindgen") is None:
        raise RuntimeError(f"cbindgen is required to generate missing Doxygen input for {target.name}")
    generated = GENERATED_ROOT / "doxygen-input" / target.name / Path(target.source_path).name
    generated.parent.mkdir(parents=True, exist_ok=True)
    run(["cbindgen", str(target.cbindgen_crate), "--lang", "c", "--output", str(generated)], cwd=REPO_ROOT)
    return [generated]


def doxyfile_text(target: DoxygenTarget, inputs: list[Path], out_dir: Path) -> str:
    input_paths = " ".join(str(path) for path in inputs)
    return f"""
PROJECT_NAME = {target.name}
OUTPUT_DIRECTORY = {out_dir}
INPUT = {input_paths}
GENERATE_XML = YES
XML_OUTPUT = xml
GENERATE_HTML = NO
GENERATE_LATEX = NO
EXTRACT_ALL = YES
EXTRACT_PRIVATE = NO
QUIET = YES
WARN_IF_UNDOCUMENTED = NO
RECURSIVE = NO
ENABLE_PREPROCESSING = YES
MACRO_EXPANSION = YES
HAVE_DOT = YES
"""


def parse_doxygen_xml(target: DoxygenTarget, xml_dir: Path) -> list[dict[str, object]]:
    entries: list[dict[str, object]] = []
    for xml_path in xml_dir.glob("*.xml"):
        if xml_path.name == "index.xml":
            continue
        root = ElementTree.parse(xml_path).getroot()
        for compound in root.iter("compounddef"):
            compound_name = text_of(compound, "compoundname")
            if "detail" in compound_name:
                continue
            compound_kind = compound.attrib.get("kind")
            if target.language == "cpp" and compound_kind in {"class", "struct"} and compound_name:
                short = compound_name.split("::")[-1]
                entries.append(
                    entry(
                        package=target.package,
                        language=target.language,
                        name=compound_name,
                        member_name=short,
                        kind="class" if compound_kind == "class" else "struct",
                        signature=f"{compound_kind} {compound_name}",
                        docs=clean_docs(text_of(compound, "briefdescription")),
                        source_path=target.source_path,
                        owner_label=short,
                        source_detail=f"Doxygen XML: {xml_path.relative_to(DOCS_ROOT)}",
                    )
                )
            for member in compound.iter("memberdef"):
                kind = doxygen_kind(member.attrib.get("kind", ""), compound_kind, target.language)
                if kind is None:
                    continue
                raw_name = text_of(member, "name")
                if not raw_name or "detail" in raw_name:
                    continue
                full_name = doxygen_full_name(target.language, compound_name, raw_name, kind)
                owner_label = doxygen_owner_label(target.language, compound_name, kind)
                entries.append(
                    entry(
                        package=target.package,
                        language=target.language,
                        name=full_name,
                        member_name=raw_name,
                        kind=kind,
                        signature=doxygen_signature(member),
                        docs=clean_docs(text_of(member, "briefdescription")),
                        source_path=target.source_path,
                        owner_label=owner_label,
                        source_detail=f"Doxygen XML: {xml_path.relative_to(DOCS_ROOT)}",
                    )
                )
    return entries


def doxygen_kind(member_kind: str, compound_kind: str | None, language: str) -> str | None:
    if member_kind == "function":
        return "method" if language == "cpp" and compound_kind in {"class", "struct"} else "function"
    if member_kind == "typedef":
        return "type alias"
    if member_kind == "enum":
        return "enum"
    if member_kind == "variable":
        return "constant"
    return None


def doxygen_full_name(language: str, compound_name: str, raw_name: str, kind: str) -> str:
    if language == "cpp" and kind in {"method", "constant"} and compound_name:
        return f"{compound_name}::{raw_name}"
    return raw_name


def doxygen_owner_label(language: str, compound_name: str, kind: str) -> str:
    if language == "cpp" and kind in {"method", "constant"} and compound_name:
        return compound_name.split("::")[-1]
    if language == "c":
        return "C FFI"
    return "Functions"


def doxygen_signature(member: ElementTree.Element) -> str:
    definition = text_of(member, "definition")
    args = text_of(member, "argsstring")
    signature = compact_text(f"{definition}{args}")
    return signature.rstrip(";") + ";" if signature else ""


def entry(
    *,
    package: str,
    language: str,
    name: str,
    member_name: str,
    kind: str,
    signature: str,
    docs: str,
    source_path: str,
    owner_label: str,
    source_detail: str = "",
    docs_rs_url: str | None = None,
) -> dict[str, object]:
    owner_slug = slugify(owner_label)
    aliases = aliases_for(name, member_name, owner_label)
    normalized_signature = compact_text(signature)
    display_kind, group = classify_entry(
        language=language,
        kind=kind,
        signature=normalized_signature,
        owner_label=owner_label,
    )
    payload: dict[str, object] = {
        "package": package,
        "language": language,
        "name": name,
        "memberName": member_name,
        "kind": kind,
        "displayKind": display_kind,
        "group": group,
        "signature": normalized_signature,
        "docs": docs,
        "source": {"path": source_path, "detail": source_detail},
        "owner": {"key": f"{language}:{owner_label}", "label": owner_label, "slug": owner_slug},
        "aliases": aliases,
    }
    if docs_rs_url is not None:
        payload["docsRsUrl"] = docs_rs_url
    return payload


def classify_entry(*, language: str, kind: str, signature: str, owner_label: str) -> tuple[str, dict[str, object]]:
    if kind in {"class", "struct", "enum", "type alias"}:
        return title_case_kind(kind), group_meta("types")
    if kind == "constant":
        if is_field(language, signature):
            return "Field", group_meta("fields")
        return "Constant", group_meta("constants")
    if kind == "function":
        return "Standalone function", group_meta("standalone-functions")
    if kind != "method":
        return title_case_kind(kind), group_meta("symbols")

    if language == "python":
        if signature.startswith("classmethod "):
            return "Class method", group_meta("class-methods")
        if signature.startswith("staticmethod "):
            return "Static method", group_meta("static-methods")
        return "Instance method", group_meta("instance-methods")

    if language in {"rust", "wasm"}:
        if rust_method_has_receiver(signature):
            return "Instance method", group_meta("instance-methods")
        return "Associated function", group_meta("associated-functions")

    if language == "cpp":
        if signature.startswith("static "):
            return "Static method", group_meta("static-methods")
        return "Instance method", group_meta("instance-methods")

    if language == "c" or owner_label == "C FFI":
        return "Standalone function", group_meta("standalone-functions")

    return "Method", group_meta("instance-methods")


def title_case_kind(kind: str) -> str:
    return " ".join(part.capitalize() for part in kind.split())


def group_meta(key: str) -> dict[str, object]:
    labels = {
        "types": "Types",
        "standalone-functions": "Standalone functions",
        "associated-functions": "Associated functions",
        "class-methods": "Class methods",
        "static-methods": "Static methods",
        "instance-methods": "Instance methods",
        "constants": "Constants",
        "fields": "Fields",
        "symbols": "Symbols",
    }
    order = {
        "types": 10,
        "standalone-functions": 20,
        "associated-functions": 30,
        "class-methods": 40,
        "static-methods": 50,
        "instance-methods": 60,
        "constants": 70,
        "fields": 80,
        "symbols": 90,
    }
    return {"key": key, "label": labels[key], "order": order[key]}


def is_field(language: str, signature: str) -> bool:
    if language not in {"c", "cpp"}:
        return False
    return "::" in signature and "(" not in signature


def rust_method_has_receiver(signature: str) -> bool:
    match = re.search(r"fn\s+[^\(]+\(([^)]*)\)", signature)
    if match is None:
        return False
    params = compact_text(match.group(1))
    if not params:
        return False
    first_param = params.split(",", 1)[0].strip()
    receiver_prefixes = ("self", "&self", "&mut self", "mut self")
    if first_param.startswith(receiver_prefixes):
        return True
    return first_param.startswith("self:")


def aliases_for(name: str, member_name: str, owner_label: str) -> list[str]:
    values = [name, member_name]
    if owner_label not in {"Functions", "Module functions", "C FFI"}:
        values.append(f"{owner_label}::{member_name}")
        values.append(f"{owner_label}.{member_name}")
        if "::" in owner_label:
            values.append(f"{owner_label.split('::', 1)[1]}::{member_name}")
    return list(dict.fromkeys(value for value in values if value))


def unique_entries(entries: list[dict[str, object]]) -> list[dict[str, object]]:
    seen: set[tuple[str, str, str, str]] = set()
    unique: list[dict[str, object]] = []
    for item in entries:
        key = (str(item["package"]), str(item["language"]), str(item["kind"]), str(item["name"]))
        if key in seen:
            continue
        seen.add(key)
        unique.append(item)
    return unique


def run(args: list[str], *, cwd: Path, env: dict[str, str] | None = None) -> None:
    try:
        subprocess.run(args, cwd=cwd, env=env, check=True, text=True)
    except subprocess.CalledProcessError as error:
        command = " ".join(args)
        raise RuntimeError(f"command failed with exit code {error.returncode}: {command}") from error


def first_child(element: ElementTree.Element, tag_name: str) -> ElementTree.Element | None:
    for child in element:
        if local_name(child.tag) == tag_name:
            return child
    return None


def local_name(tag: str) -> str:
    return tag.rsplit("}", 1)[-1]


def text_of(element: ElementTree.Element, tag_name: str) -> str:
    for child in element.iter(tag_name):
        return compact_text(" ".join(child.itertext()))
    return ""


def compact_text(value: str) -> str:
    return re.sub(r"\s+", " ", value).strip()


def clean_docs(value: str) -> str:
    return compact_text(value)


def python_signature(signature_node: ElementTree.Element) -> str:
    parts: list[str] = []
    for child in signature_node:
        tag = local_name(child.tag)
        if tag == "desc_parameterlist":
            params: list[str] = []
            for parameter in child:
                if local_name(parameter.tag) != "desc_parameter":
                    continue
                text = compact_text("".join(parameter.itertext()))
                if text:
                    params.append(text)
            parts.append(f"({', '.join(params)})")
            continue
        parts.append(compact_text("".join(child.itertext())))
    return compact_text(join_signature_parts(parts))


def python_desc_content_markdown(desc: ElementTree.Element) -> str:
    for child in desc:
        if local_name(child.tag) == "desc_content":
            return render_block_children(child)
    return ""


def render_block_children(parent: ElementTree.Element) -> str:
    blocks: list[str] = []
    for child in parent:
        tag = local_name(child.tag)
        if tag in {"desc", "index"}:
            continue
        rendered = render_block(child)
        if rendered:
            blocks.append(rendered)
    return "\n\n".join(blocks).strip()


def join_signature_parts(parts: list[str]) -> str:
    result = ""
    for part in parts:
        if not part:
            continue
        if result and not result.endswith((" ", "(", "[", "{", ":")) and not part.startswith(("(", ")", "]", "}", ",", ":", ";")):
            result += " "
        result += part
    return result


def render_block(node: ElementTree.Element) -> str:
    tag = local_name(node.tag)
    if tag == "paragraph":
        return render_inline_children(node)
    if tag == "field_list":
        return render_field_list(node)
    if tag == "bullet_list":
        return render_list(node, ordered=False)
    if tag == "enumerated_list":
        return render_list(node, ordered=True)
    if tag in {"literal_block", "doctest_block"}:
        return render_literal_block(node)
    if tag == "note":
        body = render_block_children(node)
        return f"> **Note:** {body}" if body else ""
    if tag == "warning":
        body = render_block_children(node)
        return f"> **Warning:** {body}" if body else ""
    if tag == "admonition":
        title_node = first_child(node, "title")
        title = compact_text("".join(title_node.itertext())) if title_node is not None else "Note"
        body = render_block_children(node)
        return f"> **{title}:** {body}" if body else ""
    return render_inline_children(node)


def render_field_list(node: ElementTree.Element) -> str:
    fields: list[str] = []
    for child in node:
        if local_name(child.tag) != "field":
            continue
        name_node = first_child(child, "field_name")
        body_node = first_child(child, "field_body")
        if name_node is None or body_node is None:
            continue
        name = render_inline_children(name_node)
        body = render_block_children(body_node)
        if not body:
            continue
        if "\n" in body:
            fields.append(f"- **{name}:**\n{indent_block(body, 2)}")
        else:
            fields.append(f"- **{name}:** {body}")
    return "\n".join(fields)


def render_list(node: ElementTree.Element, *, ordered: bool) -> str:
    items: list[str] = []
    for index, child in enumerate(node, start=1):
        if local_name(child.tag) != "list_item":
            continue
        body = render_block_children(child)
        if not body:
            continue
        prefix = f"{index}. " if ordered else "- "
        if "\n" in body:
            first, *rest = body.splitlines()
            lines = [f"{prefix}{first}"]
            lines.extend(f"  {line}" if line else "" for line in rest)
            items.append("\n".join(lines).rstrip())
        else:
            items.append(f"{prefix}{body}")
    return "\n".join(items)


def render_literal_block(node: ElementTree.Element) -> str:
    text = "".join(node.itertext()).rstrip()
    language = node.attrib.get("language", "").strip()
    fence = language if language else ""
    return f"```{fence}\n{text}\n```"


def render_inline_children(parent: ElementTree.Element) -> str:
    pieces: list[str] = []
    if parent.text:
        pieces.append(escape_markdown_text(parent.text))
    for child in parent:
        pieces.append(render_inline_node(child))
        if child.tail:
            pieces.append(escape_markdown_text(child.tail))
    return compact_text("".join(pieces))


def render_inline_node(node: ElementTree.Element) -> str:
    tag = local_name(node.tag)
    text = render_inline_children(node) if list(node) else escape_markdown_text(node.text or "")
    if tag == "literal":
        return code_span(text)
    if tag == "strong":
        return f"**{text}**"
    if tag == "emphasis":
        return f"*{text}*"
    if tag == "inline":
        classes = node.attrib.get("classes", "")
        if "sphinx_autodoc_typehints-type" in classes:
            return text
        return text
    if tag == "reference":
        refuri = node.attrib.get("refuri")
        if refuri:
            return f"[{text}]({refuri})"
        return text
    return text


def code_span(value: str) -> str:
    escaped = value.replace("`", "\\`")
    return f"`{escaped}`"


def escape_markdown_text(value: str) -> str:
    return (
        value.replace("\\", "\\\\")
        .replace("`", "\\`")
        .replace("*", "\\*")
        .replace("_", "\\_")
        .replace("{", "\\{")
        .replace("}", "\\}")
        .replace("[", "\\[")
        .replace("]", "\\]")
    )


def indent_block(value: str, width: int) -> str:
    prefix = " " * width
    return "\n".join(prefix + line if line else line for line in value.splitlines())


def rust_docs_markdown(value: str) -> str:
    lines: list[str] = []
    in_code_fence = False
    for line in value.splitlines():
        stripped = line.lstrip()
        if stripped.startswith("```"):
            in_code_fence = not in_code_fence
            lines.append(line)
            continue
        if not in_code_fence:
            match = re.match(r"^(#{1,6})\s+(.*)$", line)
            if match:
                level = min(6, len(match.group(1)) + 3)
                line = f"{'#' * level} {match.group(2).strip()}"
        lines.append(line)
    return "\n".join(lines).strip()


def slugify(value: str) -> str:
    slug = re.sub(r"[^A-Za-z0-9_-]+", "-", value.replace("::", "-").replace(".", "-")).strip("-").lower()
    return slug or "symbols"


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"API extraction failed: {error}", file=sys.stderr)
        raise SystemExit(1) from error
