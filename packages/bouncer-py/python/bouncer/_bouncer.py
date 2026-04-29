from __future__ import annotations

from os import PathLike
from typing import Optional

from bouncer._bouncer_native import NativeBouncer
from bouncer.models import (
    ClaimResult,
    LeaseInfo,
    ReleaseResult,
    RenewResult,
    _claim_from_native,
    _lease_from_native,
    _release_from_native,
    _renew_from_native,
)


class BouncerError(RuntimeError):
    """Umbrella exception for Bouncer's Python binding."""


def _raise_bouncer_error(exc: Exception) -> None:
    if isinstance(exc, BouncerError):
        raise exc
    raise BouncerError(str(exc)) from exc


def _call(fn, *args):
    try:
        return fn(*args)
    except Exception as exc:
        _raise_bouncer_error(exc)


def open(path: str | PathLike[str]) -> "Bouncer":
    return Bouncer(path)


class Bouncer:
    def __init__(self, path: str | PathLike[str]):
        self._native = _call(NativeBouncer, str(path))

    def bootstrap(self) -> None:
        _call(self._native.bootstrap)

    def inspect(self, name: str) -> Optional[LeaseInfo]:
        return _lease_from_native(_call(self._native.inspect, name))

    def claim(self, name: str, owner: str, *, ttl_ms: int) -> ClaimResult:
        return _claim_from_native(_call(self._native.claim, name, owner, int(ttl_ms)))

    def renew(self, name: str, owner: str, *, ttl_ms: int) -> RenewResult:
        return _renew_from_native(_call(self._native.renew, name, owner, int(ttl_ms)))

    def release(self, name: str, owner: str) -> ReleaseResult:
        return _release_from_native(_call(self._native.release, name, owner))

    def transaction(self) -> "Transaction":
        return Transaction(self._native)


class Transaction:
    def __init__(self, native: NativeBouncer):
        self._native = native
        self._entered = False
        self._finished = False

    def __enter__(self) -> "Transaction":
        if self._entered:
            raise BouncerError("transaction is already entered")
        if self._finished:
            raise BouncerError("transaction is already finished")
        _call(self._native.begin_transaction)
        self._entered = True
        return self

    def __exit__(self, exc_type, exc_value, traceback) -> bool:
        if not self._entered or self._finished:
            return False
        if exc_type is None:
            self.commit()
        else:
            self.rollback()
        return False

    def _ensure_active(self) -> None:
        if not self._entered:
            raise BouncerError(
                "transaction has not been entered; use `with db.transaction() as tx:`"
            )
        if self._finished:
            raise BouncerError("transaction is already finished")

    def execute(self, sql: str, params=None) -> int:
        self._ensure_active()
        return int(_call(self._native.execute_in_transaction, sql, _coerce_params(params)))

    def inspect(self, name: str) -> Optional[LeaseInfo]:
        self._ensure_active()
        return _lease_from_native(_call(self._native.inspect_in_transaction, name))

    def claim(self, name: str, owner: str, *, ttl_ms: int) -> ClaimResult:
        self._ensure_active()
        return _claim_from_native(
            _call(self._native.claim_in_transaction, name, owner, int(ttl_ms))
        )

    def renew(self, name: str, owner: str, *, ttl_ms: int) -> RenewResult:
        self._ensure_active()
        return _renew_from_native(
            _call(self._native.renew_in_transaction, name, owner, int(ttl_ms))
        )

    def release(self, name: str, owner: str) -> ReleaseResult:
        self._ensure_active()
        return _release_from_native(
            _call(self._native.release_in_transaction, name, owner)
        )

    def commit(self) -> None:
        self._ensure_active()
        _call(self._native.commit_transaction)
        self._finished = True

    def rollback(self) -> None:
        self._ensure_active()
        _call(self._native.rollback_transaction)
        self._finished = True


def _coerce_params(params):
    if params is None:
        return None
    if isinstance(params, (str, bytes, bytearray)):
        raise BouncerError("SQL params must be a sequence, not a string or bytes")
    try:
        return list(params)
    except TypeError as exc:
        raise BouncerError("SQL params must be a sequence") from exc
