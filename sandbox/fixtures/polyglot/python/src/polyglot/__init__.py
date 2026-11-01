from importlib.metadata import PackageNotFoundError, version as dist_version

from .contract import MESSAGE

try:
    VERSION = dist_version("continuous-release-polyglot")
except PackageNotFoundError:
    VERSION = "0.0.0-dev"


def describe() -> str:
    return f"polyglot {VERSION} {MESSAGE}"
