from __future__ import annotations

from dataclasses import replace

import json
import tempfile
import unittest
from pathlib import Path

from cityjson_index import (
    Bounds2D,
    FilteredPackageOutcome,
    LodSelection,
    MissingLodSelection,
    OpenedIndex,
    PackageFilter,
    PackageFilterReport,
    PackageFilterSummary,
)
from cityjson_lib import ModelType


REPO_ROOT = Path(__file__).resolve().parents[3]
CITYJSON_DATASET = REPO_ROOT / "tests" / "data" / "cityjson"


class OpenedIndexApiTests(unittest.TestCase):
    def test_cityjson_get_packages_and_read_package_return_actionable_payloads(self) -> None:
        """Input: a regular CityJSON dataset indexed through the Python API.
        Assertions: package lookup by CityObject id and direct package read return valid CityJSONFeature models and obsolete singular conveniences are absent.
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            index_path = Path(tmpdir) / ".cityjson_index.sqlite"
            with OpenedIndex.open(CITYJSON_DATASET, index_path) as index:
                index.reindex()
                cityobject = index.lookup_cityobject_refs("fixture-a")[0]
                refs = index.package_refs_for_cityobject(cityobject)

                self.assertTrue(refs)
                ref = refs[0]

                by_id = index.get_packages(cityobject.external_id)[0]
                by_ref = index.read_package(ref)
                self.assertEqual(by_id.summary().model_type, ModelType.CITY_JSON_FEATURE)
                self.assertEqual(by_ref.summary().model_type, ModelType.CITY_JSON_FEATURE)
                self.assertTrue(by_id.summary().has_transform)
                self.assertTrue(by_ref.summary().has_transform)
                self.assertIn(cityobject.external_id, by_id.cityobject_ids())
                self.assertFalse(hasattr(index, "get"))
                self.assertFalse(hasattr(index, "get_json"))
                self.assertFalse(hasattr(index, "feature_ref_page"))

    def test_summary_record_id_pagination_bbox_pages_and_batch_reads(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            index_path = Path(tmpdir) / ".cityjson_index.sqlite"
            with OpenedIndex.open(CITYJSON_DATASET, index_path) as index:
                index.reindex()

                summary = index.feature_bounds_summary()
                self.assertGreater(summary.package_count, 0)
                self.assertGreater(summary.cityobject_count, 0)
                self.assertIsNotNone(summary.bounds)

                package_page = index.package_ref_page_after_record_id(None, 1)
                self.assertEqual(len(package_page), 1)
                first_package = package_page[0]
                self.assertEqual(
                    index.package_ref_page_after_record_id(first_package.record_id, 1)[0].record_id,
                    first_package.record_id + 1,
                )
                self.assertEqual(
                    index.lookup_package_ref_by_record_id(first_package.record_id),
                    first_package,
                )
                self.assertIsNotNone(index.read_package_by_record_id(first_package.record_id))

                packages = index.read_packages([first_package, first_package])
                self.assertEqual([item.reference for item in packages], [first_package, first_package])
                self.assertTrue(all(item.model.summary().model_type is ModelType.CITY_JSON_FEATURE for item in packages))

                cityobject_page = index.cityobject_ref_page_after_record_id(None, 1)
                self.assertEqual(len(cityobject_page), 1)
                self.assertIsNotNone(cityobject_page[0].bounds)

                bounds = first_package.bounds or summary.bounds
                self.assertIsNotNone(bounds)
                bbox = Bounds2D(bounds.min_x, bounds.max_x, bounds.min_y, bounds.max_y)
                self.assertTrue(index.package_ref_bbox_page(bbox, None, 10))
                self.assertTrue(index.cityobject_ref_bbox_page(bbox, None, 10))

    def test_package_source_paths_preserve_order_and_report_missing_records(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            index_path = Path(tmpdir) / ".cityjson_index.sqlite"
            with OpenedIndex.open(CITYJSON_DATASET, index_path) as index:
                index.reindex()
                first = index.package_ref_page_after_record_id(None, 1)[0]

                self.assertEqual(index.package_source_paths([]), [])
                source_paths = index.package_source_paths([first, first])
                self.assertEqual(len(source_paths), 2)
                expected_path = CITYJSON_DATASET / "a" / "fixtures.city.json"
                for source_path in source_paths:
                    # Rust canonical paths may use Windows extended-length syntax.
                    self.assertTrue(Path(source_path).samefile(expected_path))
                with self.assertRaisesRegex(RuntimeError, str(2**63 - 1)):
                    index.package_source_paths([replace(first, record_id=2**63 - 1)])

    def test_read_filtered_packages_reports_package_reports(self) -> None:
        """Input: two package refs filtered for Building geometry at the highest LoD.
        Assertions: one outcome is returned per package, each model remains a CityJSONFeature, and the report records retained Building LoD 1.0.
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            index_path = Path(tmpdir) / ".cityjson_index.sqlite"
            with OpenedIndex.open(CITYJSON_DATASET, index_path) as index:
                index.reindex()
                cityobjects = index.lookup_cityobject_refs("fixture-a") + index.lookup_cityobject_refs("fixture-b")
                refs = [index.package_refs_for_cityobject(cityobject)[0] for cityobject in cityobjects]

                filter = PackageFilter(
                    cityobject_types={"Building"},
                    default_lod=LodSelection.HIGHEST,
                )
                filtered = index.read_filtered_packages(refs, filter)

                self.assertEqual(len(filtered), len(refs))
                self.assertTrue(filtered)
                self.assertEqual(filtered[0].model.summary().model_type, ModelType.CITY_JSON_FEATURE)
                self.assertIn("Building", filtered[0].report.available_types)
                self.assertIn("Building", filtered[0].report.retained_types)
                self.assertEqual(filtered[0].report.retained_lods["Building"], {"1.0"})

    def test_multiple_exact_lods_retain_geometry_and_report_partial_missing(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            document = json.loads((CITYJSON_DATASET / "a" / "fixtures.city.json").read_text(encoding="utf-8"))
            for cityobject in document["CityObjects"].values():
                geometry = cityobject["geometry"][0]
                cityobject["geometry"] = [dict(geometry, lod=lod) for lod in ["1.0", "2.0", "2.2"]]
            (root / "fixture.city.json").write_text(json.dumps(document), encoding="utf-8")
            with OpenedIndex.open(root, root / "index.sqlite") as index:
                index.reindex()
                refs = index.package_refs_for_cityobject(index.lookup_cityobject_refs("fixture-a")[0])
                for values in [["2.0"], ["1.0", "2.2"], ["1.0", "2.2", "8.8", "9.9"]]:
                    filter = PackageFilter(lods_by_type={"Building": LodSelection.Exact(values)})
                    result = index.read_filtered_packages(refs, filter)[0]
                    expected = set(values) & {"1.0", "2.0", "2.2"}
                    self.assertEqual(result.report.retained_lods["Building"], expected)
                    self.assertEqual(result.report.retained_geometry_count, len(expected))
                    self.assertIsNotNone(result.model)
                    summary = PackageFilterSummary()
                    summary.add(result.report)
                    if "8.8" in values:
                        self.assertEqual(summary.missing_lods["Building"].requested_lods, {"8.8", "9.9"})
                        with self.assertRaisesRegex(RuntimeError, "8.8.*9.9.*1.0.*2.0.*2.2"):
                            summary.ensure_requested_lods_available(filter)
                    else:
                        summary.ensure_requested_lods_available(filter)

    def test_exact_selection_requires_nonempty_sequence(self) -> None:
        for factory in [LodSelection.Exact, LodSelection.exact]:
            with self.assertRaisesRegex(ValueError, "must not be empty"):
                factory([])
            with self.assertRaisesRegex(TypeError, "sequence"):
                factory("2.0")
            self.assertEqual(factory(("1.0", "2.2")).exact_lods, ("1.0", "2.2"))

    def test_summary_merges_missing_lods_against_combined_availability(self) -> None:
        reports = [
            PackageFilterReport(
                retained_types={"Building"}, available_lods={"Building": {available}},
                missing_lods=[MissingLodSelection("Building", frozenset(missing), frozenset({available}))],
            )
            for available, missing in [("1.0", {"2.2", "8.8"}), ("2.2", {"1.0", "9.9"})]
        ]
        for ordered in [reports, list(reversed(reports))]:
            summary = PackageFilterSummary()
            for report in ordered:
                summary.add(report)
            self.assertEqual(summary.missing_lods["Building"].requested_lods, {"8.8", "9.9"})
            self.assertEqual(summary.missing_lods["Building"].available_lods, {"1.0", "2.2"})
            summary.ensure_requested_lods_available(PackageFilter(default_lod=LodSelection.Exact(["1.0", "2.2"])))

    def test_summary_validates_missing_type_overrides_and_clears_found_values(self) -> None:
        filter = PackageFilter(
            cityobject_types={"Building"},
            lods_by_type={"Road": LodSelection.Exact(["8.8", "9.9"])},
        )
        failures = PackageFilterSummary().requested_lod_failures(filter)
        self.assertEqual(failures[0].requested_lods, {"8.8", "9.9"})
        self.assertEqual(failures[0].available_lods, set())
        summary = PackageFilterSummary()
        summary.add(PackageFilterReport(
            available_lods={"Building": {"1.0"}},
            missing_lods=[MissingLodSelection("Building", frozenset({"2.2"}), frozenset({"1.0"}))],
        ))
        summary.add(PackageFilterReport(available_lods={"Building": {"2.2"}}))
        self.assertFalse(summary.missing_lods)

    def test_package_filter_summary_reports_missing_requested_lods(self) -> None:
        """Input: package refs filtered for a Building LoD that is unavailable in the dataset.
        Assertions: the summary reports no retained packages, all packages ignored, and the missing LoD failure can be raised.
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            index_path = Path(tmpdir) / ".cityjson_index.sqlite"
            with OpenedIndex.open(CITYJSON_DATASET, index_path) as index:
                index.reindex()
                cityobjects = index.lookup_cityobject_refs("fixture-a") + index.lookup_cityobject_refs("fixture-b")
                refs = [index.package_refs_for_cityobject(cityobject)[0] for cityobject in cityobjects]

                filter = PackageFilter(
                    cityobject_types={"Building"},
                    default_lod=LodSelection.HIGHEST,
                    lods_by_type={"Building": LodSelection.Exact(["2.0", "2.2"])},
                )
                filtered = index.read_filtered_packages(refs, filter)
                summary = PackageFilterSummary()
                for package in filtered:
                    summary.add(package.report)

                self.assertEqual(summary.available_lods["Building"], {"1.0"})
                self.assertEqual(summary.retained_package_count, 0)
                self.assertEqual(summary.ignored_package_count, len(refs))

                failures = summary.requested_lod_failures(filter)
                self.assertEqual(len(failures), 1)
                self.assertEqual(failures[0].cityobject_type, "Building")
                self.assertEqual(failures[0].requested_lods, {"2.0", "2.2"})
                self.assertEqual(failures[0].available_lods, {"1.0"})
                self.assertEqual(filtered[0].report.missing_lods, failures)

                with self.assertRaisesRegex(RuntimeError, "requested LoD selector matched no geometry"):
                    summary.ensure_requested_lods_available(filter)


def _write_shared_child_cityjson(root: Path) -> None:
    document = {
        "type": "CityJSON",
        "version": "2.0",
        "transform": {"scale": [1.0, 1.0, 1.0], "translate": [0.0, 0.0, 0.0]},
        "CityObjects": {
            "building-a": {"type": "Building", "children": ["shared-part"]},
            "building-b": {"type": "Building", "children": ["shared-part"]},
            "shared-part": {
                "type": "BuildingPart",
                "parents": ["building-a", "building-b"],
                "geometry": [
                    {"type": "MultiSurface", "lod": "1.0", "boundaries": [[[0, 1, 2]]]}
                ],
            },
        },
        "vertices": [[0, 0, 0], [1, 0, 1], [0, 1, 2]],
    }
    (root / "shared-child.city.json").write_text(json.dumps(document), encoding="utf-8")


def _filter_hierarchy(
    lod: str | None, filter: PackageFilter, parent_lod: str | None = None
) -> list[FilteredPackageOutcome]:
    with tempfile.TemporaryDirectory() as tmpdir:
        root = Path(tmpdir)
        _write_shared_child_cityjson(root)
        path = root / "shared-child.city.json"
        document = json.loads(path.read_text(encoding="utf-8"))
        geometry = document["CityObjects"]["shared-part"]["geometry"][0]
        if lod is None:
            del geometry["lod"]
        else:
            geometry["lod"] = lod
        for parent_id in ["building-a", "building-b"]:
            document["CityObjects"][parent_id]["geometry"] = (
                [dict(geometry, lod=parent_lod)] if parent_lod is not None else []
            )
        path.write_text(json.dumps(document), encoding="utf-8")
        with OpenedIndex.open(root, root / "index.sqlite") as index:
            index.reindex()
            child = index.lookup_cityobject_refs("shared-part")[0]
            refs = index.package_refs_for_cityobject(child)
            return index.read_filtered_packages(refs, filter)


class OpenedIndexPluralPackageApiTests(unittest.TestCase):
    def test_default_exact_lod_preserves_structural_ancestors(self) -> None:
        for cityobject_types in [None, {"Building"}]:
            filter = PackageFilter(
                cityobject_types=cityobject_types, default_lod=LodSelection.Exact(["1.0"])
            )
            summary = PackageFilterSummary()
            for result in _filter_hierarchy("1.0", filter):
                self.assertIn("Building", result.report.retained_types)
                self.assertNotIn("Building", result.report.available_lods)
                self.assertFalse(result.report.missing_lods)
                summary.add(result.report)
                self.assertIsNotNone(result.model)
                objects = json.loads(result.model.to_json_bytes())["CityObjects"]
                parent_id = next(key for key, value in objects.items() if value["type"] == "Building")
                self.assertEqual(objects[parent_id]["children"], ["shared-part"])
                self.assertEqual(objects["shared-part"]["parents"], [parent_id])
                self.assertEqual(objects["shared-part"]["geometry"][0]["lod"], "1.0")
            summary.ensure_requested_lods_available(filter)

    def test_default_exact_lod_validates_geometry_owners_and_overrides(self) -> None:
        filter = PackageFilter(default_lod=LodSelection.Exact(["1.0", "8.8", "9.9"]))
        result = _filter_hierarchy("1.0", filter)[0]
        self.assertEqual(len(result.report.missing_lods), 1)
        missing = result.report.missing_lods[0]
        self.assertEqual(missing.cityobject_type, "BuildingPart")
        self.assertEqual(missing.requested_lods, {"8.8", "9.9"})
        self.assertEqual(missing.available_lods, {"1.0"})
        summary = PackageFilterSummary()
        summary.add(result.report)
        with self.assertRaisesRegex(RuntimeError, "BuildingPart.*8.8.*9.9.*1.0"):
            summary.ensure_requested_lods_available(filter)
        for requested_type in ["Building", "Road"]:
            explicit = PackageFilter(lods_by_type={requested_type: LodSelection.Exact(["1.0"])})
            result = _filter_hierarchy("1.0", explicit)[0]
            summary = PackageFilterSummary()
            summary.add(result.report)
            with self.assertRaisesRegex(RuntimeError, requested_type):
                summary.ensure_requested_lods_available(explicit)
            if requested_type == "Building":
                self.assertEqual(result.report.missing_lods[0].cityobject_type, "Building")
        single = PackageFilter(default_lod=LodSelection.Exact(["1.0"]))
        parent_report = _filter_hierarchy("1.0", single, parent_lod="2.0")[0].report
        summary = PackageFilterSummary()
        summary.add(parent_report)
        with self.assertRaisesRegex(RuntimeError, "Building requested"):
            summary.ensure_requested_lods_available(single)
        no_lod = _filter_hierarchy(None, single)[0]
        self.assertIsNone(no_lod.model)
        self.assertEqual(no_lod.report.available_lods, {"BuildingPart": frozenset()})
        self.assertEqual(no_lod.report.missing_lods[0].requested_lods, {"1.0"})
        summary = PackageFilterSummary()
        summary.add(no_lod.report)
        self.assertEqual(summary.available_lods, {"BuildingPart": set()})
        with self.assertRaisesRegex(RuntimeError, "BuildingPart"):
            summary.ensure_requested_lods_available(single)

    def test_default_exact_lod_merges_hierarchy_availability(self) -> None:
        for values in [["1.0", "2.0"], ["1.0", "2.0", "8.8", "9.9"]]:
            filter = PackageFilter(default_lod=LodSelection.Exact(values))
            reports = [_filter_hierarchy(lod, filter)[0].report for lod in ["1.0", "2.0"]]
            for ordered in [reports, list(reversed(reports))]:
                summary = PackageFilterSummary()
                for report in ordered:
                    summary.add(report)
                self.assertNotIn("Building", summary.missing_lods)
                self.assertEqual(summary.available_lods["BuildingPart"], {"1.0", "2.0"})
                if len(values) == 2:
                    self.assertFalse(summary.missing_lods)
                    summary.ensure_requested_lods_available(filter)
                else:
                    self.assertEqual(summary.missing_lods["BuildingPart"].requested_lods, {"8.8", "9.9"})
                    with self.assertRaisesRegex(RuntimeError, "8.8.*9.9"):
                        summary.ensure_requested_lods_available(filter)

    def test_python_get_packages_returns_all_distinct_packages(self) -> None:
        """Input: a temporary CityJSON dataset with two root Buildings sharing one BuildingPart child.
        Assertions: get_packages returns both containing packages and obsolete singular conveniences are absent.
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir) / "dataset"
            root.mkdir()
            _write_shared_child_cityjson(root)
            with OpenedIndex.open(root, Path(tmpdir) / ".cityjson-index.sqlite") as index:
                index.reindex()
                packages = index.get_packages("shared-part")

                self.assertEqual(len(packages), 2)
                self.assertFalse(hasattr(index, "get"))
                self.assertFalse(hasattr(index, "get_json"))
                self.assertFalse(hasattr(index, "lookup_cityobject_ref"))

    def test_python_filtered_packages_preserve_alignment_and_none_models(self) -> None:
        """Input: package refs for a shared child filtered with a non-matching WaterBody type.
        Assertions: one outcome is returned per input ref, every model is None, and each report counts one ignored package.
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir) / "dataset"
            root.mkdir()
            _write_shared_child_cityjson(root)
            with OpenedIndex.open(root, Path(tmpdir) / ".cityjson-index.sqlite") as index:
                index.reindex()
                cityobject = index.lookup_cityobject_refs("shared-part")[0]
                refs = index.package_refs_for_cityobject(cityobject)
                outcomes = index.read_filtered_packages(
                    refs,
                    PackageFilter(cityobject_types={"WaterBody"}),
                )

                self.assertEqual(len(outcomes), len(refs))
                self.assertTrue(outcomes)
                self.assertTrue(all(outcome.model is None for outcome in outcomes))
                self.assertTrue(all(outcome.report.ignored_package_count == 1 for outcome in outcomes))


if __name__ == "__main__":
    unittest.main()
