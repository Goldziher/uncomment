# AI-RULEZ :: GENERATED FILE — DO NOT EDIT
# Content-Hash: blake3:f0861e3a09f285cd65f5c91452dbe9269b3364f36d138b56dab9f1e5f972ba27
# Source-Hash: blake3:d94e47c9b5760f2cf724e055e236d9021312752c3c3f065f46d95d0ea5818506
# Schema-Version: v1

"""Register the bundled read-only skill with Hermes Agent."""

from pathlib import Path
from typing import Protocol


class SkillContext(Protocol):
    """The registration API provided by the Hermes plugin SDK."""

    def register_skill(self, name: str, path: Path, description: str = "") -> None: ...


def register(ctx: SkillContext) -> None:
    """Resolve skill bytes relative to the installed native plugin."""
    ctx.register_skill(
        "uncomment",
        Path(__file__).parent / "skills/uncomment/SKILL.md",
        "Inventory, preserve, lint and remove source comments with Uncomment.",
    )
