"""Pytest configuration for the Aquila component tests.

Makes the component importable as ``custom_components.aquila`` (its modules use
package-relative imports) and puts this directory on ``sys.path`` so the shared
``helpers`` module imports cleanly. No Home Assistant runtime is required;
``pytest-homeassistant-custom-component`` is not installed, so the tests stub the
collaborators and use the real HA base classes only where they instantiate
without a running HA.
"""

from __future__ import annotations

import sys
from pathlib import Path

_TESTS_DIR = Path(__file__).resolve().parent
_REPO_ROOT = _TESTS_DIR.parents[2]

for _path in (_REPO_ROOT, _TESTS_DIR):
    if str(_path) not in sys.path:
        sys.path.insert(0, str(_path))
