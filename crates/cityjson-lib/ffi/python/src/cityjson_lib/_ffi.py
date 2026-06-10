"""Low-level ctypes bridge for the shared cityjson_lib C ABI."""

from __future__ import annotations

from dataclasses import dataclass
from ctypes import (
    CDLL,
    POINTER,
    Structure,
    c_bool,
    c_double,
    c_float,
    c_int,
    c_int64,
    c_size_t,
    c_ubyte,
    c_uint16,
    c_uint32,
    c_void_p,
    pointer,
    string_at,
)
from enum import IntEnum
import os
from pathlib import Path


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


class RootKind(IntEnum):
    CITY_JSON = 0
    CITY_JSON_FEATURE = 1


class Version(IntEnum):
    UNKNOWN = 0
    V1_0 = 1
    V1_1 = 2
    V2_0 = 3


class ModelType(IntEnum):
    CITY_JSON = 0
    CITY_JSON_FEATURE = 1


class GeometryType(IntEnum):
    MULTI_POINT = 0
    MULTI_LINE_STRING = 1
    MULTI_SURFACE = 2
    COMPOSITE_SURFACE = 3
    SOLID = 4
    MULTI_SOLID = 5
    COMPOSITE_SOLID = 6
    GEOMETRY_INSTANCE = 7


class ContactRole(IntEnum):
    AUTHOR = 0
    CO_AUTHOR = 1
    PROCESSOR = 2
    POINT_OF_CONTACT = 3
    OWNER = 4
    USER = 5
    DISTRIBUTOR = 6
    ORIGINATOR = 7
    CUSTODIAN = 8
    RESOURCE_PROVIDER = 9
    RIGHTS_HOLDER = 10
    SPONSOR = 11
    PRINCIPAL_INVESTIGATOR = 12
    STAKEHOLDER = 13
    PUBLISHER = 14


class ContactType(IntEnum):
    INDIVIDUAL = 0
    ORGANIZATION = 1


class ImageType(IntEnum):
    PNG = 0
    JPG = 1


class WrapMode(IntEnum):
    WRAP = 0
    MIRROR = 1
    CLAMP = 2
    BORDER = 3
    NONE = 4


class TextureType(IntEnum):
    UNKNOWN = 0
    SPECIFIC = 1
    TYPICAL = 2


class CjlibError(RuntimeError):
    def __init__(self, status: Status, kind: ErrorKind, message: str) -> None:
        super().__init__(message)
        self.status = status
        self.kind = kind


class BytesStruct(Structure):
    _fields_ = [("data", POINTER(c_ubyte)), ("len", c_size_t)]


class BytesListStruct(Structure):
    _fields_ = [("data", POINTER(BytesStruct)), ("len", c_size_t)]


class VertexStruct(Structure):
    _fields_ = [("x", c_double), ("y", c_double), ("z", c_double)]


class UVStruct(Structure):
    _fields_ = [("u", c_float), ("v", c_float)]


class VerticesStruct(Structure):
    _fields_ = [("data", POINTER(VertexStruct)), ("len", c_size_t)]


class UVsStruct(Structure):
    _fields_ = [("data", POINTER(UVStruct)), ("len", c_size_t)]


class IndicesStruct(Structure):
    _fields_ = [("data", POINTER(c_size_t)), ("len", c_size_t)]


class GeometryBoundaryStruct(Structure):
    _fields_ = [
        ("geometry_type", c_int),
        ("has_boundaries", c_bool),
        ("vertex_indices", IndicesStruct),
        ("ring_offsets", IndicesStruct),
        ("surface_offsets", IndicesStruct),
        ("shell_offsets", IndicesStruct),
        ("solid_offsets", IndicesStruct),
    ]


class GeometryTypesStruct(Structure):
    _fields_ = [("data", POINTER(c_int)), ("len", c_size_t)]


class StringViewStruct(Structure):
    _fields_ = [("data", POINTER(c_ubyte)), ("len", c_size_t)]


class IndicesViewStruct(Structure):
    _fields_ = [("data", POINTER(c_size_t)), ("len", c_size_t)]


class GeometryBoundaryViewStruct(Structure):
    _fields_ = [
        ("geometry_type", c_int),
        ("vertex_indices", IndicesViewStruct),
        ("ring_offsets", IndicesViewStruct),
        ("surface_offsets", IndicesViewStruct),
        ("shell_offsets", IndicesViewStruct),
        ("solid_offsets", IndicesViewStruct),
    ]


class WriteOptionsStruct(Structure):
    _fields_ = [("pretty", c_bool), ("validate_default_themes", c_bool)]


class CityJSONSeqWriteOptionsStruct(Structure):
    _fields_ = [
        ("validate_default_themes", c_bool),
        ("trailing_newline", c_bool),
        ("update_metadata_geographical_extent", c_bool),
    ]


class CityJSONSeqAutoTransformOptionsStruct(Structure):
    _fields_ = [
        ("scale_x", c_double),
        ("scale_y", c_double),
        ("scale_z", c_double),
        ("validate_default_themes", c_bool),
        ("trailing_newline", c_bool),
        ("update_metadata_geographical_extent", c_bool),
    ]


class TransformStruct(Structure):
    _fields_ = [
        ("scale_x", c_double),
        ("scale_y", c_double),
        ("scale_z", c_double),
        ("translate_x", c_double),
        ("translate_y", c_double),
        ("translate_z", c_double),
    ]


class ProbeStruct(Structure):
    _fields_ = [("root_kind", c_int), ("version", c_int), ("has_version", c_bool)]


class ModelSummaryStruct(Structure):
    _fields_ = [
        ("model_type", c_int),
        ("version", c_int),
        ("cityobject_count", c_size_t),
        ("geometry_count", c_size_t),
        ("geometry_template_count", c_size_t),
        ("vertex_count", c_size_t),
        ("template_vertex_count", c_size_t),
        ("uv_coordinate_count", c_size_t),
        ("semantic_count", c_size_t),
        ("material_count", c_size_t),
        ("texture_count", c_size_t),
        ("extension_count", c_size_t),
        ("has_metadata", c_bool),
        ("has_transform", c_bool),
        ("has_templates", c_bool),
        ("has_appearance", c_bool),
    ]


class ModelCapacitiesStruct(Structure):
    _fields_ = [
        ("cityobjects", c_size_t),
        ("vertices", c_size_t),
        ("semantics", c_size_t),
        ("materials", c_size_t),
        ("textures", c_size_t),
        ("geometries", c_size_t),
        ("template_vertices", c_size_t),
        ("template_geometries", c_size_t),
        ("uv_coordinates", c_size_t),
    ]


class BBoxStruct(Structure):
    _fields_ = [
        ("min_x", c_double),
        ("min_y", c_double),
        ("min_z", c_double),
        ("max_x", c_double),
        ("max_y", c_double),
        ("max_z", c_double),
    ]


class RGBStruct(Structure):
    _fields_ = [("r", c_float), ("g", c_float), ("b", c_float)]


class RGBAStruct(Structure):
    _fields_ = [("r", c_float), ("g", c_float), ("b", c_float), ("a", c_float)]


class AffineTransform4x4Struct(Structure):
    _fields_ = [("elements", c_double * 16)]


class GeometryIdStruct(Structure):
    _fields_ = [("slot", c_uint32), ("generation", c_uint16), ("reserved", c_uint16)]


class SemanticIdStruct(Structure):
    _fields_ = [("slot", c_uint32), ("generation", c_uint16), ("reserved", c_uint16)]


class MaterialIdStruct(Structure):
    _fields_ = [("slot", c_uint32), ("generation", c_uint16), ("reserved", c_uint16)]


class TextureIdStruct(Structure):
    _fields_ = [("slot", c_uint32), ("generation", c_uint16), ("reserved", c_uint16)]


class CityObjectIdStruct(Structure):
    _fields_ = [("slot", c_uint32), ("generation", c_uint16), ("reserved", c_uint16)]


class GeometryTemplateIdStruct(Structure):
    _fields_ = [("slot", c_uint32), ("generation", c_uint16), ("reserved", c_uint16)]


class GeometrySelectionSpecStruct(Structure):
    _fields_ = [("cityobject_id", StringViewStruct), ("geometry_index", c_size_t)]


@dataclass(frozen=True)
class GeometryBoundaryPayload:
    geometry_type: GeometryType
    has_boundaries: bool
    vertex_indices: list[int]
    ring_offsets: list[int]
    surface_offsets: list[int]
    shell_offsets: list[int]
    solid_offsets: list[int]


@dataclass(frozen=True)
class WriteOptionsPayload:
    pretty: bool = False
    validate_default_themes: bool = False


@dataclass(frozen=True)
class CityJSONSeqWriteOptionsPayload:
    validate_default_themes: bool = True
    trailing_newline: bool = True
    update_metadata_geographical_extent: bool = True


@dataclass(frozen=True)
class CityJSONSeqAutoTransformOptionsPayload:
    scale: tuple[float, float, float] = (0.001, 0.001, 0.001)
    validate_default_themes: bool = True
    trailing_newline: bool = True
    update_metadata_geographical_extent: bool = True


def _candidate_library_paths() -> list[Path]:
    package_dir = Path(__file__).resolve().parent
    names = ["libcityjson_lib_ffi_core.so", "libcityjson_lib_ffi_core.dylib", "cityjson_lib_ffi_core.dll"]

    candidates: list[Path] = []
    if "CITYJSON_LIB_FFI_CORE_LIB" in os.environ:
        candidates.append(Path(os.environ["CITYJSON_LIB_FFI_CORE_LIB"]))

    for name in names:
        candidates.append(package_dir / name)
        if len(package_dir.parents) > 3:
            repo_root = package_dir.parents[3]
            candidates.append(repo_root / "target" / "debug" / name)
            candidates.append(repo_root / "target" / "release" / name)

    unique_candidates: list[Path] = []
    for candidate in candidates:
        if candidate not in unique_candidates:
            unique_candidates.append(candidate)

    return unique_candidates


def _load_cdll() -> CDLL:
    for candidate in _candidate_library_paths():
        if candidate.exists():
            return CDLL(str(candidate))

    searched = ", ".join(str(candidate) for candidate in _candidate_library_paths())
    raise FileNotFoundError(f"could not locate cityjson_lib ffi shared library; searched: {searched}")


class FfiLibrary:
    def __init__(self, library: CDLL) -> None:
        self._lib = library
        self._configure()

    @classmethod
    def load(cls) -> "FfiLibrary":
        return cls(_load_cdll())

    def _configure(self) -> None:
        self._lib.cj_last_error_kind.argtypes = []
        self._lib.cj_last_error_kind.restype = c_int
        self._lib.cj_last_error_message_len.argtypes = []
        self._lib.cj_last_error_message_len.restype = c_size_t
        self._lib.cj_last_error_message_copy.argtypes = [POINTER(c_ubyte), c_size_t, POINTER(c_size_t)]
        self._lib.cj_last_error_message_copy.restype = c_int
        self._lib.cj_clear_error.argtypes = []
        self._lib.cj_clear_error.restype = c_int

        self._lib.cj_probe_bytes.argtypes = [POINTER(c_ubyte), c_size_t, POINTER(ProbeStruct)]
        self._lib.cj_probe_bytes.restype = c_int

        self._lib.cj_model_parse_document_bytes.argtypes = [POINTER(c_ubyte), c_size_t, POINTER(c_void_p)]
        self._lib.cj_model_parse_document_bytes.restype = c_int
        self._lib.cj_model_parse_feature_bytes.argtypes = [POINTER(c_ubyte), c_size_t, POINTER(c_void_p)]
        self._lib.cj_model_parse_feature_bytes.restype = c_int
        self._lib.cj_model_parse_feature_with_base_bytes.argtypes = [
            POINTER(c_ubyte),
            c_size_t,
            POINTER(c_ubyte),
            c_size_t,
            POINTER(c_void_p),
        ]
        self._lib.cj_model_parse_feature_with_base_bytes.restype = c_int
        self._lib.cj_model_parse_arrow_bytes.argtypes = [
            POINTER(c_ubyte),
            c_size_t,
            POINTER(c_void_p),
        ]
        self._lib.cj_model_parse_arrow_bytes.restype = c_int
        self._lib.cj_model_parse_parquet_file.argtypes = [StringViewStruct, POINTER(c_void_p)]
        self._lib.cj_model_parse_parquet_file.restype = c_int
        self._lib.cj_model_parse_parquet_dataset_dir.argtypes = [
            StringViewStruct,
            POINTER(c_void_p),
        ]
        self._lib.cj_model_parse_parquet_dataset_dir.restype = c_int
        self._lib.cj_model_create.argtypes = [c_int, POINTER(c_void_p)]
        self._lib.cj_model_create.restype = c_int

        self._lib.cj_model_free.argtypes = [c_void_p]
        self._lib.cj_model_free.restype = c_int
        self._lib.cj_model_selection_free.argtypes = [c_void_p]
        self._lib.cj_model_selection_free.restype = c_int
        self._lib.cj_bytes_free.argtypes = [BytesStruct]
        self._lib.cj_bytes_free.restype = c_int
        self._lib.cj_bytes_list_free.argtypes = [BytesListStruct]
        self._lib.cj_bytes_list_free.restype = c_int
        self._lib.cj_vertices_free.argtypes = [VerticesStruct]
        self._lib.cj_vertices_free.restype = c_int
        self._lib.cj_uvs_free.argtypes = [UVsStruct]
        self._lib.cj_uvs_free.restype = c_int
        self._lib.cj_indices_free.argtypes = [IndicesStruct]
        self._lib.cj_indices_free.restype = c_int
        self._lib.cj_geometry_types_free.argtypes = [GeometryTypesStruct]
        self._lib.cj_geometry_types_free.restype = c_int
        self._lib.cj_geometry_boundary_free.argtypes = [GeometryBoundaryStruct]
        self._lib.cj_geometry_boundary_free.restype = c_int
        self._lib.cj_value_free.argtypes = [c_void_p]
        self._lib.cj_value_free.restype = c_int
        self._lib.cj_contact_free.argtypes = [c_void_p]
        self._lib.cj_contact_free.restype = c_int
        self._lib.cj_cityobject_draft_free.argtypes = [c_void_p]
        self._lib.cj_cityobject_draft_free.restype = c_int
        self._lib.cj_ring_draft_free.argtypes = [c_void_p]
        self._lib.cj_ring_draft_free.restype = c_int
        self._lib.cj_surface_draft_free.argtypes = [c_void_p]
        self._lib.cj_surface_draft_free.restype = c_int
        self._lib.cj_shell_draft_free.argtypes = [c_void_p]
        self._lib.cj_shell_draft_free.restype = c_int
        self._lib.cj_solid_draft_free.argtypes = [c_void_p]
        self._lib.cj_solid_draft_free.restype = c_int
        self._lib.cj_geometry_draft_free.argtypes = [c_void_p]
        self._lib.cj_geometry_draft_free.restype = c_int

        self._lib.cj_model_serialize_document.argtypes = [c_void_p, POINTER(BytesStruct)]
        self._lib.cj_model_serialize_document.restype = c_int
        self._lib.cj_model_serialize_feature.argtypes = [c_void_p, POINTER(BytesStruct)]
        self._lib.cj_model_serialize_feature.restype = c_int
        self._lib.cj_model_serialize_arrow_bytes.argtypes = [c_void_p, POINTER(BytesStruct)]
        self._lib.cj_model_serialize_arrow_bytes.restype = c_int
        self._lib.cj_model_serialize_parquet_file.argtypes = [c_void_p, StringViewStruct]
        self._lib.cj_model_serialize_parquet_file.restype = c_int
        self._lib.cj_model_serialize_parquet_dataset_dir.argtypes = [c_void_p, StringViewStruct]
        self._lib.cj_model_serialize_parquet_dataset_dir.restype = c_int
        self._lib.cj_model_serialize_document_with_options.argtypes = [
            c_void_p,
            WriteOptionsStruct,
            POINTER(BytesStruct),
        ]
        self._lib.cj_model_serialize_document_with_options.restype = c_int
        self._lib.cj_model_serialize_feature_with_options.argtypes = [
            c_void_p,
            WriteOptionsStruct,
            POINTER(BytesStruct),
        ]
        self._lib.cj_model_serialize_feature_with_options.restype = c_int

        self._lib.cj_model_get_summary.argtypes = [c_void_p, POINTER(ModelSummaryStruct)]
        self._lib.cj_model_get_summary.restype = c_int
        self._lib.cj_model_get_metadata_title.argtypes = [c_void_p, POINTER(BytesStruct)]
        self._lib.cj_model_get_metadata_title.restype = c_int
        self._lib.cj_model_get_metadata_identifier.argtypes = [c_void_p, POINTER(BytesStruct)]
        self._lib.cj_model_get_metadata_identifier.restype = c_int
        self._lib.cj_model_get_cityobject_id.argtypes = [c_void_p, c_size_t, POINTER(BytesStruct)]
        self._lib.cj_model_get_cityobject_id.restype = c_int
        self._lib.cj_model_copy_cityobject_ids.argtypes = [c_void_p, POINTER(BytesListStruct)]
        self._lib.cj_model_copy_cityobject_ids.restype = c_int
        self._lib.cj_model_get_geometry_type.argtypes = [c_void_p, c_size_t, POINTER(c_int)]
        self._lib.cj_model_get_geometry_type.restype = c_int
        self._lib.cj_model_copy_geometry_types.argtypes = [c_void_p, POINTER(GeometryTypesStruct)]
        self._lib.cj_model_copy_geometry_types.restype = c_int
        self._lib.cj_model_copy_geometry_boundary.argtypes = [
            c_void_p,
            c_size_t,
            POINTER(GeometryBoundaryStruct),
        ]
        self._lib.cj_model_copy_geometry_boundary.restype = c_int
        self._lib.cj_model_copy_geometry_boundary_coordinates.argtypes = [
            c_void_p,
            c_size_t,
            POINTER(VerticesStruct),
        ]
        self._lib.cj_model_copy_geometry_boundary_coordinates.restype = c_int
        self._lib.cj_model_copy_uv_coordinates.argtypes = [c_void_p, POINTER(UVsStruct)]
        self._lib.cj_model_copy_uv_coordinates.restype = c_int

        self._lib.cj_model_reserve_import.argtypes = [c_void_p, ModelCapacitiesStruct]
        self._lib.cj_model_reserve_import.restype = c_int
        self._lib.cj_model_add_vertex.argtypes = [c_void_p, VertexStruct, POINTER(c_size_t)]
        self._lib.cj_model_add_vertex.restype = c_int
        self._lib.cj_model_add_template_vertex.argtypes = [c_void_p, VertexStruct, POINTER(c_size_t)]
        self._lib.cj_model_add_template_vertex.restype = c_int
        self._lib.cj_model_set_vertex.argtypes = [c_void_p, c_size_t, VertexStruct]
        self._lib.cj_model_set_vertex.restype = c_int
        self._lib.cj_model_set_template_vertex.argtypes = [c_void_p, c_size_t, VertexStruct]
        self._lib.cj_model_set_template_vertex.restype = c_int
        self._lib.cj_model_add_uv_coordinate.argtypes = [c_void_p, UVStruct, POINTER(c_size_t)]
        self._lib.cj_model_add_uv_coordinate.restype = c_int

        self._lib.cj_model_set_metadata_title.argtypes = [c_void_p, StringViewStruct]
        self._lib.cj_model_set_metadata_title.restype = c_int
        self._lib.cj_model_set_metadata_identifier.argtypes = [c_void_p, StringViewStruct]
        self._lib.cj_model_set_metadata_identifier.restype = c_int
        self._lib.cj_model_set_transform.argtypes = [c_void_p, TransformStruct]
        self._lib.cj_model_set_transform.restype = c_int
        self._lib.cj_model_clear_transform.argtypes = [c_void_p]
        self._lib.cj_model_clear_transform.restype = c_int
        self._lib.cj_model_reproject.argtypes = [c_void_p, StringViewStruct]
        self._lib.cj_model_reproject.restype = c_int
        self._lib.cj_model_remove_cityobject.argtypes = [c_void_p, StringViewStruct]
        self._lib.cj_model_remove_cityobject.restype = c_int
        self._lib.cj_model_clear_cityobject_geometry.argtypes = [c_void_p, StringViewStruct]
        self._lib.cj_model_clear_cityobject_geometry.restype = c_int
        self._lib.cj_model_cleanup.argtypes = [c_void_p]
        self._lib.cj_model_cleanup.restype = c_int
        self._lib.cj_model_append_model.argtypes = [c_void_p, c_void_p]
        self._lib.cj_model_append_model.restype = c_int
        self._lib.cj_model_subset_cityobjects.argtypes = [
            c_void_p,
            POINTER(StringViewStruct),
            c_size_t,
            c_bool,
            POINTER(c_void_p),
        ]
        self._lib.cj_model_subset_cityobjects.restype = c_int
        self._lib.cj_model_select_cityobjects_by_id.argtypes = [
            c_void_p,
            POINTER(StringViewStruct),
            c_size_t,
            POINTER(c_void_p),
        ]
        self._lib.cj_model_select_cityobjects_by_id.restype = c_int
        self._lib.cj_model_select_geometries_by_cityobject_id_and_index.argtypes = [
            c_void_p,
            POINTER(GeometrySelectionSpecStruct),
            c_size_t,
            POINTER(c_void_p),
        ]
        self._lib.cj_model_select_geometries_by_cityobject_id_and_index.restype = c_int
        self._lib.cj_model_selection_include_relatives.argtypes = [
            c_void_p,
            c_void_p,
            POINTER(c_void_p),
        ]
        self._lib.cj_model_selection_include_relatives.restype = c_int
        self._lib.cj_model_selection_union.argtypes = [c_void_p, c_void_p, POINTER(c_void_p)]
        self._lib.cj_model_selection_union.restype = c_int
        self._lib.cj_model_selection_intersection.argtypes = [
            c_void_p,
            c_void_p,
            POINTER(c_void_p),
        ]
        self._lib.cj_model_selection_intersection.restype = c_int
        self._lib.cj_model_selection_is_empty.argtypes = [c_void_p, POINTER(c_bool)]
        self._lib.cj_model_selection_is_empty.restype = c_int
        self._lib.cj_model_extract_selection.argtypes = [
            c_void_p,
            c_void_p,
            POINTER(c_void_p),
        ]
        self._lib.cj_model_extract_selection.restype = c_int
        self._lib.cj_model_merge_models.argtypes = [
            POINTER(c_void_p),
            c_size_t,
            POINTER(c_void_p),
        ]
        self._lib.cj_model_merge_models.restype = c_int

        self._lib.cj_model_parse_feature_stream_merge_bytes.argtypes = [
            POINTER(c_ubyte),
            c_size_t,
            POINTER(c_void_p),
        ]
        self._lib.cj_model_parse_feature_stream_merge_bytes.restype = c_int
        self._lib.cj_model_serialize_feature_stream.argtypes = [
            POINTER(c_void_p),
            c_size_t,
            WriteOptionsStruct,
            POINTER(BytesStruct),
        ]
        self._lib.cj_model_serialize_feature_stream.restype = c_int
        self._lib.cj_model_serialize_cityjsonseq_with_transform.argtypes = [
            c_void_p,
            POINTER(c_void_p),
            c_size_t,
            TransformStruct,
            CityJSONSeqWriteOptionsStruct,
            POINTER(BytesStruct),
        ]
        self._lib.cj_model_serialize_cityjsonseq_with_transform.restype = c_int
        self._lib.cj_model_serialize_cityjsonseq_auto_transform.argtypes = [
            c_void_p,
            POINTER(c_void_p),
            c_size_t,
            CityJSONSeqAutoTransformOptionsStruct,
            POINTER(BytesStruct),
        ]
        self._lib.cj_model_serialize_cityjsonseq_auto_transform.restype = c_int

        self._lib.cj_value_new_null.argtypes = [POINTER(c_void_p)]
        self._lib.cj_value_new_null.restype = c_int
        self._lib.cj_value_new_bool.argtypes = [c_bool, POINTER(c_void_p)]
        self._lib.cj_value_new_bool.restype = c_int
        self._lib.cj_value_new_int64.argtypes = [c_int64, POINTER(c_void_p)]
        self._lib.cj_value_new_int64.restype = c_int
        self._lib.cj_value_new_float64.argtypes = [c_double, POINTER(c_void_p)]
        self._lib.cj_value_new_float64.restype = c_int
        self._lib.cj_value_new_string.argtypes = [StringViewStruct, POINTER(c_void_p)]
        self._lib.cj_value_new_string.restype = c_int
        self._lib.cj_value_new_array.argtypes = [POINTER(c_void_p)]
        self._lib.cj_value_new_array.restype = c_int
        self._lib.cj_value_new_object.argtypes = [POINTER(c_void_p)]
        self._lib.cj_value_new_object.restype = c_int
        self._lib.cj_value_new_geometry_ref.argtypes = [GeometryIdStruct, POINTER(c_void_p)]
        self._lib.cj_value_new_geometry_ref.restype = c_int
        self._lib.cj_value_array_push.argtypes = [c_void_p, c_void_p]
        self._lib.cj_value_array_push.restype = c_int
        self._lib.cj_value_object_insert.argtypes = [c_void_p, StringViewStruct, c_void_p]
        self._lib.cj_value_object_insert.restype = c_int

        self._lib.cj_contact_new.argtypes = [POINTER(c_void_p)]
        self._lib.cj_contact_new.restype = c_int
        self._lib.cj_contact_set_name.argtypes = [c_void_p, StringViewStruct]
        self._lib.cj_contact_set_name.restype = c_int
        self._lib.cj_contact_set_email.argtypes = [c_void_p, StringViewStruct]
        self._lib.cj_contact_set_email.restype = c_int
        self._lib.cj_contact_set_role.argtypes = [c_void_p, c_int]
        self._lib.cj_contact_set_role.restype = c_int
        self._lib.cj_contact_set_website.argtypes = [c_void_p, StringViewStruct]
        self._lib.cj_contact_set_website.restype = c_int
        self._lib.cj_contact_set_type.argtypes = [c_void_p, c_int]
        self._lib.cj_contact_set_type.restype = c_int
        self._lib.cj_contact_set_phone.argtypes = [c_void_p, StringViewStruct]
        self._lib.cj_contact_set_phone.restype = c_int
        self._lib.cj_contact_set_organization.argtypes = [c_void_p, StringViewStruct]
        self._lib.cj_contact_set_organization.restype = c_int
        self._lib.cj_contact_set_address.argtypes = [c_void_p, c_void_p]
        self._lib.cj_contact_set_address.restype = c_int

        self._lib.cj_model_set_metadata_geographical_extent.argtypes = [c_void_p, BBoxStruct]
        self._lib.cj_model_set_metadata_geographical_extent.restype = c_int
        self._lib.cj_model_set_metadata_reference_date.argtypes = [c_void_p, StringViewStruct]
        self._lib.cj_model_set_metadata_reference_date.restype = c_int
        self._lib.cj_model_set_metadata_reference_system.argtypes = [c_void_p, StringViewStruct]
        self._lib.cj_model_set_metadata_reference_system.restype = c_int
        self._lib.cj_model_set_metadata_contact.argtypes = [c_void_p, c_void_p]
        self._lib.cj_model_set_metadata_contact.restype = c_int
        self._lib.cj_model_set_metadata_extra.argtypes = [c_void_p, StringViewStruct, c_void_p]
        self._lib.cj_model_set_metadata_extra.restype = c_int
        self._lib.cj_model_set_root_extra.argtypes = [c_void_p, StringViewStruct, c_void_p]
        self._lib.cj_model_set_root_extra.restype = c_int
        self._lib.cj_model_add_extension.argtypes = [c_void_p, StringViewStruct, StringViewStruct, StringViewStruct]
        self._lib.cj_model_add_extension.restype = c_int

        self._lib.cj_model_add_semantic.argtypes = [c_void_p, StringViewStruct, POINTER(SemanticIdStruct)]
        self._lib.cj_model_add_semantic.restype = c_int
        self._lib.cj_model_set_semantic_parent.argtypes = [c_void_p, SemanticIdStruct, SemanticIdStruct]
        self._lib.cj_model_set_semantic_parent.restype = c_int
        self._lib.cj_model_semantic_set_extra.argtypes = [c_void_p, SemanticIdStruct, StringViewStruct, c_void_p]
        self._lib.cj_model_semantic_set_extra.restype = c_int

        self._lib.cj_model_add_material.argtypes = [c_void_p, StringViewStruct, POINTER(MaterialIdStruct)]
        self._lib.cj_model_add_material.restype = c_int
        self._lib.cj_model_material_set_ambient_intensity.argtypes = [c_void_p, MaterialIdStruct, c_float]
        self._lib.cj_model_material_set_ambient_intensity.restype = c_int
        self._lib.cj_model_material_set_diffuse_color.argtypes = [c_void_p, MaterialIdStruct, RGBStruct]
        self._lib.cj_model_material_set_diffuse_color.restype = c_int
        self._lib.cj_model_material_set_emissive_color.argtypes = [c_void_p, MaterialIdStruct, RGBStruct]
        self._lib.cj_model_material_set_emissive_color.restype = c_int
        self._lib.cj_model_material_set_specular_color.argtypes = [c_void_p, MaterialIdStruct, RGBStruct]
        self._lib.cj_model_material_set_specular_color.restype = c_int
        self._lib.cj_model_material_set_shininess.argtypes = [c_void_p, MaterialIdStruct, c_float]
        self._lib.cj_model_material_set_shininess.restype = c_int
        self._lib.cj_model_material_set_transparency.argtypes = [c_void_p, MaterialIdStruct, c_float]
        self._lib.cj_model_material_set_transparency.restype = c_int
        self._lib.cj_model_material_set_is_smooth.argtypes = [c_void_p, MaterialIdStruct, c_bool]
        self._lib.cj_model_material_set_is_smooth.restype = c_int

        self._lib.cj_model_add_texture.argtypes = [c_void_p, StringViewStruct, c_int, POINTER(TextureIdStruct)]
        self._lib.cj_model_add_texture.restype = c_int
        self._lib.cj_model_texture_set_wrap_mode.argtypes = [c_void_p, TextureIdStruct, c_int]
        self._lib.cj_model_texture_set_wrap_mode.restype = c_int
        self._lib.cj_model_texture_set_texture_type.argtypes = [c_void_p, TextureIdStruct, c_int]
        self._lib.cj_model_texture_set_texture_type.restype = c_int
        self._lib.cj_model_texture_set_border_color.argtypes = [c_void_p, TextureIdStruct, RGBAStruct]
        self._lib.cj_model_texture_set_border_color.restype = c_int
        self._lib.cj_model_set_default_material_theme.argtypes = [c_void_p, StringViewStruct]
        self._lib.cj_model_set_default_material_theme.restype = c_int
        self._lib.cj_model_set_default_texture_theme.argtypes = [c_void_p, StringViewStruct]
        self._lib.cj_model_set_default_texture_theme.restype = c_int

        self._lib.cj_cityobject_draft_new.argtypes = [StringViewStruct, StringViewStruct, POINTER(c_void_p)]
        self._lib.cj_cityobject_draft_new.restype = c_int
        self._lib.cj_cityobject_draft_set_geographical_extent.argtypes = [c_void_p, BBoxStruct]
        self._lib.cj_cityobject_draft_set_geographical_extent.restype = c_int
        self._lib.cj_cityobject_draft_set_attribute.argtypes = [c_void_p, StringViewStruct, c_void_p]
        self._lib.cj_cityobject_draft_set_attribute.restype = c_int
        self._lib.cj_cityobject_draft_set_extra.argtypes = [c_void_p, StringViewStruct, c_void_p]
        self._lib.cj_cityobject_draft_set_extra.restype = c_int
        self._lib.cj_model_add_cityobject.argtypes = [c_void_p, c_void_p, POINTER(CityObjectIdStruct)]
        self._lib.cj_model_add_cityobject.restype = c_int
        self._lib.cj_model_cityobject_add_geometry.argtypes = [c_void_p, CityObjectIdStruct, GeometryIdStruct]
        self._lib.cj_model_cityobject_add_geometry.restype = c_int
        self._lib.cj_model_cityobject_add_parent.argtypes = [c_void_p, CityObjectIdStruct, CityObjectIdStruct]
        self._lib.cj_model_cityobject_add_parent.restype = c_int

        self._lib.cj_ring_draft_new.argtypes = [POINTER(c_void_p)]
        self._lib.cj_ring_draft_new.restype = c_int
        self._lib.cj_ring_draft_push_vertex_index.argtypes = [c_void_p, c_uint32]
        self._lib.cj_ring_draft_push_vertex_index.restype = c_int
        self._lib.cj_ring_draft_push_vertex.argtypes = [c_void_p, VertexStruct]
        self._lib.cj_ring_draft_push_vertex.restype = c_int
        self._lib.cj_ring_draft_add_texture.argtypes = [
            c_void_p,
            StringViewStruct,
            TextureIdStruct,
            POINTER(c_uint32),
            c_size_t,
        ]
        self._lib.cj_ring_draft_add_texture.restype = c_int
        self._lib.cj_ring_draft_add_texture_uvs.argtypes = [
            c_void_p,
            StringViewStruct,
            TextureIdStruct,
            POINTER(UVStruct),
            c_size_t,
        ]
        self._lib.cj_ring_draft_add_texture_uvs.restype = c_int

        self._lib.cj_surface_draft_new.argtypes = [c_void_p, POINTER(c_void_p)]
        self._lib.cj_surface_draft_new.restype = c_int
        self._lib.cj_surface_draft_add_inner_ring.argtypes = [c_void_p, c_void_p]
        self._lib.cj_surface_draft_add_inner_ring.restype = c_int
        self._lib.cj_surface_draft_set_semantic.argtypes = [c_void_p, SemanticIdStruct]
        self._lib.cj_surface_draft_set_semantic.restype = c_int
        self._lib.cj_surface_draft_add_material.argtypes = [c_void_p, StringViewStruct, MaterialIdStruct]
        self._lib.cj_surface_draft_add_material.restype = c_int

        self._lib.cj_shell_draft_new.argtypes = [POINTER(c_void_p)]
        self._lib.cj_shell_draft_new.restype = c_int
        self._lib.cj_shell_draft_add_surface.argtypes = [c_void_p, c_void_p]
        self._lib.cj_shell_draft_add_surface.restype = c_int

        self._lib.cj_solid_draft_new.argtypes = [c_void_p, POINTER(c_void_p)]
        self._lib.cj_solid_draft_new.restype = c_int
        self._lib.cj_solid_draft_add_inner_shell.argtypes = [c_void_p, c_void_p]
        self._lib.cj_solid_draft_add_inner_shell.restype = c_int

        self._lib.cj_geometry_draft_new.argtypes = [c_int, StringViewStruct, POINTER(c_void_p)]
        self._lib.cj_geometry_draft_new.restype = c_int
        self._lib.cj_geometry_draft_new_instance.argtypes = [
            GeometryTemplateIdStruct,
            c_uint32,
            AffineTransform4x4Struct,
            POINTER(c_void_p),
        ]
        self._lib.cj_geometry_draft_new_instance.restype = c_int
        self._lib.cj_geometry_draft_add_point_vertex_index.argtypes = [
            c_void_p,
            c_uint32,
            POINTER(SemanticIdStruct),
        ]
        self._lib.cj_geometry_draft_add_point_vertex_index.restype = c_int
        self._lib.cj_geometry_draft_add_linestring.argtypes = [
            c_void_p,
            POINTER(c_uint32),
            c_size_t,
            POINTER(SemanticIdStruct),
        ]
        self._lib.cj_geometry_draft_add_linestring.restype = c_int
        self._lib.cj_geometry_draft_add_surface.argtypes = [c_void_p, c_void_p]
        self._lib.cj_geometry_draft_add_surface.restype = c_int
        self._lib.cj_geometry_draft_add_solid.argtypes = [c_void_p, c_void_p]
        self._lib.cj_geometry_draft_add_solid.restype = c_int
        self._lib.cj_model_add_geometry.argtypes = [c_void_p, c_void_p, POINTER(GeometryIdStruct)]
        self._lib.cj_model_add_geometry.restype = c_int
        self._lib.cj_model_add_geometry_template.argtypes = [
            c_void_p,
            c_void_p,
            POINTER(GeometryTemplateIdStruct),
        ]
        self._lib.cj_model_add_geometry_template.restype = c_int

        self._lib.cj_proj_transformer_create.argtypes = [
            StringViewStruct,
            StringViewStruct,
            POINTER(c_void_p),
        ]
        self._lib.cj_proj_transformer_create.restype = c_int
        self._lib.cj_proj_transformer_free.argtypes = [c_void_p]
        self._lib.cj_proj_transformer_free.restype = c_int
        self._lib.cj_proj_transformer_transform.argtypes = [
            c_void_p,
            VertexStruct,
            POINTER(VertexStruct),
        ]
        self._lib.cj_proj_transformer_transform.restype = c_int

    def _raise_if_error(self, raw_status: int) -> None:
        status = Status(raw_status)
        if status is Status.SUCCESS:
            return

        length = self._lib.cj_last_error_message_len()
        message = ""
        if length > 0:
            buffer = (c_ubyte * (length + 1))()
            copied = c_size_t(0)
            copy_status = Status(
                self._lib.cj_last_error_message_copy(buffer, len(buffer), pointer(copied))
            )
            if copy_status is Status.SUCCESS:
                message = bytes(buffer[: copied.value]).decode("utf-8")

        raise CjlibError(status, ErrorKind(self._lib.cj_last_error_kind()), message)

    def _data_pointer(self, data: bytes) -> POINTER(c_ubyte):
        if not data:
            return POINTER(c_ubyte)()

        array_type = c_ubyte * len(data)
        return array_type.from_buffer_copy(data)

    def _string_view(self, data: str | None) -> tuple[StringViewStruct, object]:
        if data is None:
            return StringViewStruct(), b""

        encoded = data.encode("utf-8")
        if not encoded:
            return StringViewStruct(), b""

        array_type = c_ubyte * len(encoded)
        buffer = array_type.from_buffer_copy(encoded)
        return StringViewStruct(buffer, len(encoded)), buffer

    def _indices_view(self, values: list[int]) -> tuple[IndicesViewStruct, object]:
        if not values:
            return IndicesViewStruct(), ()

        array_type = c_size_t * len(values)
        buffer = array_type(*values)
        return IndicesViewStruct(buffer, len(values)), buffer

    def _geometry_boundary_view(
        self, payload: GeometryBoundaryPayload
    ) -> tuple[GeometryBoundaryViewStruct, list[object]]:
        vertex_indices, vertex_buffer = self._indices_view(payload.vertex_indices)
        ring_offsets, ring_buffer = self._indices_view(payload.ring_offsets)
        surface_offsets, surface_buffer = self._indices_view(payload.surface_offsets)
        shell_offsets, shell_buffer = self._indices_view(payload.shell_offsets)
        solid_offsets, solid_buffer = self._indices_view(payload.solid_offsets)
        return (
            GeometryBoundaryViewStruct(
                geometry_type=int(payload.geometry_type),
                vertex_indices=vertex_indices,
                ring_offsets=ring_offsets,
                surface_offsets=surface_offsets,
                shell_offsets=shell_offsets,
                solid_offsets=solid_offsets,
            ),
            [vertex_buffer, ring_buffer, surface_buffer, shell_buffer, solid_buffer],
        )

    def _uint32_array(self, values: list[int]) -> tuple[POINTER(c_uint32), object]:
        if not values:
            return POINTER(c_uint32)(), ()

        array_type = c_uint32 * len(values)
        buffer = array_type(*values)
        return buffer, buffer

    def _uv_array(self, values: list[UVStruct]) -> tuple[POINTER(UVStruct), object]:
        if not values:
            return POINTER(UVStruct)(), ()

        array_type = UVStruct * len(values)
        buffer = array_type(*values)
        return buffer, buffer

    def _optional_semantic_pointer(
        self, semantic: SemanticIdStruct | None
    ) -> tuple[POINTER(SemanticIdStruct), object | None]:
        if semantic is None:
            return POINTER(SemanticIdStruct)(), None

        holder = semantic
        return pointer(holder), holder

    def _write_options(self, options: WriteOptionsPayload) -> WriteOptionsStruct:
        return WriteOptionsStruct(
            pretty=options.pretty,
            validate_default_themes=options.validate_default_themes,
        )

    def _cityjsonseq_write_options(
        self, options: CityJSONSeqWriteOptionsPayload
    ) -> CityJSONSeqWriteOptionsStruct:
        return CityJSONSeqWriteOptionsStruct(
            validate_default_themes=options.validate_default_themes,
            trailing_newline=options.trailing_newline,
            update_metadata_geographical_extent=options.update_metadata_geographical_extent,
        )

    def _cityjsonseq_auto_transform_options(
        self, options: CityJSONSeqAutoTransformOptionsPayload
    ) -> CityJSONSeqAutoTransformOptionsStruct:
        return CityJSONSeqAutoTransformOptionsStruct(
            scale_x=options.scale[0],
            scale_y=options.scale[1],
            scale_z=options.scale[2],
            validate_default_themes=options.validate_default_themes,
            trailing_newline=options.trailing_newline,
            update_metadata_geographical_extent=options.update_metadata_geographical_extent,
        )

    def _take_bytes(self, payload: BytesStruct) -> bytes:
        if payload.len == 0:
            self._raise_if_error(self._lib.cj_bytes_free(payload))
            return b""

        data = string_at(payload.data, payload.len)
        self._raise_if_error(self._lib.cj_bytes_free(payload))
        return data

    def _take_bytes_list(self, payload: BytesListStruct) -> list[str]:
        if payload.len == 0:
            self._raise_if_error(self._lib.cj_bytes_list_free(payload))
            return []

        values = []
        for index in range(payload.len):
            item = payload.data[index]
            if item.len == 0:
                values.append("")
                continue
            values.append(string_at(item.data, item.len).decode("utf-8"))

        self._raise_if_error(self._lib.cj_bytes_list_free(payload))
        return values

    def _take_geometry_types(self, payload: GeometryTypesStruct) -> list[GeometryType]:
        if payload.len == 0:
            self._raise_if_error(self._lib.cj_geometry_types_free(payload))
            return []

        values = [GeometryType(payload.data[index]) for index in range(payload.len)]
        self._raise_if_error(self._lib.cj_geometry_types_free(payload))
        return values

    def _take_vertices(self, payload: VerticesStruct) -> list[VertexStruct]:
        if payload.len == 0:
            self._raise_if_error(self._lib.cj_vertices_free(payload))
            return []

        values = [
            VertexStruct(
                x=payload.data[index].x,
                y=payload.data[index].y,
                z=payload.data[index].z,
            )
            for index in range(payload.len)
        ]
        self._raise_if_error(self._lib.cj_vertices_free(payload))
        return values

    def _take_uvs(self, payload: UVsStruct) -> list[UVStruct]:
        if payload.len == 0:
            self._raise_if_error(self._lib.cj_uvs_free(payload))
            return []

        values = [
            UVStruct(u=payload.data[index].u, v=payload.data[index].v)
            for index in range(payload.len)
        ]
        self._raise_if_error(self._lib.cj_uvs_free(payload))
        return values

    def _copy_indices(self, payload: IndicesStruct) -> list[int]:
        return [payload.data[index] for index in range(payload.len)]

    def _take_geometry_boundary(self, payload: GeometryBoundaryStruct) -> GeometryBoundaryPayload:
        boundary = GeometryBoundaryPayload(
            geometry_type=GeometryType(payload.geometry_type),
            has_boundaries=bool(payload.has_boundaries),
            vertex_indices=self._copy_indices(payload.vertex_indices),
            ring_offsets=self._copy_indices(payload.ring_offsets),
            surface_offsets=self._copy_indices(payload.surface_offsets),
            shell_offsets=self._copy_indices(payload.shell_offsets),
            solid_offsets=self._copy_indices(payload.solid_offsets),
        )
        self._raise_if_error(self._lib.cj_geometry_boundary_free(payload))
        return boundary

    def _string_handles(self, values: list[str]) -> tuple[object, list[object]]:
        buffers: list[object] = []
        views: list[StringViewStruct] = []
        for value in values:
            view, buffer = self._string_view(value)
            views.append(view)
            buffers.append(buffer)

        array_type = StringViewStruct * len(views)
        return array_type(*views), buffers

    def _geometry_selection_specs(
        self, specs: list[tuple[str, int]]
    ) -> tuple[object, list[object]]:
        buffers: list[object] = []
        values: list[GeometrySelectionSpecStruct] = []
        for cityobject_id, geometry_index in specs:
            if geometry_index < 0:
                raise ValueError("geometry_index must not be negative")
            view, buffer = self._string_view(cityobject_id)
            values.append(
                GeometrySelectionSpecStruct(
                    cityobject_id=view,
                    geometry_index=geometry_index,
                )
            )
            buffers.append(buffer)

        array_type = GeometrySelectionSpecStruct * len(values)
        return array_type(*values), buffers

    def probe(self, data: bytes) -> ProbeStruct:
        probe = ProbeStruct()
        pointer_data = self._data_pointer(data)
        self._raise_if_error(self._lib.cj_probe_bytes(pointer_data, len(data), pointer(probe)))
        return probe

    def parse_document(self, data: bytes) -> int:
        handle = c_void_p()
        pointer_data = self._data_pointer(data)
        self._raise_if_error(
            self._lib.cj_model_parse_document_bytes(pointer_data, len(data), pointer(handle))
        )
        return int(handle.value)

    def parse_feature(self, data: bytes) -> int:
        handle = c_void_p()
        pointer_data = self._data_pointer(data)
        self._raise_if_error(
            self._lib.cj_model_parse_feature_bytes(pointer_data, len(data), pointer(handle))
        )
        return int(handle.value)

    def parse_feature_with_base(self, feature_data: bytes, base_data: bytes) -> int:
        handle = c_void_p()
        feature_pointer = self._data_pointer(feature_data)
        base_pointer = self._data_pointer(base_data)
        self._raise_if_error(
            self._lib.cj_model_parse_feature_with_base_bytes(
                feature_pointer,
                len(feature_data),
                base_pointer,
                len(base_data),
                pointer(handle),
            )
        )
        return int(handle.value)

    def parse_arrow(self, data: bytes) -> int:
        handle = c_void_p()
        pointer_data = self._data_pointer(data)
        self._raise_if_error(
            self._lib.cj_model_parse_arrow_bytes(pointer_data, len(data), pointer(handle))
        )
        return int(handle.value)

    def parse_parquet_file(self, path: str) -> int:
        handle = c_void_p()
        path_view, _path_buffer = self._string_view(path)
        self._raise_if_error(self._lib.cj_model_parse_parquet_file(path_view, pointer(handle)))
        return int(handle.value)

    def parse_parquet_dataset_dir(self, path: str) -> int:
        handle = c_void_p()
        path_view, _path_buffer = self._string_view(path)
        self._raise_if_error(
            self._lib.cj_model_parse_parquet_dataset_dir(path_view, pointer(handle))
        )
        return int(handle.value)

    def create(self, model_type: ModelType) -> int:
        handle = c_void_p()
        self._raise_if_error(self._lib.cj_model_create(int(model_type), pointer(handle)))
        return int(handle.value)

    def free_model(self, handle: int) -> None:
        self._raise_if_error(self._lib.cj_model_free(c_void_p(handle)))

    def free_model_selection(self, handle: int) -> None:
        self._raise_if_error(self._lib.cj_model_selection_free(c_void_p(handle)))

    def free_value(self, handle: int) -> None:
        self._raise_if_error(self._lib.cj_value_free(c_void_p(handle)))

    def free_contact(self, handle: int) -> None:
        self._raise_if_error(self._lib.cj_contact_free(c_void_p(handle)))

    def free_cityobject_draft(self, handle: int) -> None:
        self._raise_if_error(self._lib.cj_cityobject_draft_free(c_void_p(handle)))

    def free_ring_draft(self, handle: int) -> None:
        self._raise_if_error(self._lib.cj_ring_draft_free(c_void_p(handle)))

    def free_surface_draft(self, handle: int) -> None:
        self._raise_if_error(self._lib.cj_surface_draft_free(c_void_p(handle)))

    def free_shell_draft(self, handle: int) -> None:
        self._raise_if_error(self._lib.cj_shell_draft_free(c_void_p(handle)))

    def free_solid_draft(self, handle: int) -> None:
        self._raise_if_error(self._lib.cj_solid_draft_free(c_void_p(handle)))

    def free_geometry_draft(self, handle: int) -> None:
        self._raise_if_error(self._lib.cj_geometry_draft_free(c_void_p(handle)))

    def serialize_document(self, handle: int) -> bytes:
        payload = BytesStruct()
        self._raise_if_error(self._lib.cj_model_serialize_document(c_void_p(handle), pointer(payload)))
        return self._take_bytes(payload)

    def serialize_feature(self, handle: int) -> bytes:
        payload = BytesStruct()
        self._raise_if_error(self._lib.cj_model_serialize_feature(c_void_p(handle), pointer(payload)))
        return self._take_bytes(payload)

    def serialize_arrow(self, handle: int) -> bytes:
        payload = BytesStruct()
        self._raise_if_error(
            self._lib.cj_model_serialize_arrow_bytes(c_void_p(handle), pointer(payload))
        )
        return self._take_bytes(payload)

    def serialize_parquet_file(self, handle: int, path: str) -> None:
        path_view, _path_buffer = self._string_view(path)
        self._raise_if_error(
            self._lib.cj_model_serialize_parquet_file(c_void_p(handle), path_view)
        )

    def serialize_parquet_dataset_dir(self, handle: int, path: str) -> None:
        path_view, _path_buffer = self._string_view(path)
        self._raise_if_error(
            self._lib.cj_model_serialize_parquet_dataset_dir(c_void_p(handle), path_view)
        )

    def summary(self, handle: int) -> ModelSummaryStruct:
        summary = ModelSummaryStruct()
        self._raise_if_error(self._lib.cj_model_get_summary(c_void_p(handle), pointer(summary)))
        return summary

    def metadata_title(self, handle: int) -> str:
        payload = BytesStruct()
        self._raise_if_error(self._lib.cj_model_get_metadata_title(c_void_p(handle), pointer(payload)))
        return self._take_bytes(payload).decode("utf-8")

    def metadata_identifier(self, handle: int) -> str:
        payload = BytesStruct()
        self._raise_if_error(
            self._lib.cj_model_get_metadata_identifier(c_void_p(handle), pointer(payload))
        )
        return self._take_bytes(payload).decode("utf-8")

    def cityobject_id(self, handle: int, index: int) -> str:
        payload = BytesStruct()
        self._raise_if_error(
            self._lib.cj_model_get_cityobject_id(c_void_p(handle), index, pointer(payload))
        )
        return self._take_bytes(payload).decode("utf-8")

    def cityobject_ids(self, handle: int) -> list[str]:
        payload = BytesListStruct()
        self._raise_if_error(self._lib.cj_model_copy_cityobject_ids(c_void_p(handle), pointer(payload)))
        return self._take_bytes_list(payload)

    def clear_cityobject_geometry(self, handle: int, cityobject_id: str) -> None:
        cityobject_id_view, _ = self._string_view(cityobject_id)
        self._raise_if_error(
            self._lib.cj_model_clear_cityobject_geometry(
                c_void_p(handle), cityobject_id_view
            )
        )

    def geometry_type(self, handle: int, index: int) -> GeometryType:
        geometry_type = c_int()
        self._raise_if_error(
            self._lib.cj_model_get_geometry_type(c_void_p(handle), index, pointer(geometry_type))
        )
        return GeometryType(geometry_type.value)

    def geometry_types(self, handle: int) -> list[GeometryType]:
        payload = GeometryTypesStruct()
        self._raise_if_error(
            self._lib.cj_model_copy_geometry_types(c_void_p(handle), pointer(payload))
        )
        return self._take_geometry_types(payload)

    def geometry_boundary(self, handle: int, index: int) -> GeometryBoundaryPayload:
        payload = GeometryBoundaryStruct()
        self._raise_if_error(
            self._lib.cj_model_copy_geometry_boundary(c_void_p(handle), index, pointer(payload))
        )
        return self._take_geometry_boundary(payload)

    def geometry_boundary_coordinates(self, handle: int, index: int) -> list[VertexStruct]:
        payload = VerticesStruct()
        self._raise_if_error(
            self._lib.cj_model_copy_geometry_boundary_coordinates(
                c_void_p(handle), index, pointer(payload)
            )
        )
        return self._take_vertices(payload)

    def uv_coordinates(self, handle: int) -> list[UVStruct]:
        payload = UVsStruct()
        self._raise_if_error(
            self._lib.cj_model_copy_uv_coordinates(c_void_p(handle), pointer(payload))
        )
        return self._take_uvs(payload)

    def reserve_import(self, handle: int, capacities: ModelCapacitiesStruct) -> None:
        self._raise_if_error(self._lib.cj_model_reserve_import(c_void_p(handle), capacities))

    def add_vertex(self, handle: int, x: float, y: float, z: float) -> int:
        index = c_size_t(0)
        self._raise_if_error(
            self._lib.cj_model_add_vertex(
                c_void_p(handle), VertexStruct(x=x, y=y, z=z), pointer(index)
            )
        )
        return int(index.value)

    def add_template_vertex(self, handle: int, x: float, y: float, z: float) -> int:
        index = c_size_t(0)
        self._raise_if_error(
            self._lib.cj_model_add_template_vertex(
                c_void_p(handle), VertexStruct(x=x, y=y, z=z), pointer(index)
            )
        )
        return int(index.value)

    def set_vertex(self, handle: int, index: int, x: float, y: float, z: float) -> None:
        self._raise_if_error(
            self._lib.cj_model_set_vertex(
                c_void_p(handle), index, VertexStruct(x=x, y=y, z=z)
            )
        )

    def set_template_vertex(self, handle: int, index: int, x: float, y: float, z: float) -> None:
        self._raise_if_error(
            self._lib.cj_model_set_template_vertex(
                c_void_p(handle), index, VertexStruct(x=x, y=y, z=z)
            )
        )

    def add_uv_coordinate(self, handle: int, u: float, v: float) -> int:
        index = c_size_t(0)
        self._raise_if_error(
            self._lib.cj_model_add_uv_coordinate(c_void_p(handle), UVStruct(u=u, v=v), pointer(index))
        )
        return int(index.value)

    def geometry_boundary_view(
        self, payload: GeometryBoundaryPayload
    ) -> tuple[GeometryBoundaryViewStruct, list[object]]:
        return self._geometry_boundary_view(payload)

    def write_options(self, payload: WriteOptionsPayload) -> WriteOptionsStruct:
        return self._write_options(payload)

    def cityjsonseq_write_options(
        self, payload: CityJSONSeqWriteOptionsPayload
    ) -> CityJSONSeqWriteOptionsStruct:
        return self._cityjsonseq_write_options(payload)

    def cityjsonseq_auto_transform_options(
        self, payload: CityJSONSeqAutoTransformOptionsPayload
    ) -> CityJSONSeqAutoTransformOptionsStruct:
        return self._cityjsonseq_auto_transform_options(payload)

    def set_metadata_title(self, handle: int, title: str) -> None:
        view, _buffer = self._string_view(title)
        self._raise_if_error(self._lib.cj_model_set_metadata_title(c_void_p(handle), view))

    def set_metadata_identifier(self, handle: int, identifier: str) -> None:
        view, _buffer = self._string_view(identifier)
        self._raise_if_error(self._lib.cj_model_set_metadata_identifier(c_void_p(handle), view))

    def set_metadata_geographical_extent(self, handle: int, bbox: BBoxStruct) -> None:
        self._raise_if_error(
            self._lib.cj_model_set_metadata_geographical_extent(c_void_p(handle), bbox)
        )

    def set_metadata_reference_date(self, handle: int, value: str) -> None:
        view, _buffer = self._string_view(value)
        self._raise_if_error(self._lib.cj_model_set_metadata_reference_date(c_void_p(handle), view))

    def set_metadata_reference_system(self, handle: int, value: str) -> None:
        view, _buffer = self._string_view(value)
        self._raise_if_error(
            self._lib.cj_model_set_metadata_reference_system(c_void_p(handle), view)
        )

    def set_transform(self, handle: int, transform: TransformStruct) -> None:
        self._raise_if_error(self._lib.cj_model_set_transform(c_void_p(handle), transform))

    def clear_transform(self, handle: int) -> None:
        self._raise_if_error(self._lib.cj_model_clear_transform(c_void_p(handle)))

    def reproject(self, handle: int, target_crs: str) -> None:
        view, _buffer = self._string_view(target_crs)
        self._raise_if_error(self._lib.cj_model_reproject(c_void_p(handle), view))

    def remove_cityobject(self, handle: int, cityobject_id: str) -> None:
        view, _buffer = self._string_view(cityobject_id)
        self._raise_if_error(self._lib.cj_model_remove_cityobject(c_void_p(handle), view))

    def cleanup(self, handle: int) -> None:
        self._raise_if_error(self._lib.cj_model_cleanup(c_void_p(handle)))

    def append_model(self, target_handle: int, source_handle: int) -> None:
        self._raise_if_error(
            self._lib.cj_model_append_model(c_void_p(target_handle), c_void_p(source_handle))
        )

    def subset_cityobjects(self, handle: int, cityobject_ids: list[str], exclude: bool = False) -> int:
        if not cityobject_ids:
            raise ValueError("cityobject_ids must not be empty")

        array, _buffers = self._string_handles(cityobject_ids)
        extracted = c_void_p()
        self._raise_if_error(
            self._lib.cj_model_subset_cityobjects(
                c_void_p(handle), array, len(cityobject_ids), exclude, pointer(extracted)
            )
        )
        return int(extracted.value)

    def select_cityobjects_by_id(self, handle: int, cityobject_ids: list[str]) -> int:
        array, _buffers = self._string_handles(cityobject_ids)
        selection = c_void_p()
        self._raise_if_error(
            self._lib.cj_model_select_cityobjects_by_id(
                c_void_p(handle), array, len(cityobject_ids), pointer(selection)
            )
        )
        return int(selection.value)

    def select_geometries_by_cityobject_id_and_index(
        self, handle: int, specs: list[tuple[str, int]]
    ) -> int:
        array, _buffers = self._geometry_selection_specs(specs)
        selection = c_void_p()
        self._raise_if_error(
            self._lib.cj_model_select_geometries_by_cityobject_id_and_index(
                c_void_p(handle), array, len(specs), pointer(selection)
            )
        )
        return int(selection.value)

    def model_selection_include_relatives(self, selection_handle: int, model_handle: int) -> int:
        included = c_void_p()
        self._raise_if_error(
            self._lib.cj_model_selection_include_relatives(
                c_void_p(selection_handle), c_void_p(model_handle), pointer(included)
            )
        )
        return int(included.value)

    def model_selection_union(self, lhs_handle: int, rhs_handle: int) -> int:
        selection = c_void_p()
        self._raise_if_error(
            self._lib.cj_model_selection_union(
                c_void_p(lhs_handle), c_void_p(rhs_handle), pointer(selection)
            )
        )
        return int(selection.value)

    def model_selection_intersection(self, lhs_handle: int, rhs_handle: int) -> int:
        selection = c_void_p()
        self._raise_if_error(
            self._lib.cj_model_selection_intersection(
                c_void_p(lhs_handle), c_void_p(rhs_handle), pointer(selection)
            )
        )
        return int(selection.value)

    def model_selection_is_empty(self, selection_handle: int) -> bool:
        value = c_bool()
        self._raise_if_error(
            self._lib.cj_model_selection_is_empty(c_void_p(selection_handle), pointer(value))
        )
        return bool(value.value)

    def extract_selection(self, model_handle: int, selection_handle: int) -> int:
        extracted = c_void_p()
        self._raise_if_error(
            self._lib.cj_model_extract_selection(
                c_void_p(model_handle), c_void_p(selection_handle), pointer(extracted)
            )
        )
        return int(extracted.value)

    def merge_models(self, handles: list[int]) -> int:
        array = self._handle_array(handles)
        merged = c_void_p()
        self._raise_if_error(
            self._lib.cj_model_merge_models(array, len(handles), pointer(merged))
        )
        return int(merged.value)

    def serialize_document_with_options(self, handle: int, options: WriteOptionsStruct) -> bytes:
        payload = BytesStruct()
        self._raise_if_error(
            self._lib.cj_model_serialize_document_with_options(
                c_void_p(handle), options, pointer(payload)
            )
        )
        return self._take_bytes(payload)

    def serialize_feature_with_options(self, handle: int, options: WriteOptionsStruct) -> bytes:
        payload = BytesStruct()
        self._raise_if_error(
            self._lib.cj_model_serialize_feature_with_options(
                c_void_p(handle), options, pointer(payload)
            )
        )
        return self._take_bytes(payload)

    def parse_feature_stream_merge(self, data: bytes) -> int:
        handle = c_void_p()
        pointer_data = self._data_pointer(data)
        self._raise_if_error(
            self._lib.cj_model_parse_feature_stream_merge_bytes(
                pointer_data, len(data), pointer(handle)
            )
        )
        return int(handle.value)

    def _handle_array(self, handles: list[int]) -> object:
        array_type = c_void_p * len(handles)
        return array_type(*[c_void_p(handle) for handle in handles])

    def serialize_feature_stream(self, handles: list[int], options: WriteOptionsStruct) -> bytes:
        payload = BytesStruct()
        if not handles:
            self._raise_if_error(
                self._lib.cj_model_serialize_feature_stream(
                    POINTER(c_void_p)(), 0, options, pointer(payload)
                )
            )
            return self._take_bytes(payload)

        array = self._handle_array(handles)
        self._raise_if_error(
            self._lib.cj_model_serialize_feature_stream(array, len(handles), options, pointer(payload))
        )
        return self._take_bytes(payload)

    def serialize_cityjsonseq_with_transform(
        self,
        base_root_handle: int,
        feature_handles: list[int],
        transform: TransformStruct,
        options: CityJSONSeqWriteOptionsStruct,
    ) -> bytes:
        payload = BytesStruct()
        array = self._handle_array(feature_handles)
        self._raise_if_error(
            self._lib.cj_model_serialize_cityjsonseq_with_transform(
                c_void_p(base_root_handle),
                array,
                len(feature_handles),
                transform,
                options,
                pointer(payload),
            )
        )
        return self._take_bytes(payload)

    def serialize_cityjsonseq_auto_transform(
        self,
        base_root_handle: int,
        feature_handles: list[int],
        options: CityJSONSeqAutoTransformOptionsStruct,
    ) -> bytes:
        payload = BytesStruct()
        array = self._handle_array(feature_handles)
        self._raise_if_error(
            self._lib.cj_model_serialize_cityjsonseq_auto_transform(
                c_void_p(base_root_handle),
                array,
                len(feature_handles),
                options,
                pointer(payload),
            )
        )
        return self._take_bytes(payload)

    def value_new_null(self) -> int:
        handle = c_void_p()
        self._raise_if_error(self._lib.cj_value_new_null(pointer(handle)))
        return int(handle.value)

    def value_new_bool(self, value: bool) -> int:
        handle = c_void_p()
        self._raise_if_error(self._lib.cj_value_new_bool(value, pointer(handle)))
        return int(handle.value)

    def value_new_int64(self, value: int) -> int:
        handle = c_void_p()
        self._raise_if_error(self._lib.cj_value_new_int64(value, pointer(handle)))
        return int(handle.value)

    def value_new_float64(self, value: float) -> int:
        handle = c_void_p()
        self._raise_if_error(self._lib.cj_value_new_float64(value, pointer(handle)))
        return int(handle.value)

    def value_new_string(self, value: str) -> int:
        handle = c_void_p()
        view, _buffer = self._string_view(value)
        self._raise_if_error(self._lib.cj_value_new_string(view, pointer(handle)))
        return int(handle.value)

    def value_new_array(self) -> int:
        handle = c_void_p()
        self._raise_if_error(self._lib.cj_value_new_array(pointer(handle)))
        return int(handle.value)

    def value_new_object(self) -> int:
        handle = c_void_p()
        self._raise_if_error(self._lib.cj_value_new_object(pointer(handle)))
        return int(handle.value)

    def value_new_geometry_ref(self, geometry: GeometryIdStruct) -> int:
        handle = c_void_p()
        self._raise_if_error(self._lib.cj_value_new_geometry_ref(geometry, pointer(handle)))
        return int(handle.value)

    def value_array_push(self, array_handle: int, element_handle: int) -> None:
        self._raise_if_error(
            self._lib.cj_value_array_push(c_void_p(array_handle), c_void_p(element_handle))
        )

    def value_object_insert(self, object_handle: int, key: str, member_handle: int) -> None:
        view, _buffer = self._string_view(key)
        self._raise_if_error(
            self._lib.cj_value_object_insert(c_void_p(object_handle), view, c_void_p(member_handle))
        )

    def contact_new(self) -> int:
        handle = c_void_p()
        self._raise_if_error(self._lib.cj_contact_new(pointer(handle)))
        return int(handle.value)

    def contact_set_name(self, handle: int, value: str) -> None:
        view, _buffer = self._string_view(value)
        self._raise_if_error(self._lib.cj_contact_set_name(c_void_p(handle), view))

    def contact_set_email(self, handle: int, value: str) -> None:
        view, _buffer = self._string_view(value)
        self._raise_if_error(self._lib.cj_contact_set_email(c_void_p(handle), view))

    def contact_set_role(self, handle: int, value: ContactRole) -> None:
        self._raise_if_error(self._lib.cj_contact_set_role(c_void_p(handle), int(value)))

    def contact_set_website(self, handle: int, value: str) -> None:
        view, _buffer = self._string_view(value)
        self._raise_if_error(self._lib.cj_contact_set_website(c_void_p(handle), view))

    def contact_set_type(self, handle: int, value: ContactType) -> None:
        self._raise_if_error(self._lib.cj_contact_set_type(c_void_p(handle), int(value)))

    def contact_set_phone(self, handle: int, value: str) -> None:
        view, _buffer = self._string_view(value)
        self._raise_if_error(self._lib.cj_contact_set_phone(c_void_p(handle), view))

    def contact_set_organization(self, handle: int, value: str) -> None:
        view, _buffer = self._string_view(value)
        self._raise_if_error(self._lib.cj_contact_set_organization(c_void_p(handle), view))

    def contact_set_address(self, handle: int, object_handle: int) -> None:
        self._raise_if_error(
            self._lib.cj_contact_set_address(c_void_p(handle), c_void_p(object_handle))
        )

    def model_set_metadata_contact(self, model_handle: int, contact_handle: int) -> None:
        self._raise_if_error(
            self._lib.cj_model_set_metadata_contact(c_void_p(model_handle), c_void_p(contact_handle))
        )

    def model_set_metadata_extra(self, model_handle: int, key: str, value_handle: int) -> None:
        view, _buffer = self._string_view(key)
        self._raise_if_error(
            self._lib.cj_model_set_metadata_extra(c_void_p(model_handle), view, c_void_p(value_handle))
        )

    def model_set_root_extra(self, model_handle: int, key: str, value_handle: int) -> None:
        view, _buffer = self._string_view(key)
        self._raise_if_error(
            self._lib.cj_model_set_root_extra(c_void_p(model_handle), view, c_void_p(value_handle))
        )

    def model_add_extension(self, model_handle: int, name: str, url: str, version: str) -> None:
        name_view, _name = self._string_view(name)
        url_view, _url = self._string_view(url)
        version_view, _version = self._string_view(version)
        self._raise_if_error(
            self._lib.cj_model_add_extension(c_void_p(model_handle), name_view, url_view, version_view)
        )

    def model_add_semantic(self, model_handle: int, semantic_type: str) -> SemanticIdStruct:
        semantic = SemanticIdStruct()
        view, _buffer = self._string_view(semantic_type)
        self._raise_if_error(
            self._lib.cj_model_add_semantic(c_void_p(model_handle), view, pointer(semantic))
        )
        return semantic

    def model_set_semantic_parent(
        self, model_handle: int, semantic: SemanticIdStruct, parent: SemanticIdStruct
    ) -> None:
        self._raise_if_error(
            self._lib.cj_model_set_semantic_parent(c_void_p(model_handle), semantic, parent)
        )

    def model_semantic_set_extra(
        self, model_handle: int, semantic: SemanticIdStruct, key: str, value_handle: int
    ) -> None:
        view, _buffer = self._string_view(key)
        self._raise_if_error(
            self._lib.cj_model_semantic_set_extra(
                c_void_p(model_handle), semantic, view, c_void_p(value_handle)
            )
        )

    def model_add_material(self, model_handle: int, name: str) -> MaterialIdStruct:
        material = MaterialIdStruct()
        view, _buffer = self._string_view(name)
        self._raise_if_error(
            self._lib.cj_model_add_material(c_void_p(model_handle), view, pointer(material))
        )
        return material

    def model_material_set_ambient_intensity(
        self, model_handle: int, material: MaterialIdStruct, value: float
    ) -> None:
        self._raise_if_error(
            self._lib.cj_model_material_set_ambient_intensity(c_void_p(model_handle), material, value)
        )

    def model_material_set_diffuse_color(
        self, model_handle: int, material: MaterialIdStruct, value: RGBStruct
    ) -> None:
        self._raise_if_error(
            self._lib.cj_model_material_set_diffuse_color(c_void_p(model_handle), material, value)
        )

    def model_material_set_emissive_color(
        self, model_handle: int, material: MaterialIdStruct, value: RGBStruct
    ) -> None:
        self._raise_if_error(
            self._lib.cj_model_material_set_emissive_color(c_void_p(model_handle), material, value)
        )

    def model_material_set_specular_color(
        self, model_handle: int, material: MaterialIdStruct, value: RGBStruct
    ) -> None:
        self._raise_if_error(
            self._lib.cj_model_material_set_specular_color(c_void_p(model_handle), material, value)
        )

    def model_material_set_shininess(
        self, model_handle: int, material: MaterialIdStruct, value: float
    ) -> None:
        self._raise_if_error(
            self._lib.cj_model_material_set_shininess(c_void_p(model_handle), material, value)
        )

    def model_material_set_transparency(
        self, model_handle: int, material: MaterialIdStruct, value: float
    ) -> None:
        self._raise_if_error(
            self._lib.cj_model_material_set_transparency(c_void_p(model_handle), material, value)
        )

    def model_material_set_is_smooth(
        self, model_handle: int, material: MaterialIdStruct, value: bool
    ) -> None:
        self._raise_if_error(
            self._lib.cj_model_material_set_is_smooth(c_void_p(model_handle), material, value)
        )

    def model_add_texture(
        self, model_handle: int, image: str, image_type: ImageType
    ) -> TextureIdStruct:
        texture = TextureIdStruct()
        view, _buffer = self._string_view(image)
        self._raise_if_error(
            self._lib.cj_model_add_texture(
                c_void_p(model_handle), view, int(image_type), pointer(texture)
            )
        )
        return texture

    def model_texture_set_wrap_mode(
        self, model_handle: int, texture: TextureIdStruct, value: WrapMode
    ) -> None:
        self._raise_if_error(
            self._lib.cj_model_texture_set_wrap_mode(c_void_p(model_handle), texture, int(value))
        )

    def model_texture_set_texture_type(
        self, model_handle: int, texture: TextureIdStruct, value: TextureType
    ) -> None:
        self._raise_if_error(
            self._lib.cj_model_texture_set_texture_type(c_void_p(model_handle), texture, int(value))
        )

    def model_texture_set_border_color(
        self, model_handle: int, texture: TextureIdStruct, value: RGBAStruct
    ) -> None:
        self._raise_if_error(
            self._lib.cj_model_texture_set_border_color(c_void_p(model_handle), texture, value)
        )

    def model_set_default_material_theme(self, model_handle: int, theme: str) -> None:
        view, _buffer = self._string_view(theme)
        self._raise_if_error(
            self._lib.cj_model_set_default_material_theme(c_void_p(model_handle), view)
        )

    def model_set_default_texture_theme(self, model_handle: int, theme: str) -> None:
        view, _buffer = self._string_view(theme)
        self._raise_if_error(
            self._lib.cj_model_set_default_texture_theme(c_void_p(model_handle), view)
        )

    def cityobject_draft_new(self, cityobject_id: str, cityobject_type: str) -> int:
        handle = c_void_p()
        id_view, _id_buffer = self._string_view(cityobject_id)
        type_view, _type_buffer = self._string_view(cityobject_type)
        self._raise_if_error(
            self._lib.cj_cityobject_draft_new(id_view, type_view, pointer(handle))
        )
        return int(handle.value)

    def cityobject_draft_set_geographical_extent(self, handle: int, bbox: BBoxStruct) -> None:
        self._raise_if_error(
            self._lib.cj_cityobject_draft_set_geographical_extent(c_void_p(handle), bbox)
        )

    def cityobject_draft_set_attribute(self, handle: int, key: str, value_handle: int) -> None:
        view, _buffer = self._string_view(key)
        self._raise_if_error(
            self._lib.cj_cityobject_draft_set_attribute(c_void_p(handle), view, c_void_p(value_handle))
        )

    def cityobject_draft_set_extra(self, handle: int, key: str, value_handle: int) -> None:
        view, _buffer = self._string_view(key)
        self._raise_if_error(
            self._lib.cj_cityobject_draft_set_extra(c_void_p(handle), view, c_void_p(value_handle))
        )

    def model_add_cityobject(self, model_handle: int, draft_handle: int) -> CityObjectIdStruct:
        cityobject = CityObjectIdStruct()
        self._raise_if_error(
            self._lib.cj_model_add_cityobject(
                c_void_p(model_handle), c_void_p(draft_handle), pointer(cityobject)
            )
        )
        return cityobject

    def model_cityobject_add_geometry(
        self, model_handle: int, cityobject: CityObjectIdStruct, geometry: GeometryIdStruct
    ) -> None:
        self._raise_if_error(
            self._lib.cj_model_cityobject_add_geometry(c_void_p(model_handle), cityobject, geometry)
        )

    def model_cityobject_add_parent(
        self, model_handle: int, child: CityObjectIdStruct, parent: CityObjectIdStruct
    ) -> None:
        self._raise_if_error(
            self._lib.cj_model_cityobject_add_parent(c_void_p(model_handle), child, parent)
        )

    def ring_draft_new(self) -> int:
        handle = c_void_p()
        self._raise_if_error(self._lib.cj_ring_draft_new(pointer(handle)))
        return int(handle.value)

    def ring_draft_push_vertex_index(self, handle: int, index: int) -> None:
        self._raise_if_error(self._lib.cj_ring_draft_push_vertex_index(c_void_p(handle), index))

    def ring_draft_push_vertex(self, handle: int, vertex: VertexStruct) -> None:
        self._raise_if_error(self._lib.cj_ring_draft_push_vertex(c_void_p(handle), vertex))

    def ring_draft_add_texture(
        self, handle: int, theme: str, texture: TextureIdStruct, uv_indices: list[int]
    ) -> None:
        theme_view, _buffer = self._string_view(theme)
        uv_values, _uv_buffer = self._uint32_array(uv_indices)
        self._raise_if_error(
            self._lib.cj_ring_draft_add_texture(
                c_void_p(handle), theme_view, texture, uv_values, len(uv_indices)
            )
        )

    def ring_draft_add_texture_uvs(
        self, handle: int, theme: str, texture: TextureIdStruct, uvs: list[UVStruct]
    ) -> None:
        theme_view, _buffer = self._string_view(theme)
        uv_values, _uv_buffer = self._uv_array(uvs)
        self._raise_if_error(
            self._lib.cj_ring_draft_add_texture_uvs(
                c_void_p(handle), theme_view, texture, uv_values, len(uvs)
            )
        )

    def surface_draft_new(self, outer_ring_handle: int) -> int:
        handle = c_void_p()
        self._raise_if_error(self._lib.cj_surface_draft_new(c_void_p(outer_ring_handle), pointer(handle)))
        return int(handle.value)

    def surface_draft_add_inner_ring(self, handle: int, inner_ring_handle: int) -> None:
        self._raise_if_error(
            self._lib.cj_surface_draft_add_inner_ring(c_void_p(handle), c_void_p(inner_ring_handle))
        )

    def surface_draft_set_semantic(self, handle: int, semantic: SemanticIdStruct) -> None:
        self._raise_if_error(self._lib.cj_surface_draft_set_semantic(c_void_p(handle), semantic))

    def surface_draft_add_material(
        self, handle: int, theme: str, material: MaterialIdStruct
    ) -> None:
        view, _buffer = self._string_view(theme)
        self._raise_if_error(
            self._lib.cj_surface_draft_add_material(c_void_p(handle), view, material)
        )

    def shell_draft_new(self) -> int:
        handle = c_void_p()
        self._raise_if_error(self._lib.cj_shell_draft_new(pointer(handle)))
        return int(handle.value)

    def shell_draft_add_surface(self, handle: int, surface_handle: int) -> None:
        self._raise_if_error(
            self._lib.cj_shell_draft_add_surface(c_void_p(handle), c_void_p(surface_handle))
        )

    def solid_draft_new(self, outer_shell_handle: int) -> int:
        handle = c_void_p()
        self._raise_if_error(self._lib.cj_solid_draft_new(c_void_p(outer_shell_handle), pointer(handle)))
        return int(handle.value)

    def solid_draft_add_inner_shell(self, handle: int, inner_shell_handle: int) -> None:
        self._raise_if_error(
            self._lib.cj_solid_draft_add_inner_shell(c_void_p(handle), c_void_p(inner_shell_handle))
        )

    def geometry_draft_new(self, geometry_type: GeometryType, lod: str | None) -> int:
        handle = c_void_p()
        view, _buffer = self._string_view(lod)
        self._raise_if_error(
            self._lib.cj_geometry_draft_new(int(geometry_type), view, pointer(handle))
        )
        return int(handle.value)

    def geometry_draft_new_instance(
        self,
        template_id: GeometryTemplateIdStruct,
        reference_vertex_index: int,
        transform: AffineTransform4x4Struct,
    ) -> int:
        handle = c_void_p()
        self._raise_if_error(
            self._lib.cj_geometry_draft_new_instance(
                template_id, reference_vertex_index, transform, pointer(handle)
            )
        )
        return int(handle.value)

    def geometry_draft_add_point_vertex_index(
        self, handle: int, vertex_index: int, semantic: SemanticIdStruct | None
    ) -> None:
        semantic_pointer, _holder = self._optional_semantic_pointer(semantic)
        self._raise_if_error(
            self._lib.cj_geometry_draft_add_point_vertex_index(
                c_void_p(handle), vertex_index, semantic_pointer
            )
        )

    def geometry_draft_add_linestring(
        self, handle: int, vertex_indices: list[int], semantic: SemanticIdStruct | None
    ) -> None:
        indices_pointer, _buffer = self._uint32_array(vertex_indices)
        semantic_pointer, _holder = self._optional_semantic_pointer(semantic)
        self._raise_if_error(
            self._lib.cj_geometry_draft_add_linestring(
                c_void_p(handle), indices_pointer, len(vertex_indices), semantic_pointer
            )
        )

    def geometry_draft_add_surface(self, handle: int, surface_handle: int) -> None:
        self._raise_if_error(
            self._lib.cj_geometry_draft_add_surface(c_void_p(handle), c_void_p(surface_handle))
        )

    def geometry_draft_add_solid(self, handle: int, solid_handle: int) -> None:
        self._raise_if_error(
            self._lib.cj_geometry_draft_add_solid(c_void_p(handle), c_void_p(solid_handle))
        )

    def model_add_geometry(self, model_handle: int, draft_handle: int) -> GeometryIdStruct:
        geometry = GeometryIdStruct()
        self._raise_if_error(
            self._lib.cj_model_add_geometry(
                c_void_p(model_handle), c_void_p(draft_handle), pointer(geometry)
            )
        )
        return geometry

    def model_add_geometry_template(
        self, model_handle: int, draft_handle: int
    ) -> GeometryTemplateIdStruct:
        geometry = GeometryTemplateIdStruct()
        self._raise_if_error(
            self._lib.cj_model_add_geometry_template(
                c_void_p(model_handle), c_void_p(draft_handle), pointer(geometry)
            )
        )
        return geometry

    def proj_transformer_create(self, source_crs: str, target_crs: str) -> int:
        source_view, _source_buffer = self._string_view(source_crs)
        target_view, _target_buffer = self._string_view(target_crs)
        handle = c_void_p()
        self._raise_if_error(
            self._lib.cj_proj_transformer_create(source_view, target_view, pointer(handle))
        )
        return int(handle.value)

    def proj_transformer_free(self, handle: int) -> None:
        if handle:
            self._raise_if_error(self._lib.cj_proj_transformer_free(c_void_p(handle)))

    def proj_transformer_transform(self, handle: int, x: float, y: float, z: float) -> VertexStruct:
        out = VertexStruct()
        self._raise_if_error(
            self._lib.cj_proj_transformer_transform(
                c_void_p(handle), VertexStruct(x=x, y=y, z=z), pointer(out)
            )
        )
        return out
