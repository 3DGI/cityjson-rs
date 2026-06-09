"""Python bindings for cityjson_lib built on top of the shared C ABI."""

from __future__ import annotations

from dataclasses import dataclass
from importlib.metadata import PackageNotFoundError, version as _package_version
from typing import Self

from cityjson_lib._ffi import (
    AffineTransform4x4Struct,
    BBoxStruct,
    CityJSONSeqAutoTransformOptionsPayload,
    CityJSONSeqWriteOptionsPayload,
    CityObjectIdStruct,
    ContactRole,
    ContactType,
    FfiLibrary,
    GeometryBoundaryPayload,
    GeometryIdStruct,
    GeometryTemplateIdStruct,
    GeometryType,
    ImageType,
    MaterialIdStruct,
    ModelCapacitiesStruct,
    ModelType,
    RGBAStruct,
    RGBStruct,
    RootKind,
    SemanticIdStruct,
    TextureIdStruct,
    TextureType,
    TransformStruct,
    VertexStruct,
    UVStruct,
    Version,
    WrapMode,
    WriteOptionsPayload,
)

try:
    __version__ = _package_version("cityjson-lib")
except PackageNotFoundError:
    __version__ = "0.9.0"

_ffi = FfiLibrary.load()


def _as_bytes(data: bytes | bytearray | memoryview) -> bytes:
    if isinstance(data, bytes):
        return data
    if isinstance(data, bytearray):
        return bytes(data)
    if isinstance(data, memoryview):
        return data.tobytes()
    raise TypeError("expected bytes-like data")


class _OwnedHandle:
    def __init__(self, handle: int) -> None:
        self._handle = handle

    def _require_handle(self) -> int:
        if self._handle == 0:
            raise RuntimeError(f"{type(self).__name__} has already been consumed or closed")
        return self._handle

    def _release_handle(self) -> int:
        handle = self._require_handle()
        self._handle = 0
        return handle

    def _free(self, handle: int) -> None:
        raise NotImplementedError

    def close(self) -> None:
        if self._handle != 0:
            self._free(self._handle)
            self._handle = 0

    def __del__(self) -> None:
        try:
            self.close()
        except Exception:
            pass


class _SolidDraft(_OwnedHandle):
    @classmethod
    def from_outer(cls, outer: "ShellDraft") -> "_SolidDraft":
        return cls(_ffi.solid_draft_new(outer._release_handle()))

    def _free(self, handle: int) -> None:
        _ffi.free_solid_draft(handle)

    def add_inner_shell(self, inner: "ShellDraft") -> "_SolidDraft":
        _ffi.solid_draft_add_inner_shell(self._require_handle(), inner._release_handle())
        return self


@dataclass(frozen=True)
class Probe:
    root_kind: RootKind
    version: Version
    has_version: bool


@dataclass(frozen=True)
class Vertex:
    x: float
    y: float
    z: float

    def to_native(self) -> VertexStruct:
        return VertexStruct(x=self.x, y=self.y, z=self.z)


@dataclass(frozen=True)
class UV:
    u: float
    v: float

    def to_native(self) -> UVStruct:
        return UVStruct(u=self.u, v=self.v)


@dataclass(frozen=True)
class BBox:
    min_x: float
    min_y: float
    min_z: float
    max_x: float
    max_y: float
    max_z: float

    def to_native(self) -> BBoxStruct:
        return BBoxStruct(
            min_x=self.min_x,
            min_y=self.min_y,
            min_z=self.min_z,
            max_x=self.max_x,
            max_y=self.max_y,
            max_z=self.max_z,
        )


@dataclass(frozen=True)
class RGB:
    r: float
    g: float
    b: float

    def to_native(self) -> RGBStruct:
        return RGBStruct(r=self.r, g=self.g, b=self.b)


@dataclass(frozen=True)
class RGBA:
    r: float
    g: float
    b: float
    a: float

    def to_native(self) -> RGBAStruct:
        return RGBAStruct(r=self.r, g=self.g, b=self.b, a=self.a)


@dataclass(frozen=True)
class AffineTransform4x4:
    elements: tuple[float, ...]

    def __post_init__(self) -> None:
        if len(self.elements) != 16:
            raise ValueError("AffineTransform4x4.elements must contain exactly 16 values")

    def to_native(self) -> AffineTransform4x4Struct:
        native = AffineTransform4x4Struct()
        native.elements[:] = self.elements
        return native


@dataclass(frozen=True)
class GeometryBoundary:
    geometry_type: GeometryType
    has_boundaries: bool
    vertex_indices: list[int]
    ring_offsets: list[int]
    surface_offsets: list[int]
    shell_offsets: list[int]
    solid_offsets: list[int]

    def to_native_payload(self) -> GeometryBoundaryPayload:
        return GeometryBoundaryPayload(
            geometry_type=self.geometry_type,
            has_boundaries=self.has_boundaries,
            vertex_indices=self.vertex_indices,
            ring_offsets=self.ring_offsets,
            surface_offsets=self.surface_offsets,
            shell_offsets=self.shell_offsets,
            solid_offsets=self.solid_offsets,
        )


@dataclass(frozen=True)
class GeometrySelectionSpec:
    cityobject_id: str
    geometry_index: int


@dataclass(frozen=True)
class WriteOptions:
    pretty: bool = False
    validate_default_themes: bool = True

    def to_native(self):
        return _ffi.write_options(
            WriteOptionsPayload(
                pretty=self.pretty,
                validate_default_themes=self.validate_default_themes,
            )
        )


@dataclass(frozen=True)
class CityJSONSeqWriteOptions:
    validate_default_themes: bool = True
    trailing_newline: bool = True
    update_metadata_geographical_extent: bool = True

    def to_native(self):
        return _ffi.cityjsonseq_write_options(
            CityJSONSeqWriteOptionsPayload(
                validate_default_themes=self.validate_default_themes,
                trailing_newline=self.trailing_newline,
                update_metadata_geographical_extent=self.update_metadata_geographical_extent,
            )
        )


@dataclass(frozen=True)
class AutoTransformOptions:
    scale: tuple[float, float, float] = (0.001, 0.001, 0.001)
    validate_default_themes: bool = True
    trailing_newline: bool = True
    update_metadata_geographical_extent: bool = True

    def to_native(self):
        return _ffi.cityjsonseq_auto_transform_options(
            CityJSONSeqAutoTransformOptionsPayload(
                scale=self.scale,
                validate_default_themes=self.validate_default_themes,
                trailing_newline=self.trailing_newline,
                update_metadata_geographical_extent=self.update_metadata_geographical_extent,
            )
        )


@dataclass(frozen=True)
class Transform:
    scale: tuple[float, float, float]
    translate: tuple[float, float, float]

    def to_native(self) -> TransformStruct:
        return TransformStruct(
            scale_x=self.scale[0],
            scale_y=self.scale[1],
            scale_z=self.scale[2],
            translate_x=self.translate[0],
            translate_y=self.translate[1],
            translate_z=self.translate[2],
        )


@dataclass(frozen=True)
class ModelCapacities:
    cityobjects: int = 0
    vertices: int = 0
    semantics: int = 0
    materials: int = 0
    textures: int = 0
    geometries: int = 0
    template_vertices: int = 0
    template_geometries: int = 0
    uv_coordinates: int = 0

    def to_native(self) -> ModelCapacitiesStruct:
        return ModelCapacitiesStruct(
            cityobjects=self.cityobjects,
            vertices=self.vertices,
            semantics=self.semantics,
            materials=self.materials,
            textures=self.textures,
            geometries=self.geometries,
            template_vertices=self.template_vertices,
            template_geometries=self.template_geometries,
            uv_coordinates=self.uv_coordinates,
        )


@dataclass(frozen=True)
class ModelSummary:
    model_type: ModelType
    version: Version
    cityobject_count: int
    geometry_count: int
    geometry_template_count: int
    vertex_count: int
    template_vertex_count: int
    uv_coordinate_count: int
    semantic_count: int
    material_count: int
    texture_count: int
    extension_count: int
    has_metadata: bool
    has_transform: bool
    has_templates: bool
    has_appearance: bool


@dataclass(frozen=True)
class GeometryId:
    slot: int
    generation: int

    @classmethod
    def from_native(cls, native: GeometryIdStruct) -> "GeometryId":
        return cls(slot=int(native.slot), generation=int(native.generation))

    def to_native(self) -> GeometryIdStruct:
        return GeometryIdStruct(slot=self.slot, generation=self.generation, reserved=0)


@dataclass(frozen=True)
class SemanticId:
    slot: int
    generation: int

    @classmethod
    def from_native(cls, native: SemanticIdStruct) -> "SemanticId":
        return cls(slot=int(native.slot), generation=int(native.generation))

    def to_native(self) -> SemanticIdStruct:
        return SemanticIdStruct(slot=self.slot, generation=self.generation, reserved=0)


@dataclass(frozen=True)
class MaterialId:
    slot: int
    generation: int

    @classmethod
    def from_native(cls, native: MaterialIdStruct) -> "MaterialId":
        return cls(slot=int(native.slot), generation=int(native.generation))

    def to_native(self) -> MaterialIdStruct:
        return MaterialIdStruct(slot=self.slot, generation=self.generation, reserved=0)


@dataclass(frozen=True)
class TextureId:
    slot: int
    generation: int

    @classmethod
    def from_native(cls, native: TextureIdStruct) -> "TextureId":
        return cls(slot=int(native.slot), generation=int(native.generation))

    def to_native(self) -> TextureIdStruct:
        return TextureIdStruct(slot=self.slot, generation=self.generation, reserved=0)


@dataclass(frozen=True)
class CityObjectId:
    slot: int
    generation: int

    @classmethod
    def from_native(cls, native: CityObjectIdStruct) -> "CityObjectId":
        return cls(slot=int(native.slot), generation=int(native.generation))

    def to_native(self) -> CityObjectIdStruct:
        return CityObjectIdStruct(slot=self.slot, generation=self.generation, reserved=0)


@dataclass(frozen=True)
class GeometryTemplateId:
    slot: int
    generation: int

    @classmethod
    def from_native(cls, native: GeometryTemplateIdStruct) -> "GeometryTemplateId":
        return cls(slot=int(native.slot), generation=int(native.generation))

    def to_native(self) -> GeometryTemplateIdStruct:
        return GeometryTemplateIdStruct(slot=self.slot, generation=self.generation, reserved=0)


def _vertex_to_native(vertex: Vertex):
    return vertex.to_native()


class Value(_OwnedHandle):
    @classmethod
    def null(cls) -> "Value":
        return cls(_ffi.value_new_null())

    @classmethod
    def boolean(cls, value: bool) -> "Value":
        return cls(_ffi.value_new_bool(value))

    @classmethod
    def integer(cls, value: int) -> "Value":
        return cls(_ffi.value_new_int64(value))

    @classmethod
    def number(cls, value: float) -> "Value":
        return cls(_ffi.value_new_float64(value))

    @classmethod
    def string(cls, value: str) -> "Value":
        return cls(_ffi.value_new_string(value))

    @classmethod
    def geometry(cls, value: GeometryId) -> "Value":
        return cls(_ffi.value_new_geometry_ref(value.to_native()))

    @classmethod
    def array(cls) -> "Value":
        return cls(_ffi.value_new_array())

    @classmethod
    def object(cls) -> "Value":
        return cls(_ffi.value_new_object())

    def _free(self, handle: int) -> None:
        _ffi.free_value(handle)

    def push(self, value: "Value") -> Self:
        _ffi.value_array_push(self._require_handle(), value._release_handle())
        return self

    def insert(self, key: str, value: "Value") -> Self:
        _ffi.value_object_insert(self._require_handle(), key, value._release_handle())
        return self


class Contact(_OwnedHandle):
    def __init__(self) -> None:
        super().__init__(_ffi.contact_new())

    def _free(self, handle: int) -> None:
        _ffi.free_contact(handle)

    def set_name(self, value: str) -> Self:
        _ffi.contact_set_name(self._require_handle(), value)
        return self

    def set_email(self, value: str) -> Self:
        _ffi.contact_set_email(self._require_handle(), value)
        return self

    def set_role(self, value: ContactRole) -> Self:
        _ffi.contact_set_role(self._require_handle(), value)
        return self

    def set_website(self, value: str) -> Self:
        _ffi.contact_set_website(self._require_handle(), value)
        return self

    def set_type(self, value: ContactType) -> Self:
        _ffi.contact_set_type(self._require_handle(), value)
        return self

    def set_phone(self, value: str) -> Self:
        _ffi.contact_set_phone(self._require_handle(), value)
        return self

    def set_organization(self, value: str) -> Self:
        _ffi.contact_set_organization(self._require_handle(), value)
        return self

    def set_address(self, object_value: Value) -> Self:
        _ffi.contact_set_address(self._require_handle(), object_value._release_handle())
        return self


class CityObjectDraft(_OwnedHandle):
    def __init__(self, cityobject_id: str, cityobject_type: str) -> None:
        super().__init__(_ffi.cityobject_draft_new(cityobject_id, cityobject_type))

    def _free(self, handle: int) -> None:
        _ffi.free_cityobject_draft(handle)

    def set_geographical_extent(self, bbox: BBox) -> Self:
        _ffi.cityobject_draft_set_geographical_extent(self._require_handle(), bbox.to_native())
        return self

    def set_attribute(self, key: str, value: Value) -> Self:
        _ffi.cityobject_draft_set_attribute(self._require_handle(), key, value._release_handle())
        return self

    def set_extra(self, key: str, value: Value) -> Self:
        _ffi.cityobject_draft_set_extra(self._require_handle(), key, value._release_handle())
        return self


class RingDraft(_OwnedHandle):
    def __init__(self) -> None:
        super().__init__(_ffi.ring_draft_new())

    def _free(self, handle: int) -> None:
        _ffi.free_ring_draft(handle)

    def push_vertex_index(self, index: int) -> Self:
        _ffi.ring_draft_push_vertex_index(self._require_handle(), index)
        return self

    def push_vertex(self, vertex: Vertex) -> Self:
        _ffi.ring_draft_push_vertex(self._require_handle(), _vertex_to_native(vertex))
        return self

    def add_texture(self, theme: str, texture: TextureId, uv_indices: list[int]) -> Self:
        _ffi.ring_draft_add_texture(self._require_handle(), theme, texture.to_native(), uv_indices)
        return self

    def add_texture_uvs(self, theme: str, texture: TextureId, uvs: list[UV]) -> Self:
        _ffi.ring_draft_add_texture_uvs(
            self._require_handle(),
            theme,
            texture.to_native(),
            [uv.to_native() for uv in uvs],
        )
        return self


class SurfaceDraft(_OwnedHandle):
    def __init__(self, outer: RingDraft) -> None:
        super().__init__(_ffi.surface_draft_new(outer._release_handle()))

    def _free(self, handle: int) -> None:
        _ffi.free_surface_draft(handle)

    def add_inner_ring(self, inner: RingDraft) -> Self:
        _ffi.surface_draft_add_inner_ring(self._require_handle(), inner._release_handle())
        return self

    def set_semantic(self, semantic: SemanticId) -> Self:
        _ffi.surface_draft_set_semantic(self._require_handle(), semantic.to_native())
        return self

    def add_material(self, theme: str, material: MaterialId) -> Self:
        _ffi.surface_draft_add_material(self._require_handle(), theme, material.to_native())
        return self


class ShellDraft(_OwnedHandle):
    def __init__(self) -> None:
        super().__init__(_ffi.shell_draft_new())

    def _free(self, handle: int) -> None:
        _ffi.free_shell_draft(handle)

    def add_surface(self, surface: SurfaceDraft) -> Self:
        _ffi.shell_draft_add_surface(self._require_handle(), surface._release_handle())
        return self


class GeometryDraft(_OwnedHandle):
    def __init__(self, handle: int) -> None:
        super().__init__(handle)

    def _free(self, handle: int) -> None:
        _ffi.free_geometry_draft(handle)

    @classmethod
    def multi_point(cls, lod: str | None = None) -> "GeometryDraft":
        return cls(_ffi.geometry_draft_new(GeometryType.MULTI_POINT, lod))

    @classmethod
    def multi_line_string(cls, lod: str | None = None) -> "GeometryDraft":
        return cls(_ffi.geometry_draft_new(GeometryType.MULTI_LINE_STRING, lod))

    @classmethod
    def multi_surface(cls, lod: str | None = None) -> "GeometryDraft":
        return cls(_ffi.geometry_draft_new(GeometryType.MULTI_SURFACE, lod))

    @classmethod
    def composite_surface(cls, lod: str | None = None) -> "GeometryDraft":
        return cls(_ffi.geometry_draft_new(GeometryType.COMPOSITE_SURFACE, lod))

    @classmethod
    def solid(cls, lod: str | None = None) -> "GeometryDraft":
        return cls(_ffi.geometry_draft_new(GeometryType.SOLID, lod))

    @classmethod
    def multi_solid(cls, lod: str | None = None) -> "GeometryDraft":
        return cls(_ffi.geometry_draft_new(GeometryType.MULTI_SOLID, lod))

    @classmethod
    def composite_solid(cls, lod: str | None = None) -> "GeometryDraft":
        return cls(_ffi.geometry_draft_new(GeometryType.COMPOSITE_SOLID, lod))

    @classmethod
    def instance(
        cls,
        template_id: GeometryTemplateId,
        reference_vertex_index: int,
        transform: AffineTransform4x4,
    ) -> "GeometryDraft":
        return cls(
            _ffi.geometry_draft_new_instance(
                template_id.to_native(),
                reference_vertex_index,
                transform.to_native(),
            )
        )

    def add_point(self, vertex_index: int, semantic: SemanticId | None = None) -> Self:
        native_semantic = semantic.to_native() if semantic is not None else None
        _ffi.geometry_draft_add_point_vertex_index(
            self._require_handle(), vertex_index, native_semantic
        )
        return self

    def add_linestring(
        self, vertex_indices: list[int], semantic: SemanticId | None = None
    ) -> Self:
        native_semantic = semantic.to_native() if semantic is not None else None
        _ffi.geometry_draft_add_linestring(
            self._require_handle(), vertex_indices, native_semantic
        )
        return self

    def add_surface(self, surface: SurfaceDraft) -> Self:
        _ffi.geometry_draft_add_surface(self._require_handle(), surface._release_handle())
        return self

    def add_solid(self, outer: ShellDraft, inner_shells: list[ShellDraft] | None = None) -> Self:
        solid = _SolidDraft.from_outer(outer)
        try:
            for inner in inner_shells or []:
                solid.add_inner_shell(inner)
            _ffi.geometry_draft_add_solid(self._require_handle(), solid._release_handle())
        finally:
            solid.close()
        return self


def probe_bytes(data: bytes | bytearray | memoryview) -> Probe:
    native = _ffi.probe(_as_bytes(data))
    return Probe(
        root_kind=RootKind(native.root_kind),
        version=Version(native.version),
        has_version=bool(native.has_version),
    )


class ModelSelection(_OwnedHandle):
    def _free(self, handle: int) -> None:
        _ffi.free_model_selection(handle)

    @classmethod
    def select_cityobjects_by_id(cls, model: "CityModel", cityobject_ids: list[str]) -> Self:
        return cls(_ffi.select_cityobjects_by_id(model._require_handle(), cityobject_ids))

    @classmethod
    def select_geometries_by_cityobject_id_and_index(
        cls,
        model: "CityModel",
        specs: list[GeometrySelectionSpec | tuple[str, int]],
    ) -> Self:
        return cls(
            _ffi.select_geometries_by_cityobject_id_and_index(
                model._require_handle(),
                [_geometry_selection_spec_tuple(spec) for spec in specs],
            )
        )

    def include_relatives(self, model: "CityModel") -> Self:
        return type(self)(
            _ffi.model_selection_include_relatives(
                self._require_handle(),
                model._require_handle(),
            )
        )

    def union(self, other: Self) -> Self:
        return type(self)(
            _ffi.model_selection_union(self._require_handle(), other._require_handle())
        )

    def intersection(self, other: Self) -> Self:
        return type(self)(
            _ffi.model_selection_intersection(self._require_handle(), other._require_handle())
        )

    def is_empty(self) -> bool:
        return _ffi.model_selection_is_empty(self._require_handle())


def _geometry_selection_spec_tuple(
    spec: GeometrySelectionSpec | tuple[str, int],
) -> tuple[str, int]:
    if isinstance(spec, GeometrySelectionSpec):
        return (spec.cityobject_id, spec.geometry_index)
    return spec


class ProjTransformer(_OwnedHandle):
    def __init__(self, handle: int) -> None:
        super().__init__(handle)

    def _free(self, handle: int) -> None:
        _ffi.proj_transformer_free(handle)

    @classmethod
    def create(cls, source_crs: str, target_crs: str) -> Self:
        return cls(_ffi.proj_transformer_create(source_crs, target_crs))

    def transform(self, vertex: Vertex) -> Vertex:
        transformed = _ffi.proj_transformer_transform(
            self._require_handle(), vertex.x, vertex.y, vertex.z
        )
        return Vertex(x=transformed.x, y=transformed.y, z=transformed.z)


class CityModel(_OwnedHandle):
    def __init__(self, handle: int) -> None:
        super().__init__(handle)

    def _free(self, handle: int) -> None:
        _ffi.free_model(handle)

    @classmethod
    def from_document_bytes(cls, data: bytes | bytearray | memoryview) -> Self:
        return cls.parse_document_bytes(data)

    @classmethod
    def parse_document_bytes(cls, data: bytes | bytearray | memoryview) -> Self:
        return cls(_ffi.parse_document(_as_bytes(data)))

    @classmethod
    def parse_feature_bytes(cls, data: bytes | bytearray | memoryview) -> Self:
        return cls(_ffi.parse_feature(_as_bytes(data)))

    @classmethod
    def parse_feature_with_base_bytes(
        cls,
        feature_data: bytes | bytearray | memoryview,
        base_data: bytes | bytearray | memoryview,
    ) -> Self:
        return cls(_ffi.parse_feature_with_base(_as_bytes(feature_data), _as_bytes(base_data)))

    @classmethod
    def from_arrow_bytes(cls, data: bytes | bytearray | memoryview) -> Self:
        return cls.parse_arrow_bytes(data)

    @classmethod
    def parse_arrow_bytes(cls, data: bytes | bytearray | memoryview) -> Self:
        return cls(_ffi.parse_arrow(_as_bytes(data)))

    @classmethod
    def from_parquet_file(cls, path: str) -> Self:
        return cls.parse_parquet_file(path)

    @classmethod
    def parse_parquet_file(cls, path: str) -> Self:
        return cls(_ffi.parse_parquet_file(path))

    @classmethod
    def from_parquet_dataset_dir(cls, path: str) -> Self:
        return cls.parse_parquet_dataset_dir(path)

    @classmethod
    def parse_parquet_dataset_dir(cls, path: str) -> Self:
        return cls(_ffi.parse_parquet_dataset_dir(path))

    @classmethod
    def create(cls, *, model_type: ModelType) -> Self:
        return cls(_ffi.create(model_type))

    def summary(self) -> ModelSummary:
        native = _ffi.summary(self._require_handle())
        return ModelSummary(
            model_type=ModelType(native.model_type),
            version=Version(native.version),
            cityobject_count=native.cityobject_count,
            geometry_count=native.geometry_count,
            geometry_template_count=native.geometry_template_count,
            vertex_count=native.vertex_count,
            template_vertex_count=native.template_vertex_count,
            uv_coordinate_count=native.uv_coordinate_count,
            semantic_count=native.semantic_count,
            material_count=native.material_count,
            texture_count=native.texture_count,
            extension_count=native.extension_count,
            has_metadata=bool(native.has_metadata),
            has_transform=bool(native.has_transform),
            has_templates=bool(native.has_templates),
            has_appearance=bool(native.has_appearance),
        )

    def metadata_title(self) -> str:
        return _ffi.metadata_title(self._require_handle())

    def metadata_identifier(self) -> str:
        return _ffi.metadata_identifier(self._require_handle())

    def cityobject_ids(self) -> list[str]:
        return _ffi.cityobject_ids(self._require_handle())

    def geometry_types(self) -> list[GeometryType]:
        return _ffi.geometry_types(self._require_handle())

    def geometry_boundary(self, index: int) -> GeometryBoundary:
        payload = _ffi.geometry_boundary(self._require_handle(), index)
        return GeometryBoundary(
            geometry_type=payload.geometry_type,
            has_boundaries=payload.has_boundaries,
            vertex_indices=payload.vertex_indices,
            ring_offsets=payload.ring_offsets,
            surface_offsets=payload.surface_offsets,
            shell_offsets=payload.shell_offsets,
            solid_offsets=payload.solid_offsets,
        )

    def geometry_boundary_coordinates(self, index: int) -> list[Vertex]:
        return [
            Vertex(x=item.x, y=item.y, z=item.z)
            for item in _ffi.geometry_boundary_coordinates(self._require_handle(), index)
        ]

    def uv_coordinates(self) -> list[UV]:
        return [UV(u=item.u, v=item.v) for item in _ffi.uv_coordinates(self._require_handle())]

    def set_metadata_title(self, title: str) -> None:
        _ffi.set_metadata_title(self._require_handle(), title)

    def set_metadata_identifier(self, identifier: str) -> None:
        _ffi.set_metadata_identifier(self._require_handle(), identifier)

    def set_metadata_geographical_extent(self, bbox: BBox) -> None:
        _ffi.set_metadata_geographical_extent(self._require_handle(), bbox.to_native())

    def set_metadata_reference_date(self, value: str) -> None:
        _ffi.set_metadata_reference_date(self._require_handle(), value)

    def set_metadata_reference_system(self, value: str) -> None:
        _ffi.set_metadata_reference_system(self._require_handle(), value)

    def set_metadata_contact(self, contact: Contact) -> None:
        _ffi.model_set_metadata_contact(self._require_handle(), contact._release_handle())

    def set_metadata_extra(self, key: str, value: Value) -> None:
        _ffi.model_set_metadata_extra(self._require_handle(), key, value._release_handle())

    def set_root_extra(self, key: str, value: Value) -> None:
        _ffi.model_set_root_extra(self._require_handle(), key, value._release_handle())

    def add_extension(self, name: str, url: str, version: str) -> None:
        _ffi.model_add_extension(self._require_handle(), name, url, version)

    def set_transform(self, transform: Transform) -> None:
        _ffi.set_transform(self._require_handle(), transform.to_native())

    def clear_transform(self) -> None:
        _ffi.clear_transform(self._require_handle())

    def reproject(self, target_crs: str) -> None:
        _ffi.reproject(self._require_handle(), target_crs)

    def remove_cityobject(self, cityobject_id: str) -> None:
        _ffi.remove_cityobject(self._require_handle(), cityobject_id)

    def append_model(self, other: Self) -> None:
        _ffi.append_model(self._require_handle(), other._require_handle())

    def subset_cityobjects(self, cityobject_ids: list[str], exclude: bool = False) -> Self:
        return type(self)(
            _ffi.subset_cityobjects(self._require_handle(), cityobject_ids, exclude)
        )

    def extract_selection(self, selection: ModelSelection) -> Self:
        return type(self)(
            _ffi.extract_selection(self._require_handle(), selection._require_handle())
        )

    def cleanup(self) -> None:
        _ffi.cleanup(self._require_handle())

    def serialize_document(self, options: WriteOptions | None = None) -> str:
        return self.serialize_document_bytes(options).decode("utf-8")

    def serialize_document_bytes(self, options: WriteOptions | None = None) -> bytes:
        payload = options.to_native() if options is not None else WriteOptions().to_native()
        return _ffi.serialize_document_with_options(self._require_handle(), payload)

    def to_json_bytes(self, options: WriteOptions | None = None) -> bytes:
        return self.serialize_document_bytes(options)

    def serialize_feature(self, options: WriteOptions | None = None) -> str:
        return self.serialize_feature_bytes(options).decode("utf-8")

    def serialize_feature_bytes(self, options: WriteOptions | None = None) -> bytes:
        payload = options.to_native() if options is not None else WriteOptions().to_native()
        return _ffi.serialize_feature_with_options(self._require_handle(), payload)

    def serialize_arrow_bytes(self) -> bytes:
        return _ffi.serialize_arrow(self._require_handle())

    def to_arrow_bytes(self) -> bytes:
        return self.serialize_arrow_bytes()

    def serialize_parquet_file(self, path: str) -> None:
        _ffi.serialize_parquet_file(self._require_handle(), path)

    def to_parquet_file(self, path: str) -> None:
        self.serialize_parquet_file(path)

    def serialize_parquet_dataset_dir(self, path: str) -> None:
        _ffi.serialize_parquet_dataset_dir(self._require_handle(), path)

    def to_parquet_dataset_dir(self, path: str) -> None:
        self.serialize_parquet_dataset_dir(path)

    def reserve_import(self, capacities: ModelCapacities) -> None:
        _ffi.reserve_import(self._require_handle(), capacities.to_native())

    def add_vertex(self, vertex: Vertex) -> int:
        return _ffi.add_vertex(self._require_handle(), vertex.x, vertex.y, vertex.z)

    def add_template_vertex(self, vertex: Vertex) -> int:
        return _ffi.add_template_vertex(self._require_handle(), vertex.x, vertex.y, vertex.z)

    def set_vertex(self, index: int, vertex: Vertex) -> None:
        _ffi.set_vertex(self._require_handle(), index, vertex.x, vertex.y, vertex.z)

    def set_template_vertex(self, index: int, vertex: Vertex) -> None:
        _ffi.set_template_vertex(self._require_handle(), index, vertex.x, vertex.y, vertex.z)

    def add_uv_coordinate(self, uv: UV) -> int:
        return _ffi.add_uv_coordinate(self._require_handle(), uv.u, uv.v)

    def add_semantic(self, semantic_type: str) -> SemanticId:
        return SemanticId.from_native(_ffi.model_add_semantic(self._require_handle(), semantic_type))

    def set_semantic_parent(self, semantic: SemanticId, parent: SemanticId) -> None:
        _ffi.model_set_semantic_parent(
            self._require_handle(), semantic.to_native(), parent.to_native()
        )

    def set_semantic_extra(self, semantic: SemanticId, key: str, value: Value) -> None:
        _ffi.model_semantic_set_extra(
            self._require_handle(), semantic.to_native(), key, value._release_handle()
        )

    def add_material(self, name: str) -> MaterialId:
        return MaterialId.from_native(_ffi.model_add_material(self._require_handle(), name))

    def set_material_ambient_intensity(self, material: MaterialId, value: float) -> None:
        _ffi.model_material_set_ambient_intensity(
            self._require_handle(), material.to_native(), value
        )

    def set_material_diffuse_color(self, material: MaterialId, value: RGB) -> None:
        _ffi.model_material_set_diffuse_color(
            self._require_handle(), material.to_native(), value.to_native()
        )

    def set_material_emissive_color(self, material: MaterialId, value: RGB) -> None:
        _ffi.model_material_set_emissive_color(
            self._require_handle(), material.to_native(), value.to_native()
        )

    def set_material_specular_color(self, material: MaterialId, value: RGB) -> None:
        _ffi.model_material_set_specular_color(
            self._require_handle(), material.to_native(), value.to_native()
        )

    def set_material_shininess(self, material: MaterialId, value: float) -> None:
        _ffi.model_material_set_shininess(self._require_handle(), material.to_native(), value)

    def set_material_transparency(self, material: MaterialId, value: float) -> None:
        _ffi.model_material_set_transparency(
            self._require_handle(), material.to_native(), value
        )

    def set_material_is_smooth(self, material: MaterialId, value: bool) -> None:
        _ffi.model_material_set_is_smooth(self._require_handle(), material.to_native(), value)

    def add_texture(self, image: str, image_type: ImageType) -> TextureId:
        return TextureId.from_native(_ffi.model_add_texture(self._require_handle(), image, image_type))

    def set_texture_wrap_mode(self, texture: TextureId, value: WrapMode) -> None:
        _ffi.model_texture_set_wrap_mode(self._require_handle(), texture.to_native(), value)

    def set_texture_type(self, texture: TextureId, value: TextureType) -> None:
        _ffi.model_texture_set_texture_type(self._require_handle(), texture.to_native(), value)

    def set_texture_border_color(self, texture: TextureId, value: RGBA) -> None:
        _ffi.model_texture_set_border_color(
            self._require_handle(), texture.to_native(), value.to_native()
        )

    def set_default_material_theme(self, theme: str) -> None:
        _ffi.model_set_default_material_theme(self._require_handle(), theme)

    def set_default_texture_theme(self, theme: str) -> None:
        _ffi.model_set_default_texture_theme(self._require_handle(), theme)

    def add_geometry(self, draft: GeometryDraft) -> GeometryId:
        return GeometryId.from_native(
            _ffi.model_add_geometry(self._require_handle(), draft._release_handle())
        )

    def add_geometry_template(self, draft: GeometryDraft) -> GeometryTemplateId:
        return GeometryTemplateId.from_native(
            _ffi.model_add_geometry_template(self._require_handle(), draft._release_handle())
        )

    def add_cityobject(self, draft: CityObjectDraft) -> CityObjectId:
        return CityObjectId.from_native(
            _ffi.model_add_cityobject(self._require_handle(), draft._release_handle())
        )

    def add_cityobject_geometry(self, cityobject: CityObjectId, geometry: GeometryId) -> None:
        _ffi.model_cityobject_add_geometry(
            self._require_handle(), cityobject.to_native(), geometry.to_native()
        )

    def add_cityobject_parent(self, child: CityObjectId, parent: CityObjectId) -> None:
        _ffi.model_cityobject_add_parent(
            self._require_handle(), child.to_native(), parent.to_native()
        )


def merge_feature_stream_bytes(data: bytes | bytearray | memoryview) -> CityModel:
    return CityModel(_ffi.parse_feature_stream_merge(_as_bytes(data)))


def merge_models(models: list[CityModel]) -> CityModel:
    handles = [model._require_handle() for model in models]
    return CityModel(_ffi.merge_models(handles))


def serialize_feature_stream(
    models: list[CityModel],
    options: WriteOptions | None = None,
) -> str:
    return serialize_feature_stream_bytes(models, options).decode("utf-8")


def serialize_feature_stream_bytes(
    models: list[CityModel],
    options: WriteOptions | None = None,
) -> bytes:
    payload = options.to_native() if options is not None else WriteOptions().to_native()
    handles = [model._require_handle() for model in models]
    return _ffi.serialize_feature_stream(handles, payload)


def write_cityjsonseq_with_transform(
    base_root: CityModel,
    features: list[CityModel],
    transform: Transform,
    options: CityJSONSeqWriteOptions | None = None,
) -> str:
    return write_cityjsonseq_with_transform_bytes(
        base_root,
        features,
        transform,
        options,
    ).decode("utf-8")


def write_cityjsonseq_with_transform_bytes(
    base_root: CityModel,
    features: list[CityModel],
    transform: Transform,
    options: CityJSONSeqWriteOptions | None = None,
) -> bytes:
    payload = (
        options.to_native()
        if options is not None
        else CityJSONSeqWriteOptions().to_native()
    )
    handles = [model._require_handle() for model in features]
    return _ffi.serialize_cityjsonseq_with_transform(
        base_root._require_handle(),
        handles,
        transform.to_native(),
        payload,
    )


def write_cityjsonseq_auto_transform(
    base_root: CityModel,
    features: list[CityModel],
    options: AutoTransformOptions | None = None,
) -> str:
    return write_cityjsonseq_auto_transform_bytes(
        base_root,
        features,
        options,
    ).decode("utf-8")


def write_cityjsonseq_auto_transform_bytes(
    base_root: CityModel,
    features: list[CityModel],
    options: AutoTransformOptions | None = None,
) -> bytes:
    payload = (
        options.to_native()
        if options is not None
        else AutoTransformOptions().to_native()
    )
    handles = [model._require_handle() for model in features]
    return _ffi.serialize_cityjsonseq_auto_transform(
        base_root._require_handle(),
        handles,
        payload,
    )


Model = CityModel

__all__ = [
    "AffineTransform4x4",
    "AutoTransformOptions",
    "BBox",
    "CityJSONSeqWriteOptions",
    "CityModel",
    "CityObjectDraft",
    "CityObjectId",
    "Contact",
    "ContactRole",
    "ContactType",
    "GeometryBoundary",
    "GeometryDraft",
    "GeometryId",
    "GeometrySelectionSpec",
    "GeometryTemplateId",
    "GeometryType",
    "ImageType",
    "MaterialId",
    "Model",
    "ModelCapacities",
    "ModelSelection",
    "ModelSummary",
    "ModelType",
    "ProjTransformer",
    "Probe",
    "RGBA",
    "RGB",
    "RingDraft",
    "RootKind",
    "SemanticId",
    "ShellDraft",
    "SurfaceDraft",
    "TextureId",
    "TextureType",
    "Transform",
    "UV",
    "Value",
    "Version",
    "Vertex",
    "WrapMode",
    "merge_feature_stream_bytes",
    "merge_models",
    "probe_bytes",
    "serialize_feature_stream",
    "serialize_feature_stream_bytes",
    "write_cityjsonseq_auto_transform",
    "write_cityjsonseq_auto_transform_bytes",
    "write_cityjsonseq_with_transform",
    "write_cityjsonseq_with_transform_bytes",
]
