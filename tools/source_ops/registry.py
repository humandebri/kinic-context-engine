# Where: tools/source_ops/registry.py
# What: Registry loading and validation for source refresh automation.
# Why: Keep source definitions declarative so new sources can be added without changing code.
from __future__ import annotations

from typing import Any

from .common import load_yaml_like_json
from .config import Settings


REQUIRED_SOURCE_FIELDS = [
    "source_id",
    "kind",
    "enabled",
    "crawl_targets",
    "normalization_profile",
    "catalog_metadata",
    "cadence",
    "version_strategy",
    "extraction_hints",
]
SOURCE_TYPES = {"docs_site", "llms_full", "repo_docs", "changelog", "examples"}
CRAWL_STRATEGIES = {"explicit_urls", "llms_full", "sitemap", "github_tree"}
COVERAGE_ROLES = {
    "overview",
    "api_reference",
    "integration",
    "migration",
    "troubleshooting",
    "examples",
    "release_notes",
    "sdk_reference",
}
REQUIRED_COVERAGE_ROLES = {"overview", "api_reference", "examples"}
REQUIRED_TARGET_FIELDS = [
    "label",
    "source_type",
    "crawl_strategy",
    "url",
    "include_prefixes",
    "exclude_prefixes",
    "max_pages",
    "coverage_role",
]


def load_registry(settings: Settings) -> list[dict[str, Any]]:
    registry = load_yaml_like_json(settings.registry_path)
    if not isinstance(registry, list):
        raise ValueError("registry.yaml must contain a top-level array")
    return registry


def validate_registry_entry(source: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    source_id = source.get("source_id", "<missing>")
    for field in REQUIRED_SOURCE_FIELDS:
        if field not in source:
            errors.append(f"{source_id}: missing `{field}`")

    metadata = source.get("catalog_metadata", {})
    smoke_queries = source.get("smoke_queries", {})
    extraction_hints = source.get("extraction_hints", {})
    crawl_targets = source.get("crawl_targets", [])
    for field in ["title", "aliases", "domain", "trust", "supported_versions", "citations"]:
        if field not in metadata:
            errors.append(f"{source_id}: catalog_metadata missing `{field}`")
    for field in ["resolve", "query", "pack"]:
        if field not in smoke_queries:
            errors.append(f"{source_id}: smoke_queries missing `{field}`")
    for field in ["content_roots", "drop_selectors", "chunk_target_chars"]:
        if field not in extraction_hints:
            errors.append(f"{source_id}: extraction_hints missing `{field}`")
    if not isinstance(crawl_targets, list) or not crawl_targets:
        errors.append(f"{source_id}: crawl_targets must be a non-empty list")
    coverage_roles: set[str] = set()
    for index, target in enumerate(crawl_targets):
        target_label = target.get("label", f"target-{index}")
        for field in REQUIRED_TARGET_FIELDS:
            if field not in target:
                errors.append(f"{source_id}/{target_label}: crawl_targets missing `{field}`")
        if target.get("source_type") not in SOURCE_TYPES:
            errors.append(f"{source_id}/{target_label}: unknown source_type `{target.get('source_type')}`")
        if target.get("crawl_strategy") not in CRAWL_STRATEGIES:
            errors.append(f"{source_id}/{target_label}: unknown crawl_strategy `{target.get('crawl_strategy')}`")
        coverage_role = target.get("coverage_role")
        if coverage_role not in COVERAGE_ROLES:
            errors.append(f"{source_id}/{target_label}: unknown coverage_role `{coverage_role}`")
        elif isinstance(coverage_role, str):
            coverage_roles.add(coverage_role)
        if not isinstance(target.get("max_pages"), int) or target.get("max_pages", 0) < 1:
            errors.append(f"{source_id}/{target_label}: max_pages must be a positive integer")
        for field in ["include_prefixes", "exclude_prefixes"]:
            if not isinstance(target.get(field), list):
                errors.append(f"{source_id}/{target_label}: {field} must be a list")
    missing_roles = sorted(REQUIRED_COVERAGE_ROLES - coverage_roles)
    if missing_roles:
        errors.append(f"{source_id}: missing required coverage_role(s) {', '.join(missing_roles)}")
    return errors


def validate_registry(sources: list[dict[str, Any]]) -> list[str]:
    errors: list[str] = []
    seen: set[str] = set()
    for source in sources:
        source_id = source.get("source_id")
        if source_id in seen:
            errors.append(f"{source_id}: duplicate source_id")
        if source_id:
            seen.add(source_id)
        errors.extend(validate_registry_entry(source))
    return errors


def select_sources(
    sources: list[dict[str, Any]],
    *,
    source_id: str | None = None,
    cadence: str | None = None,
) -> list[dict[str, Any]]:
    selected = [source for source in sources if source.get("enabled", False)]
    if source_id is not None:
        selected = [source for source in selected if source["source_id"] == source_id]
    if cadence is not None:
        selected = [source for source in selected if source.get("cadence") == cadence]
    return selected
