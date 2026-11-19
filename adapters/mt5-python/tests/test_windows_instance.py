import ctypes
import unittest
from unittest.mock import patch

from mt5_adapter import windows_instance


class ApiCall:
    def __init__(self, result):
        self.result = result
        self.calls = []
        self.argtypes = None
        self.restype = None

    def __call__(self, *args):
        self.calls.append(args)
        return self.result


class Kernel32:
    def __init__(self, create_result=101, release_result=1, close_result=1):
        self.CreateMutexW = ApiCall(create_result)
        self.ReleaseMutex = ApiCall(release_result)
        self.CloseHandle = ApiCall(close_result)


class WindowsInstanceTests(unittest.TestCase):
    def test_mutex_name_is_stable_and_does_not_expose_identifiers(self):
        first = windows_instance.mutex_name("account-123", "terminal-456")
        self.assertEqual(first, windows_instance.mutex_name("account-123", "terminal-456"))
        self.assertTrue(first.startswith("Global\\QuantOS.MT5.Worker."))
        self.assertNotIn("account-123", first)
        self.assertNotIn("terminal-456", first)

    def test_mutex_scope_rejects_empty_or_untrimmed_identity(self):
        for account, terminal in (("", "terminal"), ("account", ""), (" account", "terminal")):
            with self.subTest(account=account, terminal=terminal):
                with self.assertRaises(windows_instance.WindowsInstanceError):
                    windows_instance.mutex_name(account, terminal)

    def test_non_windows_provider_does_not_load_windows_api(self):
        with patch.object(windows_instance.sys, "platform", "darwin"):
            with patch.object(ctypes, "WinDLL", create=True) as win_dll:
                with windows_instance.account_terminal_mutex("account", "terminal"):
                    pass
                win_dll.assert_not_called()

    def test_windows_mutex_is_held_and_released_for_context_lifetime(self):
        kernel = Kernel32()
        with patch.object(windows_instance.sys, "platform", "win32"):
            with patch.object(ctypes, "WinDLL", return_value=kernel, create=True):
                with patch.object(ctypes, "get_last_error", return_value=0, create=True):
                    with windows_instance.account_terminal_mutex("account", "terminal"):
                        self.assertEqual(len(kernel.ReleaseMutex.calls), 0)
                        self.assertEqual(len(kernel.CloseHandle.calls), 0)
        self.assertEqual(len(kernel.CreateMutexW.calls), 1)
        self.assertEqual(kernel.CreateMutexW.calls[0][1], 1)
        self.assertEqual(len(kernel.ReleaseMutex.calls), 1)
        self.assertEqual(len(kernel.CloseHandle.calls), 1)

    def test_duplicate_windows_worker_fails_closed(self):
        kernel = Kernel32()
        with patch.object(windows_instance.sys, "platform", "win32"):
            with patch.object(ctypes, "WinDLL", return_value=kernel, create=True):
                with patch.object(ctypes, "get_last_error", return_value=183, create=True):
                    with self.assertRaisesRegex(
                        windows_instance.WindowsInstanceError, "another MT5 worker"
                    ):
                        with windows_instance.account_terminal_mutex("account", "terminal"):
                            self.fail("duplicate worker must not enter")
        self.assertEqual(len(kernel.CloseHandle.calls), 1)
        self.assertEqual(len(kernel.ReleaseMutex.calls), 0)

    def test_windows_mutex_api_failure_fails_closed(self):
        kernel = Kernel32(create_result=None)
        with patch.object(windows_instance.sys, "platform", "win32"):
            with patch.object(ctypes, "WinDLL", return_value=kernel, create=True):
                with patch.object(ctypes, "get_last_error", return_value=5, create=True):
                    with self.assertRaises(windows_instance.WindowsInstanceError):
                        with windows_instance.account_terminal_mutex("account", "terminal"):
                            self.fail("unavailable mutex must not start worker")


if __name__ == "__main__":
    unittest.main()
