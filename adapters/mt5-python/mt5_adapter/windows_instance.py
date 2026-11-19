"""Host-wide single-instance guard for the Windows MT5 worker.

The named mutex is scoped to the Quant account + terminal pair and is held
for the complete worker lifetime. The PostgreSQL lease remains authoritative
across hosts and provides fencing; this mutex only prevents duplicate local
processes/sessions on one Windows host.
"""
from __future__ import annotations

import ctypes
import hashlib
import sys
from contextlib import contextmanager
from typing import Iterator


ERROR_ALREADY_EXISTS = 183


class WindowsInstanceError(RuntimeError):
    """The Windows host cannot safely establish exclusive worker ownership."""


def mutex_name(account_id: str, terminal_id: str) -> str:
    """Return a stable, non-identifying Global namespace mutex name."""
    if (
        not account_id
        or not terminal_id
        or account_id.strip() != account_id
        or terminal_id.strip() != terminal_id
    ):
        raise WindowsInstanceError("MT5 account/terminal mutex scope is invalid")
    digest = hashlib.sha256(
        (account_id + chr(0) + terminal_id).encode("utf-8")
    ).hexdigest()
    return "Global\\QuantOS.MT5.Worker." + digest


@contextmanager
def account_terminal_mutex(account_id: str, terminal_id: str) -> Iterator[None]:
    """Acquire a Windows host-wide mutex or fail closed on duplicate ownership.

    Non-Windows development providers do not use a Windows mutex. They remain
    protected by their own provider contract and are not Windows deployment
    evidence.
    """
    if sys.platform != "win32":
        yield
        return

    name = mutex_name(account_id, terminal_id)
    try:
        kernel32 = ctypes.WinDLL("Kernel32", use_last_error=True)
        create_mutex = kernel32.CreateMutexW
        create_mutex.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_wchar_p]
        create_mutex.restype = ctypes.c_void_p
        release_mutex = kernel32.ReleaseMutex
        release_mutex.argtypes = [ctypes.c_void_p]
        release_mutex.restype = ctypes.c_int
        close_handle = kernel32.CloseHandle
        close_handle.argtypes = [ctypes.c_void_p]
        close_handle.restype = ctypes.c_int
        set_last_error = getattr(ctypes, "set_last_error", None)
        get_last_error = getattr(ctypes, "get_last_error", lambda: 0)
        if set_last_error is not None:
            set_last_error(0)
        handle = create_mutex(None, 1, name)
        error = get_last_error()
    except (AttributeError, OSError, TypeError) as exc:
        raise WindowsInstanceError(
            "Windows MT5 single-instance guard could not be initialized"
        ) from exc

    if not handle:
        raise WindowsInstanceError(
            "Windows MT5 single-instance guard could not acquire host ownership"
        )
    if error == ERROR_ALREADY_EXISTS:
        close_handle(handle)
        raise WindowsInstanceError(
            "another MT5 worker already owns this account/terminal on the Windows host"
        )

    try:
        yield
    finally:
        released = release_mutex(handle)
        closed = close_handle(handle)
        if not released or not closed:
            raise WindowsInstanceError(
                "Windows MT5 single-instance guard could not release its host handle cleanly"
            )
