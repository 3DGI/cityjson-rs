from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from cityjson_index import FeatureFilter, FeatureFilterSummary, LodSelection, OpenedIndex
from cityjson_lib import ModelType


REPO_ROOT = Path(__file__).resolve().parents[3]
CITYJSON_DATASET = REPO_ROOT / "tests" / "data" / "cityjson"


class OpenedIndexApiTests(unittest.TestCase):
    def test_cityjson_get_and_read_feature_return_actionable_feature_payloads(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            index_path = Path(tmpdir) / ".cityjson_index.sqlite"
            with OpenedIndex.open(CITYJSON_DATASET, index_path) as index:
                index.reindex()
                refs = index.feature_ref_page(0, 1)

                self.assertTrue(refs)
                ref = refs[0]

                by_id = index.get(ref.feature_id)
                self.assertIsNotNone(by_id)
                by_ref = index.read_feature(ref)
                self.assertEqual(by_id.summary().model_type, ModelType.CITY_JSON_FEATURE)
                self.assertEqual(by_ref.summary().model_type, ModelType.CITY_JSON_FEATURE)
                self.assertTrue(by_id.summary().has_transform)
                self.assertTrue(by_ref.summary().has_transform)
                self.assertIn(ref.feature_id, by_id.cityobject_ids())

                by_id_payload = index.get_json(ref.feature_id)
                by_ref_payload = index.read_feature_json(ref)

                self.assertEqual(by_id_payload["type"], "CityJSONFeature")
                self.assertEqual(by_ref_payload["type"], "CityJSONFeature")
                self.assertIn("transform", by_id_payload)
                self.assertIn("transform", by_ref_payload)
                self.assertIn("metadata", by_id_payload)
                self.assertIn("metadata", by_ref_payload)
                self.assertIn(ref.feature_id, by_id_payload["CityObjects"])
                self.assertEqual(by_id_payload, by_ref_payload)

    def test_read_filtered_features_reports_diagnostics(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            index_path = Path(tmpdir) / ".cityjson_index.sqlite"
            with OpenedIndex.open(CITYJSON_DATASET, index_path) as index:
                index.reindex()
                refs = index.feature_ref_page(0, 2)

                filter = FeatureFilter(
                    cityobject_types={"Building"},
                    default_lod=LodSelection.HIGHEST,
                )
                filtered = index.read_filtered_features(refs, filter)

                self.assertEqual(len(filtered), len(refs))
                self.assertTrue(filtered)
                self.assertEqual(filtered[0].model.summary().model_type, ModelType.CITY_JSON_FEATURE)
                self.assertIn("Building", filtered[0].diagnostics.available_types)
                self.assertIn("Building", filtered[0].diagnostics.retained_types)
                self.assertEqual(filtered[0].diagnostics.retained_lods["Building"], {"1.0"})

                single = index.read_filtered_feature(refs[0], filter)
                self.assertEqual(single.model.cityobject_ids(), filtered[0].model.cityobject_ids())
                self.assertEqual(single.diagnostics, filtered[0].diagnostics)

    def test_filter_summary_reports_missing_requested_lods(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            index_path = Path(tmpdir) / ".cityjson_index.sqlite"
            with OpenedIndex.open(CITYJSON_DATASET, index_path) as index:
                index.reindex()
                refs = index.feature_ref_page(0, 2)

                filter = FeatureFilter(
                    cityobject_types={"Building"},
                    default_lod=LodSelection.HIGHEST,
                    lods_by_type={"Building": LodSelection.Exact("2.0")},
                )
                filtered = index.read_filtered_features(refs, filter)
                summary = FeatureFilterSummary()
                for feature in filtered:
                    summary.add(feature.diagnostics)

                self.assertEqual(summary.available_lods["Building"], {"1.0"})
                self.assertEqual(summary.retained_feature_count, 0)
                self.assertEqual(summary.ignored_feature_count, len(refs))

                failures = summary.requested_lod_failures(filter)
                self.assertEqual(len(failures), 1)
                self.assertEqual(failures[0].cityobject_type, "Building")
                self.assertEqual(failures[0].requested_lod, "2.0")
                self.assertEqual(failures[0].available_lods, {"1.0"})
                self.assertEqual(filtered[0].diagnostics.missing_lods, failures)

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


class OpenedIndexPluralPackageApiTests(unittest.TestCase):
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
                    FeatureFilter(cityobject_types={"WaterBody"}),
                )

                self.assertEqual(len(outcomes), len(refs))
                self.assertTrue(outcomes)
                self.assertTrue(all(outcome.model is None for outcome in outcomes))
                self.assertTrue(all(outcome.report.ignored_package_count == 1 for outcome in outcomes))


if __name__ == "__main__":
    unittest.main()
