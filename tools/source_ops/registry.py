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
    "public_urls",
    "discovery_urls",
    "normalization_profile",
    "catalog_metadata",
    "memory_targets",
    "cadence",
    "version_strategy",
    "extraction_hints",
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
    memory_targets = source.get("memory_targets", {})
    smoke_queries = source.get("smoke_queries", {})
    extraction_hints = source.get("extraction_hints", {})
    for field in ["title", "aliases", "domain", "trust", "supported_versions", "citations"]:
        if field not in metadata:
            errors.append(f"{source_id}: catalog_metadata missing `{field}`")
    for field in ["staging_canister_ids", "prod_canister_ids"]:
        if field not in memory_targets:
            errors.append(f"{source_id}: memory_targets missing `{field}`")
    for field in ["resolve", "query", "pack"]:
        if field not in smoke_queries:
            errors.append(f"{source_id}: smoke_queries missing `{field}`")
    for field in ["content_roots", "drop_selectors", "chunk_target_chars"]:
        if field not in extraction_hints:
            errors.append(f"{source_id}: extraction_hints missing `{field}`")
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
