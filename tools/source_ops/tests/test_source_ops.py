# Where: tools/source_ops/tests/test_source_ops.py
# What: Unit and integration coverage for source_ops registry, normalization, diffing, smoke, and orchestration.
# Why: Keep the automation contract stable before wiring it to daily Codex runs and real canisters.
from __future__ import annotations

import json
import os
import tempfile
import unittest
from io import StringIO
from pathlib import Path
from unittest.mock import patch

from tools.source_ops import apply_catalog, diff, kinic_writer, normalize, registry, run_refresh, smoke, validate
from tools.source_ops.common import dump_json, write_text
from tools.source_ops.config import load_settings


class SourceOpsTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.raw_dir = self.root / "raw"
        self.normalized_dir = self.root / "normalized"
        self.reports_dir = self.root / "reports"
        self.state_path = self.root / "state" / "manifest.json"
        dump_json(self.state_path, {"last_run_at": None, "sources": {}})
        self.fixture = self.root / "next.html"
        write_text(
            self.fixture,
            "<html><head><title>Next.js Middleware</title></head><body>Use middleware to inspect cookies and redirect unauthenticated users.</body></html>",
        )

    def _settings(self):
        os.environ["SOURCE_OPS_HTTP_TIMEOUT"] = "5"
        os.environ["SOURCE_OPS_MEMORY_WRITER_TEMPLATE"] = "python3 -c \"print('write ok')\""
        os.environ["SOURCE_OPS_CLI_BIN"] = f"python3 {self._cli_stub()}"
        settings = load_settings()
        return settings

    def _cli_stub(self) -> Path:
        path = self.root / "fake_cli.py"
        write_text(
            path,
            "\n".join(
                [
                    "import json, sys",
                    "cmd = sys.argv[1]",
                    "if cmd == 'resolve':",
                    "    print(json.dumps({'candidate_sources':[{'source_id':'/vercel/next.js'}]}))",
                    "elif cmd == 'query':",
                    "    print(json.dumps({'source_id':'/vercel/next.js','snippets':[{'citation':'https://nextjs.org/docs','title':'t','snippet':'s'}]}))",
                    "else:",
                    "    print(json.dumps({'resolved_sources':['/vercel/next.js'],'evidence':[{'citation':'https://nextjs.org/docs'}]}))",
                ]
            )
            + "\n",
        )
        return path

    def _source(self):
        return {
            "source_id": "/vercel/next.js",
            "kind": "docs",
            "enabled": True,
            "public_urls": [{"label": "middleware", "url": self.fixture.as_uri()}],
            "discovery_urls": [],
            "normalization_profile": "docs_html",
            "catalog_metadata": {
                "title": "Next.js Docs",
                "aliases": ["next", "middleware"],
                "domain": "code_docs",
                "trust": "official",
                "skill_kind": None,
                "targets": [],
                "capabilities": [],
                "supported_versions": ["15"],
                "citations": ["https://nextjs.org/docs"],
                "retrieved_at": "2026-03-18T00:00:00Z",
            },
            "memory_targets": {
                "staging_canister_ids": ["aaaaa-aa"],
                "prod_canister_ids": ["bbbbb-bb"],
            },
            "cadence": "daily",
            "version_strategy": "latest_supported_version",
            "extraction_hints": {
                "content_roots": ["main", "article"],
                "drop_selectors": ["nav", "header", "footer", "aside"],
                "chunk_target_chars": 900,
            },
            "smoke_queries": {
                "resolve": "next middleware",
                "query": "middleware cookies",
                "pack": "protect route in next.js with supabase auth",
            },
        }

    def test_registry_file_has_required_fields(self) -> None:
        errors = registry.validate_registry(registry.load_registry(load_settings()))
        self.assertEqual(errors, [])

    def test_normalize_and_validate_fixture_source(self) -> None:
        source = self._source()
        raw_path = self.raw_dir / "vercel__next_js"
        raw_path.mkdir(parents=True)
        dump_json(
            raw_path / "latest.json",
            {"source_id": source["source_id"], "collected_at": "2026-03-18T00:00:00Z", "items": [{"url": self.fixture.as_uri(), "final_url": self.fixture.as_uri(), "label": "middleware", "role": "public_urls", "body": self.fixture.read_text(), "content_type": "text/html", "sha256": "x", "status_code": 200, "etag": None, "last_modified": None}]},
        )
        rows = normalize.normalize_source(source, self.raw_dir, self.normalized_dir)
        self.assertEqual(rows[0]["source_id"], source["source_id"])
        self.assertGreater(len(rows), 0)
        self.assertEqual(validate.validate_source(source, self.normalized_dir), [])

    def test_normalize_splits_large_sections_into_multiple_chunks(self) -> None:
        source = self._source()
        big_body = (
            "<html><head><title>Next.js Middleware</title></head><body>"
            "<h1 id='middleware'>Middleware</h1>"
            + "<p>" + ("Sentence. " * 250) + "</p>"
            + "</body></html>"
        )
        raw_path = self.raw_dir / "vercel__next_js"
        raw_path.mkdir(parents=True)
        dump_json(
            raw_path / "latest.json",
            {"source_id": source["source_id"], "collected_at": "2026-03-18T00:00:00Z", "items": [{"url": self.fixture.as_uri(), "final_url": self.fixture.as_uri(), "label": "middleware", "role": "public_urls", "body": big_body, "content_type": "text/html", "sha256": "x", "status_code": 200, "etag": None, "last_modified": None}]},
        )
        rows = normalize.normalize_source(source, self.raw_dir, self.normalized_dir)
        self.assertGreater(len(rows), 1)
        self.assertTrue(any("#middleware" in row["citation"] for row in rows))

    def test_normalize_fails_on_suspicious_html_extraction(self) -> None:
        source = self._source()
        raw_path = self.raw_dir / "vercel__next_js"
        raw_path.mkdir(parents=True)
        dump_json(
            raw_path / "latest.json",
            {"source_id": source["source_id"], "collected_at": "2026-03-18T00:00:00Z", "items": [{"url": self.fixture.as_uri(), "final_url": self.fixture.as_uri(), "label": "middleware", "role": "public_urls", "body": "<html><body><script>1</script></body></html>" + (" " * 3000), "content_type": "text/html", "sha256": "x", "status_code": 200, "etag": None, "last_modified": None}]},
        )
        with self.assertRaises(ValueError):
            normalize.normalize_source(source, self.raw_dir, self.normalized_dir)

    def test_normalize_ignores_discovery_urls(self) -> None:
        source = self._source()
        raw_path = self.raw_dir / "vercel__next_js"
        raw_path.mkdir(parents=True)
        dump_json(
            raw_path / "latest.json",
            {
                "source_id": source["source_id"],
                "collected_at": "2026-03-18T00:00:00Z",
                "items": [
                    {"url": self.fixture.as_uri(), "final_url": self.fixture.as_uri(), "label": "middleware", "role": "public_urls", "body": self.fixture.read_text(), "content_type": "text/html", "sha256": "x", "status_code": 200, "etag": None, "last_modified": None},
                    {"url": "https://nextjs.org/docs", "final_url": "https://nextjs.org/docs", "label": "docs-index", "role": "discovery_urls", "body": "<html><body>index</body></html>", "content_type": "text/html", "sha256": "y", "status_code": 200, "etag": "1", "last_modified": "yesterday"},
                ],
            },
        )
        rows = normalize.normalize_source(source, self.raw_dir, self.normalized_dir)
        self.assertTrue(all("docs-index" not in row["section"] for row in rows))

    def test_normalize_records_extraction_warning_when_content_root_misses(self) -> None:
        source = self._source()
        source["extraction_hints"]["content_roots"] = ["main"]
        raw_path = self.raw_dir / "vercel__next_js"
        raw_path.mkdir(parents=True)
        dump_json(
            raw_path / "latest.json",
            {
                "source_id": source["source_id"],
                "collected_at": "2026-03-18T00:00:00Z",
                "items": [
                    {"url": self.fixture.as_uri(), "final_url": "https://docs.example/redirected", "label": "middleware", "role": "public_urls", "body": "<html><body><article><h1>Middleware</h1><p>" + ("Useful text. " * 20) + "</p></article></body></html>", "content_type": "text/html", "sha256": "x", "status_code": 200, "etag": None, "last_modified": None},
                ],
            },
        )
        rows = normalize.normalize_source(source, self.raw_dir, self.normalized_dir)
        self.assertTrue(all(row["citation"].startswith("https://docs.example/redirected") for row in rows))
        meta = normalize.load_normalization_meta(self.normalized_dir, source["source_id"])
        self.assertGreater(meta["warning_count"], 0)
        self.assertTrue(any(item["kind"] == "content_roots_unmatched" for item in meta["warnings"]))

    def test_diff_detects_changes_and_thresholds(self) -> None:
        source = self._source()
        settings = self._settings()
        payloads = [{"citation": "https://nextjs.org/docs", "source_id": source["source_id"], "title": "A", "snippet": "B", "version": "15"}]
        previous = {"record_hashes": {}, "metadata_hash": "old"}
        result = diff.compute_diff(source, payloads, previous, settings)
        self.assertEqual(result["status"], "new")
        self.assertTrue(result["metadata_changed"])

    def test_diff_counts_each_chunk_with_shared_citation(self) -> None:
        source = self._source()
        settings = self._settings()
        payloads = [
            {
                "citation": "https://nextjs.org/docs#middleware",
                "source_id": source["source_id"],
                "title": "A",
                "snippet": "B",
                "content": "chunk one",
                "version": "15",
                "section_index": 0,
                "chunk_index": 0,
            },
            {
                "citation": "https://nextjs.org/docs#middleware",
                "source_id": source["source_id"],
                "title": "A",
                "snippet": "C",
                "content": "chunk two",
                "version": "15",
                "section_index": 0,
                "chunk_index": 1,
            },
        ]
        previous = {"record_hashes": {}, "metadata_hash": "old"}
        result = diff.compute_diff(source, payloads, previous, settings)
        self.assertEqual(result["added_records"], 2)
        self.assertEqual(len(result["record_hashes"]), 2)

    def test_default_writer_template_points_to_standard_runner(self) -> None:
        settings = load_settings()
        self.assertIn("kinic_writer.py", settings.memory_writer_template)
        self.assertEqual(settings.memory_reset_dim, 1024)

    def test_apply_catalog_builds_upsert_payload(self) -> None:
        source = self._source()
        payload = apply_catalog.build_upsert_args(source, "staging")
        self.assertIn('source_id = "/vercel/next.js"', payload)
        self.assertIn('canister_ids = vec {"aaaaa-aa"}', payload)

    def test_kinic_writer_builds_embedding_text_and_insert_args(self) -> None:
        payload = {
            "title": "Next.js Middleware",
            "snippet": "Use middleware to inspect cookies.",
            "content": "Full chunk text",
        }
        text = kinic_writer.embedding_input_text(payload)
        self.assertIn("Next.js Middleware", text)
        args = kinic_writer._format_insert_args(payload, [0.1, 0.2])
        self.assertIn("float32", args)
        self.assertIn('\\"title\\"', args)

    def test_smoke_uses_cli_contract(self) -> None:
        source = self._source()
        settings = self._settings()
        settings = settings.__class__(**{**settings.__dict__, "staging_catalog_canister_id": "t63gs-up777-77776-aaaba-cai"})
        report = smoke.smoke_source(source, settings, "staging", False)
        self.assertEqual(report["status"], "ok")

    def test_run_refresh_dry_run_writes_report(self) -> None:
        source = self._source()
        registry_path = self.root / "registry.yaml"
        write_text(registry_path, json.dumps([source], indent=2))
        os.environ["SOURCE_OPS_MEMORY_WRITER_TEMPLATE"] = "python3 -c \"print('write ok')\""
        os.environ["SOURCE_OPS_CLI_BIN"] = f"python3 {self._cli_stub()}"
        os.environ["SOURCE_OPS_STAGING_CATALOG_CANISTER_ID"] = "t63gs-up777-77776-aaaba-cai"
        settings = load_settings()
        settings = settings.__class__(
            **{
                **settings.__dict__,
                "registry_path": registry_path,
                "raw_dir": self.raw_dir,
                "normalized_dir": self.normalized_dir,
                "reports_dir": self.reports_dir,
                "state_path": self.state_path,
            }
        )
        report = run_refresh.run_refresh(settings, source_id=source["source_id"], dry_run=True)
        self.assertIn(report["status"], {"ok", "partial"})
        self.assertTrue(any(self.reports_dir.iterdir()))

    def test_run_refresh_marks_needs_review_when_extraction_warning_exists(self) -> None:
        source = self._source()
        source["extraction_hints"]["content_roots"] = ["main"]
        registry_path = self.root / "registry.yaml"
        write_text(registry_path, json.dumps([source], indent=2))
        long_html = "<html><body><article><h1>Middleware</h1><p>" + ("Useful text. " * 30) + "</p></article></body></html>"
        raw_path = self.raw_dir / "vercel__next_js"
        raw_path.mkdir(parents=True)
        dump_json(
            raw_path / "latest.json",
            {
                "source_id": source["source_id"],
                "collected_at": "2026-03-18T00:00:00Z",
                "items": [
                    {"url": self.fixture.as_uri(), "final_url": self.fixture.as_uri(), "label": "middleware", "role": "public_urls", "body": long_html, "content_type": "text/html", "sha256": "x", "status_code": 200, "etag": None, "last_modified": None},
                ],
            },
        )
        os.environ["SOURCE_OPS_MEMORY_WRITER_TEMPLATE"] = "python3 -c \"print('write ok')\""
        os.environ["SOURCE_OPS_CLI_BIN"] = f"python3 {self._cli_stub()}"
        os.environ["SOURCE_OPS_STAGING_CATALOG_CANISTER_ID"] = "t63gs-up777-77776-aaaba-cai"
        settings = load_settings()
        settings = settings.__class__(
            **{
                **settings.__dict__,
                "registry_path": registry_path,
                "raw_dir": self.raw_dir,
                "normalized_dir": self.normalized_dir,
                "reports_dir": self.reports_dir,
                "state_path": self.state_path,
            }
        )
        with patch.object(run_refresh, "collect_source", return_value={}), patch.object(
            run_refresh, "apply_memory", return_value={"status": "ok"}
        ), patch.object(run_refresh, "apply_catalog", return_value={"status": "ok"}), patch.object(
            run_refresh, "smoke_source", return_value={"status": "ok"}
        ):
            report = run_refresh.run_refresh(settings, source_id=source["source_id"], dry_run=False)
        self.assertEqual(report["sources"][0]["status"], "needs_review")
        self.assertIn("warnings", report["sources"][0])

    def test_run_refresh_noop_keeps_previous_success_snapshot(self) -> None:
        source = self._source()
        registry_path = self.root / "registry.yaml"
        write_text(registry_path, json.dumps([source], indent=2))
        raw_path = self.raw_dir / "vercel__next_js"
        raw_path.mkdir(parents=True)
        dump_json(
            raw_path / "latest.json",
            {
                "source_id": source["source_id"],
                "collected_at": "2026-03-18T00:00:00Z",
                "items": [
                    {
                        "url": self.fixture.as_uri(),
                        "final_url": self.fixture.as_uri(),
                        "label": "middleware",
                        "role": "public_urls",
                        "body": self.fixture.read_text(),
                        "content_type": "text/html",
                        "sha256": "x",
                        "status_code": 200,
                        "etag": None,
                        "last_modified": None,
                    }
                ],
            },
        )
        previous_snapshot_path = self.root / "snapshots" / "vercel__next_js.json"
        previous_snapshot_path.parent.mkdir(parents=True, exist_ok=True)
        dump_json(previous_snapshot_path, [{"citation": "https://nextjs.org/docs"}])
        os.environ["SOURCE_OPS_MEMORY_WRITER_TEMPLATE"] = "python3 -c \"print('write ok')\""
        os.environ["SOURCE_OPS_CLI_BIN"] = f"python3 {self._cli_stub()}"
        settings = load_settings()
        settings = settings.__class__(
            **{
                **settings.__dict__,
                "registry_path": registry_path,
                "raw_dir": self.raw_dir,
                "normalized_dir": self.normalized_dir,
                "reports_dir": self.reports_dir,
                "state_path": self.state_path,
            }
        )
        initial_rows = normalize.normalize_source(source, self.raw_dir, self.normalized_dir)
        initial_diff = diff.compute_diff(source, initial_rows, None, settings)
        initial_diff["success_snapshot"] = {
            "source": source,
            "payload_snapshot_path": str(previous_snapshot_path),
        }
        dump_json(
            self.state_path,
            {
                "last_run_at": None,
                "sources": {source["source_id"]: initial_diff},
            },
        )
        with patch.object(run_refresh, "collect_source", return_value={}):
            report = run_refresh.run_refresh(settings, source_id=source["source_id"], dry_run=False)
        self.assertEqual(report["sources"][0]["status"], "noop")
        persisted = json.loads(self.state_path.read_text())
        self.assertIn("success_snapshot", persisted["sources"][source["source_id"]])
        self.assertEqual(
            persisted["sources"][source["source_id"]]["success_snapshot"]["payload_snapshot_path"],
            str(previous_snapshot_path),
        )

    def test_rollback_runs_when_prod_fails(self) -> None:
        source = self._source()
        settings = self._settings()
        previous_snapshot_path = self.root / "snapshots" / "vercel__next_js.json"
        previous_snapshot_path.parent.mkdir(parents=True, exist_ok=True)
        dump_json(previous_snapshot_path, [{"source_id": source["source_id"], "citation": "https://nextjs.org/docs", "title": "old", "snippet": "old"}])
        snapshot = {"source": source, "payload_snapshot_path": str(previous_snapshot_path)}
        with patch.object(run_refresh, "apply_memory", side_effect=[
            {"status": "ok"},
            {"status": "failed"},
            {"status": "ok"},
        ]), patch.object(run_refresh, "apply_catalog", side_effect=[
            {"status": "ok"},
            {"status": "ok"},
            {"status": "ok"},
        ]), patch.object(run_refresh, "smoke_source", side_effect=[
            {"status": "ok"},
            {"status": "failed"},
            {"status": "ok"},
        ]):
            rollback = run_refresh._rollback_source(source, snapshot, settings, dry_run=False)
        self.assertEqual(rollback["status"], "rolled_back")

    def test_run_refresh_main_returns_error_for_partial_status(self) -> None:
        stdout = StringIO()
        stderr = StringIO()
        with patch.object(run_refresh, "run_refresh", return_value={"status": "partial"}), patch(
            "sys.argv",
            ["run_refresh.py"],
        ), patch("sys.stdout", stdout), patch("sys.stderr", stderr):
            self.assertEqual(run_refresh.main(), 1)

    def test_run_refresh_main_writes_non_ok_reports_to_stderr(self) -> None:
        stdout = StringIO()
        stderr = StringIO()
        with patch.object(run_refresh, "run_refresh", return_value={"status": "partial"}), patch(
            "sys.argv",
            ["run_refresh.py"],
        ), patch("sys.stdout", stdout), patch("sys.stderr", stderr):
            self.assertEqual(run_refresh.main(), 1)
        self.assertEqual(stdout.getvalue(), "")
        self.assertIn('"status": "partial"', stderr.getvalue())


if __name__ == "__main__":
    unittest.main()
