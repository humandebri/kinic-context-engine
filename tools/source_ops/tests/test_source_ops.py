# Where: tools/source_ops/tests/test_source_ops.py
# What: Unit and integration coverage for source_ops registry, normalization, diffing, smoke, and orchestration.
# Why: Keep the automation contract stable before wiring it to daily Codex runs and real wiki databases.
from __future__ import annotations

import json
import os
import hashlib
import tempfile
import unittest
from io import StringIO
from pathlib import Path
from unittest.mock import patch

from tools.source_ops import (
    collect,
    diff,
    embedding,
    kinic_writer,
    normalize,
    register_source,
    registry,
    run_refresh,
    smoke,
    validate,
)
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
        os.environ["SOURCE_OPS_STAGING_DATABASE_ID"] = "staging-db"
        os.environ["SOURCE_OPS_PROD_DATABASE_ID"] = "prod-db"
        os.environ["SOURCE_OPS_WIKI_CLI_BIN"] = f"python3 {self._cli_stub()}"
        os.environ["SOURCE_OPS_CLI_BIN"] = "python3 missing-context-cli.py"
        settings = load_settings()
        return settings

    def _cli_stub(self) -> Path:
        path = self.root / "fake_cli.py"
        write_text(
            path,
            "\n".join(
                [
                    "import json, sys",
                    "commands = {'search-remote', 'read-node-context', 'query-context', 'source-evidence'}",
                    "cmd = next((arg for arg in sys.argv[1:] if arg in commands), '')",
                    "if cmd == 'search-remote':",
                    "    print(json.dumps([{'path':'/Wiki/sources/vercel__next_js/15/middleware-s0000-c0000.md'}]))",
                    "elif cmd == 'read-node-context':",
                    "    print(json.dumps({'node':{'path':'/Wiki/sources/vercel__next_js/index.md'},'incoming_links':[],'outgoing_links':[{'target_path':'/Sources/raw/vercel__next_js/vercel__next_js.md'}]}))",
                    "elif cmd in {'query-context', 'source-evidence'}:",
                    "    raise SystemExit(7)",
                    "else:",
                    "    raise SystemExit(2)",
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
            "crawl_targets": [
                {
                    "label": "overview",
                    "source_type": "docs_site",
                    "crawl_strategy": "explicit_urls",
                    "url": self.fixture.as_uri(),
                    "include_prefixes": [],
                    "exclude_prefixes": [],
                    "max_pages": 1,
                    "coverage_role": "overview",
                },
                {
                    "label": "api-reference",
                    "source_type": "docs_site",
                    "crawl_strategy": "explicit_urls",
                    "url": f"{self.fixture.as_uri()}?api",
                    "include_prefixes": [],
                    "exclude_prefixes": [],
                    "max_pages": 1,
                    "coverage_role": "api_reference",
                },
                {
                    "label": "examples",
                    "source_type": "examples",
                    "crawl_strategy": "explicit_urls",
                    "url": f"{self.fixture.as_uri()}?examples",
                    "include_prefixes": [],
                    "exclude_prefixes": [],
                    "max_pages": 1,
                    "coverage_role": "examples",
                }
            ],
            "normalization_profile": "docs_html",
            "catalog_metadata": {
                "title": "Next.js Docs",
                "aliases": ["next", "middleware"],
                "domain": "code_docs",
                "trust": "official",
                "supported_versions": ["15"],
                "citations": ["https://nextjs.org/docs"],
                "retrieved_at": "2026-03-18T00:00:00Z",
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
        sources = registry.load_registry(load_settings())
        errors = registry.validate_registry(sources)
        self.assertEqual(errors, [])
        by_id = {source["source_id"]: source for source in sources}
        self.assertEqual(by_id["/vercel/next.js"]["catalog_metadata"]["supported_versions"], ["16"])
        self.assertEqual(by_id["/react/docs"]["catalog_metadata"]["supported_versions"], ["19.2"])
        self.assertEqual(by_id["/supabase/docs"]["catalog_metadata"]["retrieved_at"], "2026-05-25T00:00:00Z")

    def test_register_source_builds_valid_registry_entry(self) -> None:
        entry = register_source.build_source_entry(
            source_id="/tanstack/query",
            title="TanStack Query Docs",
            urls=[
                "overview=https://tanstack.com/query/latest/docs/framework/react/overview",
                "api=https://tanstack.com/query/latest/docs/framework/react/reference/useQuery",
                "examples=https://tanstack.com/query/latest/docs/framework/react/examples/basic",
            ],
            aliases=["tanstack query", "react query"],
            versions=["latest"],
            citations=[],
            trust="official",
            domain="code_docs",
            cadence="manual",
            chunk_target_chars=900,
        )
        self.assertEqual(registry.validate_registry([entry]), [])
        self.assertEqual(entry["crawl_targets"][0]["label"], "overview")
        self.assertEqual(entry["crawl_targets"][0]["crawl_strategy"], "explicit_urls")
        self.assertEqual(entry["crawl_targets"][2]["coverage_role"], "examples")
        self.assertIn("tanstack query", entry["catalog_metadata"]["aliases"])

    def test_register_source_accepts_unlabeled_url_with_query_equals(self) -> None:
        parsed = register_source.parse_labeled_url("https://example.com/docs?foo=bar", 1)
        self.assertEqual(parsed, {"label": "docs-1", "url": "https://example.com/docs?foo=bar"})

    def test_register_source_accepts_labeled_url_with_query_equals(self) -> None:
        parsed = register_source.parse_labeled_url("docs=https://example.com/docs?foo=bar", 1)
        self.assertEqual(parsed, {"label": "docs", "url": "https://example.com/docs?foo=bar"})

    def test_register_source_requires_minimum_coverage_urls(self) -> None:
        with self.assertRaises(ValueError):
            register_source.build_source_entry(
                source_id="/tanstack/query",
                title="TanStack Query Docs",
                urls=["overview=https://tanstack.com/query/latest/docs/framework/react/overview"],
                aliases=[],
                versions=["latest"],
                citations=[],
                trust="official",
                domain="code_docs",
                cadence="manual",
                chunk_target_chars=900,
            )

    def test_register_source_upserts_existing_entry(self) -> None:
        existing = self._source()
        entry = register_source.build_source_entry(
            source_id=existing["source_id"],
            title="Next.js Docs Replacement",
            urls=[
                "overview=https://nextjs.org/docs",
                "api=https://nextjs.org/docs/app/api-reference",
                "examples=https://github.com/vercel/next.js/tree/canary/examples",
            ],
            aliases=["next"],
            versions=["15"],
            citations=[],
            trust="official",
            domain="code_docs",
            cadence="manual",
            chunk_target_chars=900,
        )
        updated = register_source.upsert_source_entry([existing], entry)
        self.assertEqual(len(updated), 1)
        self.assertEqual(updated[0]["catalog_metadata"]["title"], "Next.js Docs Replacement")

    def test_registry_rejects_invalid_crawl_target(self) -> None:
        source = self._source()
        source["crawl_targets"][0].pop("max_pages")
        source["crawl_targets"][0]["crawl_strategy"] = "unknown"
        source["crawl_targets"][0]["coverage_role"] = "unknown"
        errors = registry.validate_registry([source])
        self.assertTrue(any("max_pages" in error for error in errors))
        self.assertTrue(any("unknown crawl_strategy" in error for error in errors))
        self.assertTrue(any("unknown coverage_role" in error for error in errors))

    def test_registry_rejects_missing_required_coverage_roles(self) -> None:
        source = self._source()
        source["crawl_targets"] = source["crawl_targets"][:1]
        errors = registry.validate_registry([source])
        self.assertTrue(any("missing required coverage_role" in error for error in errors))

    def test_collect_sitemap_filters_same_host_and_prefix(self) -> None:
        sitemap = """<?xml version="1.0"?><urlset>
        <url><loc>https://example.com/docs/a</loc></url>
        <url><loc>https://example.com/blog/b</loc></url>
        <url><loc>https://other.example/docs/c</loc></url>
        </urlset>"""
        target = {
            "label": "docs",
            "source_type": "docs_site",
            "crawl_strategy": "sitemap",
            "url": "https://example.com/sitemap.xml",
            "include_prefixes": ["https://example.com/docs"],
            "exclude_prefixes": [],
            "max_pages": 5,
            "coverage_role": "api_reference",
        }
        with patch.object(collect, "_fetch_url", return_value={"body": sitemap}):
            self.assertEqual(collect._sitemap_urls(target, 5), ["https://example.com/docs/a"])

    def test_collect_sitemap_stops_nested_fetches_at_max_pages(self) -> None:
        index = """<?xml version="1.0"?><sitemapindex>
        <sitemap><loc>https://example.com/sitemap-a.xml</loc></sitemap>
        <sitemap><loc>https://example.com/sitemap-b.xml</loc></sitemap>
        </sitemapindex>"""
        sitemap_a = """<?xml version="1.0"?><urlset>
        <url><loc>https://example.com/docs/a</loc></url>
        <url><loc>https://example.com/blog/b</loc></url>
        <url><loc>https://other.example/docs/c</loc></url>
        <url><loc>https://example.com/docs/excluded</loc></url>
        <url><loc>https://example.com/docs/e</loc></url>
        </urlset>"""
        sitemap_b = """<?xml version="1.0"?><urlset>
        <url><loc>https://example.com/docs/f</loc></url>
        </urlset>"""
        fetched_urls: list[str] = []

        def fake_fetch(url: str, timeout: int) -> dict[str, object]:
            fetched_urls.append(url)
            bodies = {
                "https://example.com/sitemap.xml": index,
                "https://example.com/sitemap-a.xml": sitemap_a,
                "https://example.com/sitemap-b.xml": sitemap_b,
            }
            return {"body": bodies[url]}

        target = {
            "label": "docs",
            "source_type": "docs_site",
            "crawl_strategy": "sitemap",
            "url": "https://example.com/sitemap.xml",
            "include_prefixes": ["https://example.com/docs"],
            "exclude_prefixes": ["https://example.com/docs/excluded"],
            "max_pages": 2,
            "coverage_role": "api_reference",
        }
        with patch.object(collect, "_fetch_url", side_effect=fake_fetch):
            self.assertEqual(collect._sitemap_urls(target, 5), ["https://example.com/docs/a", "https://example.com/docs/e"])
        self.assertEqual(fetched_urls, ["https://example.com/sitemap.xml", "https://example.com/sitemap-a.xml"])

    def test_collect_github_tree_selects_markdown_docs(self) -> None:
        tree = {
            "tree": [
                {"type": "blob", "path": "docs/guide.md"},
                {"type": "blob", "path": "docs/app.ts"},
                {"type": "blob", "path": "examples/demo.mdx"},
            ]
        }
        target = {
            "label": "repo-docs",
            "source_type": "repo_docs",
            "crawl_strategy": "github_tree",
            "url": "https://github.com/acme/docs/tree/main/docs",
            "include_prefixes": ["docs"],
            "exclude_prefixes": [],
            "max_pages": 10,
            "coverage_role": "api_reference",
        }
        with patch.object(collect, "_fetch_url", return_value={"body": json.dumps(tree)}):
            urls = collect._github_tree_urls(target, 5)
        self.assertEqual(urls, ["https://raw.githubusercontent.com/acme/docs/main/docs/guide.md"])

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

    def test_normalize_keeps_inline_code_inside_paragraph_chunk(self) -> None:
        source = self._source()
        raw_path = self.raw_dir / "vercel__next_js"
        raw_path.mkdir(parents=True)
        body = (
            "<html><body><article><h1 id='middleware'>Middleware</h1>"
            "<p>Use <code>cookies</code> with <code>NextResponse</code> in proxy logic.</p>"
            "</article></body></html>"
        )
        dump_json(
            raw_path / "latest.json",
            {"source_id": source["source_id"], "collected_at": "2026-03-18T00:00:00Z", "items": [{"url": self.fixture.as_uri(), "final_url": self.fixture.as_uri(), "label": "middleware", "role": "public_urls", "body": body, "content_type": "text/html", "sha256": "x", "status_code": 200, "etag": None, "last_modified": None}]},
        )
        rows = normalize.normalize_source(source, self.raw_dir, self.normalized_dir)
        self.assertEqual(len(rows), 1)
        self.assertIn("cookies", rows[0]["content"])
        self.assertIn("NextResponse", rows[0]["content"])

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

    def test_normalize_accepts_large_hydration_html_with_valid_article(self) -> None:
        source = self._source()
        raw_path = self.raw_dir / "vercel__next_js"
        raw_path.mkdir(parents=True)
        body = (
            "<html><head><title>Next.js Middleware</title></head><body>"
            "<script>" + ("var x='hydration';" * 5000) + "</script>"
            "<article><h1 id='middleware'>Middleware</h1><p>"
            + ("Use middleware to inspect cookies. " * 8)
            + "</p></article></body></html>"
        )
        dump_json(
            raw_path / "latest.json",
            {"source_id": source["source_id"], "collected_at": "2026-03-18T00:00:00Z", "items": [{"url": self.fixture.as_uri(), "final_url": self.fixture.as_uri(), "label": "middleware", "role": "public_urls", "body": body, "content_type": "text/html", "sha256": "x", "status_code": 200, "etag": None, "last_modified": None}]},
        )
        rows = normalize.normalize_source(source, self.raw_dir, self.normalized_dir)
        self.assertGreater(len(rows), 0)
        self.assertTrue(rows[0]["citation"].endswith("#middleware"))

    def test_normalize_records_crawl_target_metadata(self) -> None:
        source = self._source()
        raw_path = self.raw_dir / "vercel__next_js"
        raw_path.mkdir(parents=True)
        dump_json(
            raw_path / "latest.json",
            {
                "source_id": source["source_id"],
                "collected_at": "2026-03-18T00:00:00Z",
                "items": [
                    {"url": self.fixture.as_uri(), "final_url": self.fixture.as_uri(), "label": "middleware", "role": "crawl_targets", "source_type": "docs_site", "target_label": "middleware", "coverage_role": "api_reference", "body": self.fixture.read_text(), "content_type": "text/html", "sha256": "x", "status_code": 200, "etag": None, "last_modified": None},
                    {"url": "https://nextjs.org/docs/llms-full.txt", "final_url": "https://nextjs.org/docs/llms-full.txt", "label": "llms-full", "role": "crawl_targets", "source_type": "llms_full", "target_label": "llms-full", "coverage_role": "overview", "body": "# Middleware\nUse middleware from llms-full.", "content_type": "text/plain", "sha256": "y", "status_code": 200, "etag": "1", "last_modified": "yesterday"},
                ],
            },
        )
        rows = normalize.normalize_source(source, self.raw_dir, self.normalized_dir)
        self.assertEqual({row["source_type"] for row in rows}, {"docs_site", "llms_full"})
        self.assertEqual({row["coverage_role"] for row in rows}, {"api_reference", "overview"})
        self.assertTrue(any(row["target_label"] == "llms-full" for row in rows))

    def test_normalize_markdown_keeps_code_fence_together(self) -> None:
        source = self._source()
        source["extraction_hints"]["chunk_target_chars"] = 80
        raw_path = self.raw_dir / "vercel__next_js"
        raw_path.mkdir(parents=True)
        markdown = "\n".join(
            [
                "# Middleware",
                "Use middleware before rendering.",
                "",
                "```ts",
                "export function middleware() {",
                "  return Response.json({ ok: true })",
                "}",
                "```",
                "",
                "More text after the code fence.",
            ]
        )
        dump_json(
            raw_path / "latest.json",
            {
                "source_id": source["source_id"],
                "collected_at": "2026-03-18T00:00:00Z",
                "items": [
                    {"url": "https://nextjs.org/docs/llms-full.txt", "final_url": "https://nextjs.org/docs/llms-full.txt", "label": "llms-full", "role": "crawl_targets", "source_type": "llms_full", "target_label": "llms-full", "body": markdown, "content_type": "text/plain", "sha256": "x", "status_code": 200, "etag": None, "last_modified": None}
                ],
            },
        )
        rows = normalize.normalize_source(source, self.raw_dir, self.normalized_dir)
        self.assertTrue(any(row["citation"].endswith("#middleware") for row in rows))
        code_rows = [row for row in rows if "```ts" in row["content"]]
        self.assertEqual(len(code_rows), 1)
        self.assertIn("```", code_rows[0]["content"])

    def test_normalize_skips_markdown_chunks_without_display_text(self) -> None:
        source = self._source()
        raw_path = self.raw_dir / "vercel__next_js"
        raw_path.mkdir(parents=True)
        dump_json(
            raw_path / "latest.json",
            {
                "source_id": source["source_id"],
                "collected_at": "2026-03-18T00:00:00Z",
                "items": [
                    {"url": "https://nextjs.org/docs/llms-full.txt", "final_url": "https://nextjs.org/docs/llms-full.txt", "label": "llms-full", "role": "crawl_targets", "source_type": "llms_full", "target_label": "llms-full", "body": "# Middleware\n<OnlyTag>\n\nUse middleware.", "content_type": "text/plain", "sha256": "x", "status_code": 200, "etag": None, "last_modified": None}
                ],
            },
        )
        rows = normalize.normalize_source(source, self.raw_dir, self.normalized_dir)
        self.assertTrue(all(row["snippet"] for row in rows))

    def test_normalize_deduplicates_identical_chunks(self) -> None:
        source = self._source()
        raw_path = self.raw_dir / "vercel__next_js"
        raw_path.mkdir(parents=True)
        item = {"url": "https://nextjs.org/docs/a", "final_url": "https://nextjs.org/docs/a", "label": "docs", "role": "crawl_targets", "source_type": "docs_site", "target_label": "docs", "body": "<html><body><article><h1 id='a'>A</h1><p>Use middleware safely.</p></article></body></html>", "content_type": "text/html", "sha256": "x", "status_code": 200, "etag": None, "last_modified": None}
        dump_json(
            raw_path / "latest.json",
            {"source_id": source["source_id"], "collected_at": "2026-03-18T00:00:00Z", "items": [item, {**item, "target_label": "llms-full"}]},
        )
        rows = normalize.normalize_source(source, self.raw_dir, self.normalized_dir)
        self.assertEqual(len(rows), 1)

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

    def test_default_writer_uses_wiki_cli(self) -> None:
        settings = load_settings()
        self.assertEqual(settings.wiki_cli_bin, "kinic-vfs-cli")

    def test_kinic_writer_builds_wiki_nodes(self) -> None:
        source = self._source()
        payload = {
            "source_id": "/vercel/next.js",
            "title": "Next.js Middleware",
            "snippet": "Use middleware to inspect cookies.",
            "content": "Full chunk text",
            "section": "middleware",
            "version": "15",
            "citation": "https://nextjs.org/docs/middleware",
            "tags": ["routing"],
            "source_type": "docs_site",
            "target_label": "middleware",
            "coverage_role": "api_reference",
            "upstream_url": "https://nextjs.org/docs/middleware",
        }
        text = embedding.document_input_text(payload)
        self.assertIn("Next.js Middleware", text)
        self.assertTrue(text.startswith("passage: "))
        nodes = kinic_writer.build_wiki_nodes(source, [payload])
        self.assertEqual(nodes[0]["path"], "/Sources/raw/vercel__next_js/vercel__next_js.md")
        self.assertEqual(nodes[0]["kind"], "source")
        self.assertTrue(any(path.endswith("-middleware-s0000-c0000.md") for path in [node["path"] for node in nodes]))
        self.assertIn("Source evidence: [/Sources/raw/vercel__next_js/vercel__next_js.md]", nodes[-1]["content"])
        metadata = json.loads(nodes[-1]["metadata_json"])
        self.assertEqual(metadata["source_id"], "/vercel/next.js")
        self.assertEqual(metadata["section_index"], 0)
        self.assertEqual(metadata["chunk_index"], 0)
        self.assertEqual(metadata["source_type"], "docs_site")
        self.assertEqual(metadata["target_label"], "middleware")
        self.assertEqual(metadata["coverage_role"], "api_reference")
        self.assertEqual(len(metadata["chunk_id"]), 16)
        self.assertEqual(
            metadata["content_sha256"],
            hashlib.sha256(b"Full chunk text").hexdigest(),
        )
        section_text = embedding.section_input_text("middleware", "Middleware", "Use middleware.")
        self.assertTrue(section_text.startswith("passage: "))

    def test_kinic_writer_disambiguates_chunk_paths_by_citation(self) -> None:
        source = self._source()
        payloads = [
            {
                "source_id": "/vercel/next.js",
                "title": "A",
                "snippet": "one",
                "content": "one",
                "section": "middleware",
                "version": "15",
                "citation": "https://nextjs.org/docs/a#middleware",
                "section_index": 0,
                "chunk_index": 0,
            },
            {
                "source_id": "/vercel/next.js",
                "title": "B",
                "snippet": "two",
                "content": "two",
                "section": "middleware",
                "version": "15",
                "citation": "https://nextjs.org/docs/b#middleware",
                "section_index": 0,
                "chunk_index": 0,
            },
        ]
        doc_paths = [node["path"] for node in kinic_writer.build_wiki_nodes(source, payloads)[2:]]
        doc_metadata = [json.loads(node["metadata_json"]) for node in kinic_writer.build_wiki_nodes(source, payloads)[2:]]
        self.assertEqual(len(doc_paths), len(set(doc_paths)))
        self.assertEqual(len({item["chunk_id"] for item in doc_metadata}), 2)
        self.assertTrue(all(path.endswith("-middleware-s0000-c0000.md") for path in doc_paths))

        duplicate = [payloads[0], {**payloads[0], "title": "Duplicate"}]
        with self.assertRaises(ValueError):
            kinic_writer.build_wiki_nodes(source, duplicate)

    def test_kinic_writer_requires_non_empty_chunk_text(self) -> None:
        source = self._source()
        fallback = {
            "source_id": "/vercel/next.js",
            "title": "Fallback",
            "snippet": "snippet text",
            "content": "",
            "section": "middleware",
            "version": "15",
            "citation": "https://nextjs.org/docs/fallback",
            "section_index": 0,
            "chunk_index": 0,
        }
        nodes = kinic_writer.build_wiki_nodes(source, [fallback])
        self.assertIn("snippet text", nodes[-1]["content"])

        empty = {**fallback, "snippet": " ", "content": " "}
        with self.assertRaises(ValueError):
            kinic_writer.build_wiki_nodes(source, [empty])

    def test_kinic_writer_uses_write_nodes_batch_command(self) -> None:
        source = self._source()
        payload = {
            "source_id": "/vercel/next.js",
            "title": "Next.js Middleware",
            "snippet": "Use middleware to inspect cookies.",
            "content": "Full chunk text",
            "section": "middleware",
            "version": "15",
            "citation": "https://nextjs.org/docs/middleware",
            "tags": ["routing"],
            "source_type": "docs_site",
            "target_label": "middleware",
            "coverage_role": "api_reference",
            "upstream_url": "https://nextjs.org/docs/middleware",
        }
        normalized_path = self.normalized_dir / "vercel__next_js.jsonl"
        self.normalized_dir.mkdir(parents=True)
        write_text(normalized_path, json.dumps(payload, sort_keys=True) + "\n")
        settings = self._settings()
        settings = settings.__class__(
            **{
                **settings.__dict__,
                "normalized_dir": self.normalized_dir,
                "wiki_nodes_dir": self.root / "wiki_nodes",
            }
        )
        report = kinic_writer.write_batch(source, settings, "staging", dry_run=True)
        self.assertEqual(report["command_count"], 1)
        self.assertEqual(report["node_count"], 3)
        command = report["results"][0]["command"]
        self.assertIn("write-nodes", command)
        self.assertNotIn("write-node", command)
        batch_nodes = json.loads(Path(report["batch_input"]).read_text())
        self.assertEqual(len(batch_nodes), 3)
        self.assertEqual(batch_nodes[-1]["metadata_json"], kinic_writer.build_wiki_nodes(source, [payload])[-1]["metadata_json"])

    def test_smoke_uses_cli_contract(self) -> None:
        source = self._source()
        settings = self._settings()
        report = smoke.smoke_source(source, settings, "staging", False)
        self.assertEqual(report["status"], "ok")
        commands = [" ".join(check["command"]) for check in report["checks"]]
        self.assertTrue(all(str(self.root / "fake_cli.py") in command for command in commands))
        self.assertTrue(any("read-node-context" in command for command in commands))
        self.assertFalse(any("query-context" in command for command in commands))
        self.assertFalse(any("source-evidence" in command for command in commands))

    def test_smoke_fails_when_index_has_no_raw_source_link(self) -> None:
        source = self._source()
        broken_cli = self.root / "broken_cli.py"
        write_text(
            broken_cli,
            "\n".join(
                [
                    "import json, sys",
                    "if 'search-remote' in sys.argv:",
                    "    print(json.dumps([{'path':'/Wiki/sources/vercel__next_js/15/middleware-s0000-c0000.md'}]))",
                    "elif 'read-node-context' in sys.argv:",
                    "    print(json.dumps({'node':{'path':'/Wiki/sources/vercel__next_js/index.md'},'incoming_links':[],'outgoing_links':[]}))",
                    "else:",
                    "    raise SystemExit(2)",
                ]
            )
            + "\n",
        )
        settings = self._settings()
        settings = settings.__class__(**{**settings.__dict__, "wiki_cli_bin": f"python3 {broken_cli}"})
        report = smoke.smoke_source(source, settings, "staging", False)
        self.assertEqual(report["status"], "failed")
        self.assertIn("source-evidence", report["failures"])

    def test_run_refresh_dry_run_writes_report(self) -> None:
        source = self._source()
        registry_path = self.root / "registry.yaml"
        write_text(registry_path, json.dumps([source], indent=2))
        os.environ["SOURCE_OPS_STAGING_DATABASE_ID"] = "staging-db"
        os.environ["SOURCE_OPS_PROD_DATABASE_ID"] = "prod-db"
        os.environ["SOURCE_OPS_WIKI_CLI_BIN"] = "python3 -c \"print('write ok')\""
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
        report = run_refresh.run_refresh(settings, source_id=source["source_id"], dry_run=True)
        self.assertIn(report["status"], {"ok", "partial"})
        self.assertIn("quality_gates", report["sources"][0])
        self.assertIn("wiki_write", report["sources"][0]["staging"])
        self.assertIn("search-remote returns docs chunks under /Wiki/sources", report["sources"][0]["quality_gates"]["required_smoke"])
        self.assertEqual(report["sources"][0]["coverage"]["missing_required_roles"], [])
        self.assertEqual(
            set(report["sources"][0]["coverage"]["coverage_role_breakdown"]),
            {"overview", "api_reference", "examples"},
        )
        self.assertTrue(any(self.reports_dir.iterdir()))

    def test_run_refresh_accepts_manual_source_when_explicit(self) -> None:
        source = self._source()
        source["cadence"] = "manual"
        registry_path = self.root / "registry.yaml"
        write_text(registry_path, json.dumps([source], indent=2))
        os.environ["SOURCE_OPS_STAGING_DATABASE_ID"] = "staging-db"
        os.environ["SOURCE_OPS_PROD_DATABASE_ID"] = "prod-db"
        os.environ["SOURCE_OPS_WIKI_CLI_BIN"] = "python3 -c \"print('write ok')\""
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
        report = run_refresh.run_refresh(settings, source_id=source["source_id"], dry_run=True)
        self.assertEqual(len(report["sources"]), 1)

    def test_run_refresh_fails_for_unknown_explicit_source(self) -> None:
        source = self._source()
        registry_path = self.root / "registry.yaml"
        write_text(registry_path, json.dumps([source], indent=2))
        settings = load_settings()
        settings = settings.__class__(**{**settings.__dict__, "registry_path": registry_path})
        report = run_refresh.run_refresh(settings, source_id="/missing/docs", dry_run=True)
        self.assertEqual(report["status"], "invalid_source")
        self.assertEqual(report["sources"], [])

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
        os.environ["SOURCE_OPS_STAGING_DATABASE_ID"] = "staging-db"
        os.environ["SOURCE_OPS_PROD_DATABASE_ID"] = "prod-db"
        os.environ["SOURCE_OPS_WIKI_CLI_BIN"] = "python3 -c \"print('write ok')\""
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
        with patch.object(run_refresh, "collect_source", return_value={}), patch.object(
            run_refresh, "apply_wiki", return_value={"status": "ok"}
        ), patch.object(run_refresh, "smoke_source", return_value={"status": "ok"}
        ):
            report = run_refresh.run_refresh(settings, source_id=source["source_id"], dry_run=False)
        self.assertEqual(report["sources"][0]["status"], "needs_review")
        self.assertIn("warnings", report["sources"][0])

    def test_run_refresh_marks_needs_review_when_noop_lacks_required_role(self) -> None:
        source = self._source()
        source["extraction_hints"]["content_roots"] = []
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
                    {"url": self.fixture.as_uri(), "final_url": self.fixture.as_uri(), "label": "overview", "role": "crawl_targets", "source_type": "docs_site", "target_label": "overview", "coverage_role": "overview", "body": self.fixture.read_text(), "content_type": "text/html", "sha256": "x", "status_code": 200, "etag": None, "last_modified": None},
                    {"url": f"{self.fixture.as_uri()}?api", "final_url": f"{self.fixture.as_uri()}?api", "label": "api", "role": "crawl_targets", "source_type": "docs_site", "target_label": "api", "coverage_role": "api_reference", "body": self.fixture.read_text(), "content_type": "text/html", "sha256": "y", "status_code": 200, "etag": None, "last_modified": None},
                ],
            },
        )
        os.environ["SOURCE_OPS_STAGING_DATABASE_ID"] = "staging-db"
        os.environ["SOURCE_OPS_PROD_DATABASE_ID"] = "prod-db"
        os.environ["SOURCE_OPS_WIKI_CLI_BIN"] = "python3 -c \"print('write ok')\""
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
        dump_json(self.state_path, {"last_run_at": None, "sources": {source["source_id"]: initial_diff}})
        with patch.object(run_refresh, "collect_source", return_value={}), patch.object(
            run_refresh, "apply_wiki", return_value={"status": "ok"}
        ) as apply_wiki_mock, patch.object(run_refresh, "smoke_source", return_value={"status": "ok"}):
            report = run_refresh.run_refresh(settings, source_id=source["source_id"], dry_run=False)
        item = report["sources"][0]
        self.assertEqual(item["status"], "needs_review")
        self.assertEqual(item["coverage"]["missing_required_roles"], ["examples"])
        self.assertEqual(item["diff"]["missing_required_roles"], ["examples"])
        self.assertTrue(item["quality_gates"]["needs_review"])
        apply_wiki_mock.assert_called_once()

    def test_run_refresh_noop_keeps_previous_success_snapshot(self) -> None:
        source = self._source()
        source["extraction_hints"]["content_roots"] = []
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
                    {"url": self.fixture.as_uri(), "final_url": self.fixture.as_uri(), "label": "overview", "role": "crawl_targets", "source_type": "docs_site", "target_label": "overview", "coverage_role": "overview", "body": self.fixture.read_text(), "content_type": "text/html", "sha256": "x", "status_code": 200, "etag": None, "last_modified": None},
                    {"url": f"{self.fixture.as_uri()}?api", "final_url": f"{self.fixture.as_uri()}?api", "label": "api", "role": "crawl_targets", "source_type": "docs_site", "target_label": "api", "coverage_role": "api_reference", "body": self.fixture.read_text(), "content_type": "text/html", "sha256": "y", "status_code": 200, "etag": None, "last_modified": None},
                    {"url": f"{self.fixture.as_uri()}?examples", "final_url": f"{self.fixture.as_uri()}?examples", "label": "examples", "role": "crawl_targets", "source_type": "examples", "target_label": "examples", "coverage_role": "examples", "body": self.fixture.read_text(), "content_type": "text/html", "sha256": "z", "status_code": 200, "etag": None, "last_modified": None},
                ],
            },
        )
        previous_snapshot_path = self.root / "snapshots" / "vercel__next_js.json"
        previous_snapshot_path.parent.mkdir(parents=True, exist_ok=True)
        dump_json(previous_snapshot_path, [{"citation": "https://nextjs.org/docs"}])
        os.environ["SOURCE_OPS_STAGING_DATABASE_ID"] = "staging-db"
        os.environ["SOURCE_OPS_PROD_DATABASE_ID"] = "prod-db"
        os.environ["SOURCE_OPS_WIKI_CLI_BIN"] = "python3 -c \"print('write ok')\""
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
        with patch.object(run_refresh, "apply_wiki", side_effect=[
            {"status": "ok"},
            {"status": "failed"},
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
