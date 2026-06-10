from __future__ import annotations

import json
import os
from collections.abc import Mapping
from dataclasses import dataclass, field
from enum import Enum
from typing import TYPE_CHECKING, Any, ClassVar, Self

from . import _native

if TYPE_CHECKING:
    from cityjson_lib import CityModel


class _LodSelectionKind(Enum):
    ALL = "all"
    HIGHEST = "highest"
    EXACT = "exact"


@dataclass(frozen=True, slots=True)
class LodSelection:
    kind: _LodSelectionKind
    exact_lod: str | None = None

    ALL: ClassVar["LodSelection"]
    HIGHEST: ClassVar["LodSelection"]

    @classmethod
    def all(cls) -> Self:
        return cls(_LodSelectionKind.ALL)

    @classmethod
    def highest(cls) -> Self:
        return cls(_LodSelectionKind.HIGHEST)

    @classmethod
    def exact(cls, lod: str) -> Self:
        if not lod:
            raise ValueError("lod must not be empty")
        return cls(_LodSelectionKind.EXACT, lod)

    @classmethod
    def Exact(cls, lod: str) -> Self:
        return cls.exact(lod)

    @property
    def _native_kind(self) -> int:
        if self.kind is _LodSelectionKind.ALL:
            return 0
        if self.kind is _LodSelectionKind.HIGHEST:
            return 1
        return 2


LodSelection.ALL = LodSelection.all()
LodSelection.HIGHEST = LodSelection.highest()


@dataclass(frozen=True, slots=True)
class PackageFilter:
    cityobject_types: frozenset[str] | None = None
    default_lod: LodSelection = field(default_factory=LodSelection.all)
    lods_by_type: Mapping[str, LodSelection] = field(default_factory=dict)

    def __post_init__(self) -> None:
        if self.cityobject_types is not None:
            object.__setattr__(self, "cityobject_types", frozenset(self.cityobject_types))
        object.__setattr__(self, "lods_by_type", dict(self.lods_by_type))


@dataclass(frozen=True, slots=True)
class MissingLodSelection:
    cityobject_type: str
    requested_lod: str
    available_lods: frozenset[str] = field(default_factory=frozenset)

    def __post_init__(self) -> None:
        object.__setattr__(self, "available_lods", frozenset(self.available_lods))


@dataclass(frozen=True, slots=True)
class PackageFilterReport:
    available_types: frozenset[str] = field(default_factory=frozenset)
    retained_types: frozenset[str] = field(default_factory=frozenset)
    ignored_types: frozenset[str] = field(default_factory=frozenset)
    available_lods: Mapping[str, frozenset[str]] = field(default_factory=dict)
    retained_lods: Mapping[str, frozenset[str]] = field(default_factory=dict)
    missing_lods: list[MissingLodSelection] = field(default_factory=list)
    retained_geometry_count: int = 0

    def __post_init__(self) -> None:
        object.__setattr__(self, "available_types", frozenset(self.available_types))
        object.__setattr__(self, "retained_types", frozenset(self.retained_types))
        object.__setattr__(self, "ignored_types", frozenset(self.ignored_types))
        object.__setattr__(
            self,
            "available_lods",
            {key: frozenset(value) for key, value in self.available_lods.items()},
        )
        object.__setattr__(
            self,
            "retained_lods",
            {key: frozenset(value) for key, value in self.retained_lods.items()},
        )

    @property
    def retained_package_count(self) -> int:
        return 1 if self.retained_geometry_count > 0 else 0

    @property
    def ignored_package_count(self) -> int:
        return 1 if self.retained_geometry_count == 0 else 0


@dataclass(slots=True)
class PackageFilterSummary:
    available_types: set[str] = field(default_factory=set)
    retained_types: set[str] = field(default_factory=set)
    ignored_types: set[str] = field(default_factory=set)
    available_lods: dict[str, set[str]] = field(default_factory=dict)
    retained_lods: dict[str, set[str]] = field(default_factory=dict)
    missing_lods: dict[str, MissingLodSelection] = field(default_factory=dict)
    retained_package_count: int = 0
    ignored_package_count: int = 0

    def add(self, diagnostics: PackageFilterReport) -> None:
        self.available_types.update(diagnostics.available_types)
        self.retained_types.update(diagnostics.retained_types)
        self.ignored_types.update(diagnostics.ignored_types)
        _merge_lod_sets(self.available_lods, diagnostics.available_lods)
        _merge_lod_sets(self.retained_lods, diagnostics.retained_lods)
        for missing in diagnostics.missing_lods:
            if missing.cityobject_type not in self.missing_lods:
                self.missing_lods[missing.cityobject_type] = missing
        if diagnostics.retained_geometry_count == 0:
            self.ignored_package_count += 1
        else:
            self.retained_package_count += 1

    def requested_lod_failures(self, filter: PackageFilter) -> list[MissingLodSelection]:
        failures: list[MissingLodSelection] = []
        for cityobject_type, selection in filter.lods_by_type.items():
            if selection.kind is not _LodSelectionKind.EXACT:
                continue
            eligible = (
                cityobject_type in self.available_lods
                or cityobject_type in self.retained_types
                or filter.cityobject_types is None
                or cityobject_type in filter.cityobject_types
            )
            if not eligible:
                continue
            available_lods = self.available_lods.get(cityobject_type, set())
            if selection.exact_lod in available_lods:
                continue
            failures.append(
                MissingLodSelection(
                    cityobject_type=cityobject_type,
                    requested_lod=selection.exact_lod or "",
                    available_lods=frozenset(available_lods),
                )
            )
        return failures

    def ensure_requested_lods_available(self, filter: PackageFilter) -> None:
        failures = self.requested_lod_failures(filter)
        if not failures:
            return

        details = "; ".join(
            (
                f"{missing.cityobject_type} requested LoD '{missing.requested_lod}' "
                f"but available LoDs are: {_format_available_lods(missing.available_lods)}"
            )
            for missing in failures
        )
        raise RuntimeError(f"requested LoD selector matched no geometry: {details}")


def _merge_lod_sets(target: dict[str, set[str]], source: Mapping[str, frozenset[str]]) -> None:
    for cityobject_type, lods in source.items():
        if cityobject_type not in target:
            target[cityobject_type] = set()
        target[cityobject_type].update(lods)


def _format_available_lods(lods: frozenset[str]) -> str:
    if not lods:
        return "none"
    return ", ".join(sorted(lods))


@dataclass(frozen=True, slots=True)
class Bounds2D:
    min_x: float
    max_x: float
    min_y: float
    max_y: float


@dataclass(frozen=True, slots=True)
class Bounds3D:
    min_x: float
    max_x: float
    min_y: float
    max_y: float
    min_z: float
    max_z: float

    @classmethod
    def from_native(cls, native: _native._Bounds3D) -> Self:
        return cls(
            min_x=float(native.min_x),
            max_x=float(native.max_x),
            min_y=float(native.min_y),
            max_y=float(native.max_y),
            min_z=float(native.min_z),
            max_z=float(native.max_z),
        )


@dataclass(frozen=True, slots=True)
class FeatureBoundsSummary:
    package_count: int
    cityobject_count: int
    bounds: Bounds3D | None = None

    @classmethod
    def from_native(cls, native: _native._FeatureBoundsSummary) -> Self:
        return cls(
            package_count=int(native.package_count),
            cityobject_count=int(native.cityobject_count),
            bounds=Bounds3D.from_native(native.bounds) if native.has_bounds else None,
        )


@dataclass(frozen=True, slots=True)
class CityObjectRef:
    record_id: int
    external_id: str
    cityobject_type: str
    bounds: Bounds3D | None = None


@dataclass(frozen=True, slots=True)
class PackageRef:
    record_id: int
    model_id: str
    package_type: int = 0
    bounds: Bounds3D | None = None


@dataclass(frozen=True, slots=True)
class IndexedPackage:
    reference: PackageRef
    model: "CityModel"


@dataclass(frozen=True, slots=True)
class FilteredPackageOutcome:
    model: "CityModel | None"
    report: PackageFilterReport


@dataclass(frozen=True, slots=True)
class IndexStatus:
    exists: bool = True
    needs_reindex: bool = False
    indexed_feature_count: int = 0
    indexed_source_count: int = 0

    @classmethod
    def from_native(cls, native: _native._IndexStatus) -> Self:
        return cls(
            exists=bool(native.exists),
            needs_reindex=bool(native.needs_reindex),
            indexed_feature_count=int(native.indexed_feature_count),
            indexed_source_count=int(native.indexed_source_count),
        )


def _require_citymodel_type() -> type["CityModel"]:
    try:
        from cityjson_lib import CityModel
    except ImportError as exc:
        raise RuntimeError(
            "cityjson-index model APIs require the cityjson-lib Python package to be importable"
        ) from exc
    return CityModel


def _parse_citymodel_bytes(payload: bytes) -> "CityModel":
    try:
        from cityjson_lib import RootKind, probe_bytes
    except ImportError as exc:
        raise RuntimeError(
            "cityjson-index model APIs require the cityjson-lib Python package to be importable"
        ) from exc

    citymodel_type = _require_citymodel_type()
    probe = probe_bytes(payload)
    if probe.root_kind is RootKind.CITY_JSON_FEATURE:
        try:
            return citymodel_type.parse_feature_bytes(payload)
        except RuntimeError:
            fallback_payload = _empty_feature_as_document_bytes(payload)
            if fallback_payload is None:
                raise
            return citymodel_type.parse_document_bytes(fallback_payload)
    return citymodel_type.parse_document_bytes(payload)


def _empty_feature_as_document_bytes(payload: bytes) -> bytes | None:
    try:
        document = json.loads(payload)
    except json.JSONDecodeError:
        return None
    if not isinstance(document, dict):
        return None
    cityobjects = document.get("CityObjects")
    if (
        document.get("type") != "CityJSONFeature"
        or document.get("id") is not None
        or not isinstance(cityobjects, dict)
        or cityobjects
    ):
        return None

    document["type"] = "CityJSON"
    if "version" not in document:
        document["version"] = "2.0"
    document.pop("id", None)
    return json.dumps(document, separators=(",", ":")).encode("utf-8")


class OpenedIndex:
    def __init__(self, dataset_dir: str, index_path_override: str | None = None) -> None:
        self._dataset_dir = dataset_dir
        self._index_path_override = index_path_override
        self._handle = None

    def _require_handle(self):
        if self._handle is None:
            raise RuntimeError("OpenedIndex has already been closed or was not opened")
        return self._handle

    @classmethod
    def open(
        cls,
        dataset_dir: str | os.PathLike[str],
        index_path: str | os.PathLike[str] | None = None,
    ) -> Self:
        instance = cls(str(dataset_dir), None if index_path is None else str(index_path))
        instance._handle = _native.open_index(instance._dataset_dir, instance._index_path_override)
        return instance

    def close(self) -> None:
        if self._handle is None:
            return
        _native.close_index(self._handle)
        self._handle = None

    def __enter__(self) -> Self:
        return self

    def __exit__(self, exc_type, exc, tb) -> None:
        self.close()

    def __del__(self) -> None:
        try:
            self.close()
        except Exception:
            pass

    def status(self) -> IndexStatus:
        return IndexStatus.from_native(_native.index_status(self._require_handle()))

    def reindex(self) -> None:
        _native.reindex(self._require_handle())

    def feature_bounds_summary(self) -> FeatureBoundsSummary:
        return FeatureBoundsSummary.from_native(
            _native.feature_bounds_summary(self._require_handle())
        )

    def package_ref_page_after_record_id(
        self, after_record_id: int | None = None, limit: int = 512
    ) -> list[PackageRef]:
        return _native.package_ref_page_after_record_id(
            self._require_handle(), after_record_id, limit
        )

    def cityobject_ref_page_after_record_id(
        self, after_record_id: int | None = None, limit: int = 512
    ) -> list[CityObjectRef]:
        return _native.cityobject_ref_page_after_record_id(
            self._require_handle(), after_record_id, limit
        )

    def package_ref_bbox_page(
        self,
        bbox: Bounds2D,
        after_record_id: int | None = None,
        limit: int = 512,
    ) -> list[PackageRef]:
        return _native.query_package_refs_page(
            self._require_handle(), bbox, after_record_id, limit
        )

    def cityobject_ref_bbox_page(
        self,
        bbox: Bounds2D,
        after_record_id: int | None = None,
        limit: int = 512,
    ) -> list[CityObjectRef]:
        return _native.query_cityobject_refs_page(
            self._require_handle(), bbox, after_record_id, limit
        )

    def lookup_package_ref_by_record_id(self, record_id: int) -> PackageRef | None:
        return _native.lookup_package_ref_by_record_id(self._require_handle(), record_id)

    def lookup_cityobject_refs(self, external_id: str) -> list[CityObjectRef]:
        return _native.lookup_cityobject_refs(self._require_handle(), external_id)

    def package_refs_for_cityobject(self, ref: CityObjectRef) -> list[PackageRef]:
        return _native.package_refs_for_cityobject(self._require_handle(), ref)

    def read_package(self, ref: PackageRef) -> "CityModel":
        return _parse_citymodel_bytes(_native.read_package_model_bytes(self._require_handle(), ref))

    def read_package_by_record_id(self, record_id: int) -> IndexedPackage | None:
        ref = self.lookup_package_ref_by_record_id(record_id)
        if ref is None:
            return None
        payload = _native.read_package_by_record_id_model_bytes(
            self._require_handle(), record_id
        )
        if payload is None:
            return None
        return IndexedPackage(reference=ref, model=_parse_citymodel_bytes(payload))

    def read_packages(self, refs: list[PackageRef]) -> list[IndexedPackage]:
        payloads = _native.read_packages_model_bytes(self._require_handle(), refs)
        return [
            IndexedPackage(reference=ref, model=_parse_citymodel_bytes(payload))
            for ref, payload in zip(refs, payloads, strict=True)
        ]

    def read_cityobject_packages(self, ref: CityObjectRef) -> list[IndexedPackage]:
        return [
            IndexedPackage(reference=package_ref, model=self.read_package(package_ref))
            for package_ref in self.package_refs_for_cityobject(ref)
        ]

    def get_packages(self, external_id: str) -> list["CityModel"]:
        seen: set[int] = set()
        refs: list[PackageRef] = []
        for cityobject in self.lookup_cityobject_refs(external_id):
            for package in self.package_refs_for_cityobject(cityobject):
                if package.record_id in seen:
                    continue
                seen.add(package.record_id)
                refs.append(package)
        refs.sort(key=lambda ref: ref.record_id)
        return [package.model for package in self.read_packages(refs)]

    def read_filtered_packages(
        self,
        refs: list[PackageRef],
        filter: PackageFilter,
    ) -> list[FilteredPackageOutcome]:
        return _native.read_filtered_packages(self._require_handle(), refs, filter)



__all__ = [
    "CityObjectRef",
    "FeatureBoundsSummary",
    "Bounds3D",
    "Bounds2D",
    "FilteredPackageOutcome",
    "IndexStatus",
    "IndexedPackage",
    "LodSelection",
    "MissingLodSelection",
    "OpenedIndex",
    "PackageFilter",
    "PackageFilterReport",
    "PackageFilterSummary",
    "PackageRef",
]
