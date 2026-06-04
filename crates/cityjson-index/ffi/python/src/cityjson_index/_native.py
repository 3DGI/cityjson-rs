from __future__ import annotations

from ctypes import (
    CDLL,
    POINTER,
    Structure,
    byref,
    c_bool,
    c_char_p,
    c_int,
    c_int64,
    c_size_t,
    c_uint64,
    c_double,
    c_void_p,
    cast,
    create_string_buffer,
    string_at,
)
from enum import IntEnum
from pathlib import Path
from typing import Any
import os
import sys


class Status(IntEnum):
    SUCCESS = 0
    INVALID_ARGUMENT = 1
    IO = 2
    SYNTAX = 3
    VERSION = 4
    SHAPE = 5
    UNSUPPORTED = 6
    MODEL = 7
    INTERNAL = 8


class ErrorKind(IntEnum):
    NONE = 0
    INVALID_ARGUMENT = 1
    IO = 2
    SYNTAX = 3
    VERSION = 4
    SHAPE = 5
    UNSUPPORTED = 6
    MODEL = 7
    INTERNAL = 8


SUCCESS = Status.SUCCESS.value
INVALID_ARGUMENT = Status.INVALID_ARGUMENT.value


class CjxError(RuntimeError):
    def __init__(self, status: Status, kind: ErrorKind, message: str) -> None:
        super().__init__(message)
        self.status = status
        self.kind = kind


class _Bytes(Structure):
    _fields_ = [("data", c_void_p), ("len", c_size_t)]


class _IndexStatus(Structure):
    _fields_ = [
        ("exists", c_bool),
        ("needs_reindex", c_bool),
        ("indexed_feature_count", c_size_t),
        ("indexed_source_count", c_size_t),
    ]


class _Bounds3D(Structure):
    _fields_ = [
        ("min_x", c_double),
        ("max_x", c_double),
        ("min_y", c_double),
        ("max_y", c_double),
        ("min_z", c_double),
        ("max_z", c_double),
    ]


class _CityObjectRef(Structure):
    _fields_ = [
        ("record_id", c_int64),
        ("external_id", _Bytes),
        ("cityobject_type", _Bytes),
        ("has_bounds", c_bool),
        ("bounds", _Bounds3D),
    ]


class _PackageRef(Structure):
    _fields_ = [
        ("record_id", c_int64),
        ("model_id", _Bytes),
        ("package_type", c_int),
        ("has_bounds", c_bool),
        ("bounds", _Bounds3D),
    ]


class _StringList(Structure):
    _fields_ = [("data", POINTER(_Bytes)), ("len", c_size_t)]


class _LodSelection(Structure):
    _fields_ = [("kind", c_int), ("exact_lod", _Bytes)]


class _LodByType(Structure):
    _fields_ = [("cityobject_type", _Bytes), ("selection", _LodSelection)]


class _PackageFilter(Structure):
    _fields_ = [
        ("has_cityobject_types", c_bool),
        ("cityobject_types", _StringList),
        ("default_lod", _LodSelection),
        ("lods_by_type", POINTER(_LodByType)),
        ("lods_by_type_len", c_size_t),
    ]


class _LodMapEntry(Structure):
    _fields_ = [("cityobject_type", _Bytes), ("lods", _StringList)]


class _LodMap(Structure):
    _fields_ = [("data", POINTER(_LodMapEntry)), ("len", c_size_t)]


class _MissingLodSelection(Structure):
    _fields_ = [
        ("cityobject_type", _Bytes),
        ("requested_lod", _Bytes),
        ("available_lods", _StringList),
    ]


class _MissingLodSelectionList(Structure):
    _fields_ = [("data", POINTER(_MissingLodSelection)), ("len", c_size_t)]


class _PackageFilterReport(Structure):
    _fields_ = [
        ("available_types", _StringList),
        ("retained_types", _StringList),
        ("ignored_types", _StringList),
        ("available_lods", _LodMap),
        ("retained_lods", _LodMap),
        ("missing_lods", _MissingLodSelectionList),
        ("retained_geometry_count", c_size_t),
    ]


class _FilteredPackage(Structure):
    _fields_ = [("model_json", _Bytes), ("diagnostics", _PackageFilterReport)]


def _library_name() -> str:
    if sys.platform.startswith("win"):
        return "cityjson_index_ffi_core.dll"
    if sys.platform == "darwin":
        return "libcityjson_index_ffi_core.dylib"
    return "libcityjson_index_ffi_core.so"


def _find_repo_root(package_dir: Path) -> Path | None:
    for candidate in package_dir.parents:
        if (candidate / "Cargo.toml").exists() and (candidate / "ffi" / "python" / "pyproject.toml").exists():
            return candidate
    return None


def _candidate_paths() -> list[Path]:
    lib_name = _library_name()
    package_dir = Path(__file__).resolve().parent

    candidates: list[Path] = []
    env = os.environ.get("CITYJSON_INDEX_LIBRARY_PATH")
    if env:
        candidates.append(Path(env))

    candidates.append(package_dir / lib_name)

    repo_root = _find_repo_root(package_dir)
    if repo_root is not None:
        candidates.extend(
            [
                repo_root / "target" / "release" / lib_name,
                repo_root / "target" / "debug" / lib_name,
                repo_root / "target" / "release" / "deps" / lib_name,
                repo_root / "target" / "debug" / "deps" / lib_name,
            ]
        )

    unique_candidates: list[Path] = []
    for candidate in candidates:
        if candidate not in unique_candidates:
            unique_candidates.append(candidate)
    return unique_candidates


def _load_cdll() -> CDLL:
    for candidate in _candidate_paths():
        if candidate.exists():
            return CDLL(str(candidate))

    searched = ", ".join(str(candidate) for candidate in _candidate_paths())
    raise FileNotFoundError(f"could not locate cityjson-index shared library; searched: {searched}")


def _coerce_status(value: int) -> Status:
    return Status(value) if value in Status._value2member_map_ else Status.INTERNAL


def _coerce_error_kind(value: int) -> ErrorKind:
    return ErrorKind(value) if value in ErrorKind._value2member_map_ else ErrorKind.INTERNAL


class FfiLibrary:
    def __init__(self, library: CDLL) -> None:
        self._lib = library
        self._configure()

    @classmethod
    def load(cls) -> "FfiLibrary":
        return cls(_load_cdll())

    def _configure(self) -> None:
        self._lib.cjx_clear_error.argtypes = []
        self._lib.cjx_clear_error.restype = c_int
        self._lib.cjx_last_error_kind.argtypes = []
        self._lib.cjx_last_error_kind.restype = c_int
        self._lib.cjx_last_error_message_len.argtypes = []
        self._lib.cjx_last_error_message_len.restype = c_size_t
        self._lib.cjx_last_error_message_copy.argtypes = [c_char_p, c_size_t, POINTER(c_size_t)]
        self._lib.cjx_last_error_message_copy.restype = c_int
        self._lib.cjx_bytes_free.argtypes = [_Bytes]
        self._lib.cjx_bytes_free.restype = c_int
        self._lib.cjx_index_open.argtypes = [c_char_p, c_size_t, c_char_p, c_size_t, POINTER(c_void_p)]
        self._lib.cjx_index_open.restype = c_int
        self._lib.cjx_index_free.argtypes = [c_void_p]
        self._lib.cjx_index_free.restype = c_int
        self._lib.cjx_index_status.argtypes = [c_void_p, POINTER(_IndexStatus)]
        self._lib.cjx_index_status.restype = c_int
        self._lib.cjx_index_reindex.argtypes = [c_void_p]
        self._lib.cjx_index_reindex.restype = c_int
        self._lib.cjx_filtered_packages_free.argtypes = [POINTER(_FilteredPackage), c_size_t]
        self._lib.cjx_filtered_packages_free.restype = c_int
        self._lib.cjx_index_lookup_cityobject_refs.argtypes = [
            c_void_p,
            c_char_p,
            c_size_t,
            POINTER(POINTER(_CityObjectRef)),
            POINTER(c_size_t),
        ]
        self._lib.cjx_index_lookup_cityobject_refs.restype = c_int
        self._lib.cjx_index_package_refs_for_cityobject.argtypes = [
            c_void_p,
            POINTER(_CityObjectRef),
            POINTER(POINTER(_PackageRef)),
            POINTER(c_size_t),
        ]
        self._lib.cjx_index_package_refs_for_cityobject.restype = c_int
        self._lib.cjx_index_read_package_model_bytes.argtypes = [
            c_void_p,
            POINTER(_PackageRef),
            POINTER(_Bytes),
        ]
        self._lib.cjx_index_read_package_model_bytes.restype = c_int
        self._lib.cjx_index_read_filtered_packages.argtypes = [
            c_void_p,
            POINTER(_PackageRef),
            c_size_t,
            POINTER(_PackageFilter),
            POINTER(POINTER(_FilteredPackage)),
            POINTER(c_size_t),
        ]
        self._lib.cjx_index_read_filtered_packages.restype = c_int
        self._lib.cjx_cityobject_refs_free.argtypes = [POINTER(_CityObjectRef), c_size_t]
        self._lib.cjx_cityobject_refs_free.restype = c_int
        self._lib.cjx_package_refs_free.argtypes = [POINTER(_PackageRef), c_size_t]
        self._lib.cjx_package_refs_free.restype = c_int

    def clear_error(self) -> None:
        self._check_status(self._lib.cjx_clear_error())

    def _last_error_message(self) -> str:
        length = self._lib.cjx_last_error_message_len()
        if length == 0:
            return ""

        buffer = create_string_buffer(length + 1)
        out_len = c_size_t()
        status = self._lib.cjx_last_error_message_copy(buffer, len(buffer), byref(out_len))
        if status != Status.SUCCESS:
            return ""
        return buffer.value.decode("utf-8", errors="replace")

    def _check_status(self, status: int) -> None:
        if status == Status.SUCCESS:
            return

        status_enum = _coerce_status(status)
        kind = _coerce_error_kind(self._lib.cjx_last_error_kind())
        message = self._last_error_message()
        if not message:
            message = f"cityjson-index native call failed with status {status_enum.value}"
        raise CjxError(status=status_enum, kind=kind, message=message)

    def open_index(self, dataset_dir: str, index_path: str | None) -> c_void_p:
        handle = c_void_p()
        dataset_bytes = dataset_dir.encode("utf-8")
        if index_path is None:
            index_bytes = None
            index_len = 0
        else:
            index_bytes = index_path.encode("utf-8")
            index_len = len(index_bytes)

        self._check_status(
            self._lib.cjx_index_open(
                c_char_p(dataset_bytes),
                len(dataset_bytes),
                c_char_p(index_bytes) if index_bytes is not None else c_char_p(),
                index_len,
                byref(handle),
            )
        )
        return handle

    def close_index(self, handle: c_void_p) -> None:
        if not handle:
            return
        self._check_status(self._lib.cjx_index_free(handle))

    def index_status(self, handle: c_void_p) -> _IndexStatus:
        status = _IndexStatus()
        self._check_status(self._lib.cjx_index_status(handle, byref(status)))
        return status

    def reindex(self, handle: c_void_p) -> None:
        self._check_status(self._lib.cjx_index_reindex(handle))

    def lookup_cityobject_refs(self, handle: c_void_p, external_id: str) -> list[object]:
        refs = POINTER(_CityObjectRef)()
        count = c_size_t()
        payload = external_id.encode("utf-8")
        self._check_status(
            self._lib.cjx_index_lookup_cityobject_refs(
                handle, c_char_p(payload), len(payload), byref(refs), byref(count)
            )
        )
        try:
            from . import CityObjectRef

            return [
                CityObjectRef(
                    record_id=int(refs[index].record_id),
                    external_id=_bytes_to_py(refs[index].external_id).decode("utf-8"),
                    cityobject_type=_bytes_to_py(refs[index].cityobject_type).decode("utf-8"),
                )
                for index in range(count.value)
            ]
        finally:
            self._check_status(self._lib.cjx_cityobject_refs_free(refs, count.value))

    def package_refs_for_cityobject(self, handle: c_void_p, ref: object) -> list[object]:
        keepalive: list[Any] = []
        native = _cityobject_ref_to_native(ref, keepalive)
        refs = POINTER(_PackageRef)()
        count = c_size_t()
        self._check_status(
            self._lib.cjx_index_package_refs_for_cityobject(handle, byref(native), byref(refs), byref(count))
        )
        try:
            return _package_refs_from_native(refs, count.value)
        finally:
            self._check_status(self._lib.cjx_package_refs_free(refs, count.value))

    def read_package_model_bytes(self, handle: c_void_p, ref: object) -> bytes:
        keepalive: list[Any] = []
        native = _package_ref_to_native(ref, keepalive)
        out = _Bytes()
        self._check_status(self._lib.cjx_index_read_package_model_bytes(handle, byref(native), byref(out)))
        try:
            return _bytes_to_py(out)
        finally:
            self._check_status(self._lib.cjx_bytes_free(out))

    def read_filtered_packages(self, handle: c_void_p, refs: list[object], filter: object) -> list[object]:
        keepalive: list[Any] = []
        native_refs = _package_ref_array(refs, keepalive)
        native_filter = _package_filter_to_native(filter, keepalive)
        out = POINTER(_FilteredPackage)()
        count = c_size_t()
        self._check_status(
            self._lib.cjx_index_read_filtered_packages(
                handle, native_refs, len(refs), byref(native_filter), byref(out), byref(count)
            )
        )
        try:
            return _filtered_package_outcomes_from_native(out, count.value)
        finally:
            self._check_status(self._lib.cjx_filtered_packages_free(out, count.value))


def _cityobject_ref_to_native(ref: object, keepalive: list[Any]) -> _CityObjectRef:
    external = str(getattr(ref, "external_id", "")).encode("utf-8")
    cityobject_type = str(getattr(ref, "cityobject_type", "")).encode("utf-8")
    external_buffer = create_string_buffer(external)
    type_buffer = create_string_buffer(cityobject_type)
    native = _CityObjectRef()
    native.record_id = int(getattr(ref, "record_id"))
    native.external_id = _Bytes(cast(external_buffer, c_void_p), len(external))
    native.cityobject_type = _Bytes(cast(type_buffer, c_void_p), len(cityobject_type))
    keepalive.extend([external_buffer, type_buffer])
    return native


def _package_ref_to_native(ref: object, keepalive: list[Any]) -> _PackageRef:
    model_id = str(getattr(ref, "model_id", "")).encode("utf-8")
    model_buffer = create_string_buffer(model_id)
    native = _PackageRef()
    native.record_id = int(getattr(ref, "record_id"))
    native.model_id = _Bytes(cast(model_buffer, c_void_p), len(model_id))
    native.package_type = int(getattr(ref, "package_type", 0))
    keepalive.append(model_buffer)
    return native


def _package_ref_array(refs: list[object], keepalive: list[Any]) -> Any:
    if not refs:
        return POINTER(_PackageRef)()
    items = [_package_ref_to_native(ref, keepalive) for ref in refs]
    array_type = _PackageRef * len(items)
    array = array_type(*items)
    keepalive.append(array)
    return array


def _package_refs_from_native(refs: POINTER(_PackageRef), count: int) -> list[object]:
    if count == 0 or not refs:
        return []
    from . import PackageRef

    return [
        PackageRef(
            record_id=int(refs[index].record_id),
            model_id=_bytes_to_py(refs[index].model_id).decode("utf-8"),
            package_type=int(refs[index].package_type),
        )
        for index in range(count)
    ]


def _bytes_to_py(value: _Bytes) -> bytes:
    if value.data is None or value.len == 0:
        return b""
    return string_at(value.data, value.len)


def _bytes_from_str(value: str, keepalive: list[Any]) -> _Bytes:
    payload = value.encode("utf-8")
    buffer = create_string_buffer(payload)
    keepalive.append(buffer)
    return _Bytes(cast(buffer, c_void_p), len(payload))


def _string_list_to_native(values: object, keepalive: list[Any]) -> _StringList:
    items = [_bytes_from_str(str(value), keepalive) for value in values]
    if not items:
        return _StringList(POINTER(_Bytes)(), 0)

    array_type = _Bytes * len(items)
    array = array_type(*items)
    keepalive.append(array)
    return _StringList(array, len(items))


def _lod_selection_to_native(selection: object, keepalive: list[Any]) -> _LodSelection:
    exact_lod = getattr(selection, "exact_lod")
    if exact_lod is None:
        exact_lod_bytes = _Bytes()
    else:
        exact_lod_bytes = _bytes_from_str(exact_lod, keepalive)
    return _LodSelection(int(getattr(selection, "_native_kind")), exact_lod_bytes)


def _package_filter_to_native(filter: object, keepalive: list[Any]) -> _PackageFilter:
    cityobject_types = getattr(filter, "cityobject_types")
    if cityobject_types is None:
        native_cityobject_types = _StringList(POINTER(_Bytes)(), 0)
        has_cityobject_types = False
    else:
        native_cityobject_types = _string_list_to_native(sorted(cityobject_types), keepalive)
        has_cityobject_types = True

    lods_by_type = getattr(filter, "lods_by_type")
    items: list[_LodByType] = []
    for cityobject_type, selection in sorted(lods_by_type.items()):
        items.append(
            _LodByType(
                _bytes_from_str(cityobject_type, keepalive),
                _lod_selection_to_native(selection, keepalive),
            )
        )

    if items:
        array_type = _LodByType * len(items)
        array = array_type(*items)
        keepalive.append(array)
        lods_by_type_ptr = array
    else:
        lods_by_type_ptr = POINTER(_LodByType)()

    return _PackageFilter(
        has_cityobject_types,
        native_cityobject_types,
        _lod_selection_to_native(getattr(filter, "default_lod"), keepalive),
        lods_by_type_ptr,
        len(items),
    )


def _string_list_to_frozenset(value: _StringList) -> frozenset[str]:
    if value.len == 0 or not value.data:
        return frozenset()
    return frozenset(_bytes_to_py(value.data[index]).decode("utf-8") for index in range(value.len))


def _lod_map_to_dict(value: _LodMap) -> dict[str, frozenset[str]]:
    if value.len == 0 or not value.data:
        return {}

    result: dict[str, frozenset[str]] = {}
    for index in range(value.len):
        entry = value.data[index]
        cityobject_type = _bytes_to_py(entry.cityobject_type).decode("utf-8")
        result[cityobject_type] = _string_list_to_frozenset(entry.lods)
    return result


def _missing_lods_to_list(value: _MissingLodSelectionList) -> list[object]:
    if value.len == 0 or not value.data:
        return []

    from . import MissingLodSelection

    result: list[MissingLodSelection] = []
    for index in range(value.len):
        missing = value.data[index]
        result.append(
            MissingLodSelection(
                cityobject_type=_bytes_to_py(missing.cityobject_type).decode("utf-8"),
                requested_lod=_bytes_to_py(missing.requested_lod).decode("utf-8"),
                available_lods=_string_list_to_frozenset(missing.available_lods),
            )
        )
    return result


def _package_report_from_native(value: _PackageFilterReport) -> object:
    from . import PackageFilterReport

    return PackageFilterReport(
        available_types=_string_list_to_frozenset(value.available_types),
        retained_types=_string_list_to_frozenset(value.retained_types),
        ignored_types=_string_list_to_frozenset(value.ignored_types),
        available_lods=_lod_map_to_dict(value.available_lods),
        retained_lods=_lod_map_to_dict(value.retained_lods),
        missing_lods=_missing_lods_to_list(value.missing_lods),
        retained_geometry_count=int(value.retained_geometry_count),
    )


def _filtered_package_outcomes_from_native(packages: POINTER(_FilteredPackage), count: int) -> list[object]:
    if count == 0 or not packages:
        return []

    from . import FilteredPackageOutcome, _parse_citymodel_bytes

    result: list[FilteredPackageOutcome] = []
    for index in range(count):
        package = packages[index]
        model_payload = _bytes_to_py(package.model_json)
        result.append(
            FilteredPackageOutcome(
                model=None if not model_payload else _parse_citymodel_bytes(model_payload),
                report=_package_report_from_native(package.diagnostics),
            )
        )
    return result


_ffi = FfiLibrary.load()


def clear_error() -> None:
    _ffi.clear_error()


def open_index(dataset_dir: str, index_path: str | None) -> c_void_p:
    return _ffi.open_index(dataset_dir, index_path)


def close_index(handle: c_void_p) -> None:
    _ffi.close_index(handle)


def index_status(handle: c_void_p) -> _IndexStatus:
    return _ffi.index_status(handle)


def reindex(handle: c_void_p) -> None:
    _ffi.reindex(handle)


def lookup_cityobject_refs(handle: c_void_p, external_id: str) -> list[object]:
    return _ffi.lookup_cityobject_refs(handle, external_id)


def package_refs_for_cityobject(handle: c_void_p, ref: object) -> list[object]:
    return _ffi.package_refs_for_cityobject(handle, ref)


def read_package_model_bytes(handle: c_void_p, ref: object) -> bytes:
    return _ffi.read_package_model_bytes(handle, ref)


def read_filtered_packages(handle: c_void_p, refs: list[object], filter: object) -> list[object]:
    return _ffi.read_filtered_packages(handle, refs, filter)
