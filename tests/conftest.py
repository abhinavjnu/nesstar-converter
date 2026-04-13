import pytest


def pytest_configure(config):
    config.addinivalue_line("markers", "unit: Unit tests (fast, no real data needed)")
    config.addinivalue_line("markers", "integration: Integration tests (need real Nesstar data)")
    config.addinivalue_line("markers", "slow: Slow tests (full dataset validation)")
