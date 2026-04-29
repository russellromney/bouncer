from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Optional


@dataclass(frozen=True, slots=True)
class LeaseInfo:
    name: str
    owner: str
    token: int
    lease_expires_at_ms: int


@dataclass(frozen=True, slots=True)
class ClaimResult:
    acquired: bool
    lease: Optional[LeaseInfo]
    current: Optional[LeaseInfo]


@dataclass(frozen=True, slots=True)
class RenewResult:
    renewed: bool
    lease: Optional[LeaseInfo]
    current: Optional[LeaseInfo]


@dataclass(frozen=True, slots=True)
class ReleaseResult:
    released: bool
    name: str
    token: Optional[int]
    current: Optional[LeaseInfo]


def _lease_from_native(data: Optional[dict[str, Any]]) -> Optional[LeaseInfo]:
    if data is None:
        return None
    return LeaseInfo(
        name=str(data["name"]),
        owner=str(data["owner"]),
        token=int(data["token"]),
        lease_expires_at_ms=int(data["lease_expires_at_ms"]),
    )


def _claim_from_native(data: dict[str, Any]) -> ClaimResult:
    return ClaimResult(
        acquired=bool(data["acquired"]),
        lease=_lease_from_native(data.get("lease")),
        current=_lease_from_native(data.get("current")),
    )


def _renew_from_native(data: dict[str, Any]) -> RenewResult:
    return RenewResult(
        renewed=bool(data["renewed"]),
        lease=_lease_from_native(data.get("lease")),
        current=_lease_from_native(data.get("current")),
    )


def _release_from_native(data: dict[str, Any]) -> ReleaseResult:
    return ReleaseResult(
        released=bool(data["released"]),
        name=str(data["name"]),
        token=None if data.get("token") is None else int(data["token"]),
        current=_lease_from_native(data.get("current")),
    )
