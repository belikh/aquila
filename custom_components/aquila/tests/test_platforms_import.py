"""Import smoke tests for every Aquila component module.

Home Assistant imports the integration and each platform at setup time, so a
``NameError``/``AttributeError`` raised while a class body is evaluated (for
example an invalid ``SensorDeviceClass`` member) surfaces only then — after a
restart, on a live system. Importing every module here catches that class of
failure in CI instead.

See ``home-assistant.md`` §6.1 (entity model) and §7 (config flow).
"""

from __future__ import annotations

import importlib

import pytest

MODULES = [
    "custom_components.aquila",
    "custom_components.aquila.api",
    "custom_components.aquila.binary_sensor",
    "custom_components.aquila.config_flow",
    "custom_components.aquila.const",
    "custom_components.aquila.coordinator",
    "custom_components.aquila.diagnostics",
    "custom_components.aquila.sensor",
    "custom_components.aquila.stream",
    "custom_components.aquila.update",
]


@pytest.mark.parametrize("module", MODULES)
def test_module_imports(module: str) -> None:
    """Every component module imports without raising."""
    importlib.import_module(module)


def test_every_declared_platform_has_an_importable_module() -> None:
    """Each platform in ``PLATFORMS`` maps to a module of the same name."""
    from custom_components.aquila.const import PLATFORMS

    for platform in PLATFORMS:
        importlib.import_module(f"custom_components.aquila.{platform.value}")
